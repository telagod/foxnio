//! 模型路由服务
//!
//! 提供智能模型路由、降级和参数映射功能

#![allow(dead_code)]
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::gateway::models::{
    get_model_config, get_model_info, list_all_models, list_models_by_provider,
    resolve_model_alias, Model, ModelConfig, ModelInfo, ModelProvider, ProviderConfig,
};

/// 模型路由器
#[derive(Clone)]

pub struct ModelRouter {
    /// 提供商配置
    providers: HashMap<ModelProvider, ProviderConfig>,
    /// 模型可用性状态
    model_availability: Arc<RwLock<HashMap<Model, bool>>>,
    /// 提供商可用性状态
    provider_availability: Arc<RwLock<HashMap<ModelProvider, bool>>>,
    /// 降级历史
    fallback_history: Arc<RwLock<Vec<FallbackEvent>>>,
}

/// 降级事件
#[derive(Debug, Clone)]
pub struct FallbackEvent {
    pub original_model: Model,
    pub fallback_model: Model,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 模型状态
#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub model: Model,
    pub is_available: bool,
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

/// 路由结果
#[derive(Debug, Clone)]
pub struct RouteResult {
    pub model: Model,
    pub provider: ModelProvider,
    pub config: ModelConfig,
    pub provider_config: ProviderConfig,
    pub is_fallback: bool,
    pub original_model: Option<Model>,
}

impl ModelRouter {
    /// 创建新的模型路由器
    pub fn new() -> Self {
        Self {
            providers: ProviderConfig::all(),
            model_availability: Arc::new(RwLock::new(HashMap::new())),
            provider_availability: Arc::new(RwLock::new(HashMap::new())),
            fallback_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 路由模型到对应的提供商
    pub fn route(&self, model: Model) -> Result<RouteResult> {
        // 获取模型配置
        let config = get_model_config(model)
            .ok_or_else(|| anyhow::anyhow!("Model config not found: {:?}", model))?;

        // 获取提供商配置
        let provider = model.provider();
        let provider_config = ProviderConfig::get(provider);

        Ok(RouteResult {
            model,
            provider,
            config,
            provider_config,
            is_fallback: false,
            original_model: None,
        })
    }

    /// 路由模型，支持降级
    pub async fn route_with_fallback(&self, model_name: &str) -> Result<RouteResult> {
        // 解析模型名称
        let model = resolve_model_alias(model_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;

        // 检查模型是否可用
        if self.is_model_available(model).await {
            return self.route(model);
        }

        // 尝试降级
        self.try_fallback(model).await
    }

    /// 尝试降级模型
    async fn try_fallback(&self, original_model: Model) -> Result<RouteResult> {
        let fallbacks = original_model.fallback_models();

        for fallback_model in fallbacks {
            if self.is_model_available(fallback_model).await {
                // 记录降级事件
                self.record_fallback(FallbackEvent {
                    original_model,
                    fallback_model,
                    reason: "Original model unavailable".to_string(),
                    timestamp: chrono::Utc::now(),
                })
                .await;

                // 路由到降级模型
                let config = get_model_config(fallback_model).ok_or_else(|| {
                    anyhow::anyhow!("Model config not found: {:?}", fallback_model)
                })?;

                let provider = fallback_model.provider();
                let provider_config = ProviderConfig::get(provider);

                return Ok(RouteResult {
                    model: fallback_model,
                    provider,
                    config,
                    provider_config,
                    is_fallback: true,
                    original_model: Some(original_model),
                });
            }
        }

        bail!("No available fallback model for {:?}", original_model)
    }

    /// 检查模型是否可用
    pub async fn is_model_available(&self, model: Model) -> bool {
        let availability = self.model_availability.read().await;

        // 如果没有明确设置为不可用，则认为可用
        availability.get(&model).copied().unwrap_or(true)
    }

    /// 检查提供商是否可用
    pub async fn is_provider_available(&self, provider: ModelProvider) -> bool {
        let availability = self.provider_availability.read().await;

        // 如果没有明确设置为不可用，则认为可用
        availability.get(&provider).copied().unwrap_or(true)
    }

    /// 设置模型可用性
    pub async fn set_model_available(&self, model: Model, available: bool) {
        let mut availability = self.model_availability.write().await;
        availability.insert(model, available);
    }

    /// 设置提供商可用性
    pub async fn set_provider_available(&self, provider: ModelProvider, available: bool) {
        let mut availability = self.provider_availability.write().await;
        availability.insert(provider, available);

        // 同时更新该提供商下所有模型的可用性
        let mut model_availability = self.model_availability.write().await;
        for model in Model::by_provider(provider) {
            model_availability.insert(model, available);
        }
    }

    /// 获取模型信息
    pub fn get_model_info(&self, model: Model) -> Option<ModelInfo> {
        get_model_info(model)
    }

    /// 列出所有可用模型
    pub fn list_available_models(&self) -> Vec<ModelInfo> {
        list_all_models()
    }

    /// 按提供商列出模型
    pub fn list_models_by_provider(&self, provider: ModelProvider) -> Vec<ModelInfo> {
        list_models_by_provider(provider)
    }

    /// 获取降级历史
    pub async fn get_fallback_history(&self) -> Vec<FallbackEvent> {
        self.fallback_history.read().await.clone()
    }

    /// 记录降级事件
    async fn record_fallback(&self, event: FallbackEvent) {
        let mut history = self.fallback_history.write().await;
        history.push(event);

        // 只保留最近 100 条记录
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// 映射请求参数
    ///
    /// 将统一格式的请求参数映射到特定提供商的格式
    pub fn map_request_params(&self, model: Model, params: &mut serde_json::Value) -> Result<()> {
        // 确保使用正确的模型名称
        let api_name = model.api_name();

        if let Some(obj) = params.as_object_mut() {
            // 设置模型名称
            obj.insert("model".to_string(), serde_json::json!(api_name));

            // 根据提供商调整参数
            match model.provider() {
                ModelProvider::Anthropic => {
                    // Anthropic 使用 max_tokens 而不是 max_completion_tokens
                    if let Some(max_tokens) = obj.remove("max_completion_tokens") {
                        obj.insert("max_tokens".to_string(), max_tokens);
                    }

                    // Anthropic 不支持某些参数
                    obj.remove("logprobs");
                    obj.remove("top_logprobs");
                }
                ModelProvider::OpenAI => {
                    // OpenAI 使用 max_completion_tokens
                    if let Some(max_tokens) = obj.remove("max_tokens") {
                        if !obj.contains_key("max_completion_tokens") {
                            obj.insert("max_completion_tokens".to_string(), max_tokens);
                        }
                    }
                }
                ModelProvider::Google => {
                    // Gemini 参数格式不同
                    if let Some(messages) = obj.remove("messages") {
                        obj.insert("contents".to_string(), messages);
                    }
                    if let Some(max_tokens) = obj.remove("max_tokens") {
                        obj.insert(
                            "generationConfig".to_string(),
                            serde_json::json!({"maxOutputTokens": max_tokens}),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取模型状态
    pub async fn get_model_status(&self, model: Model) -> ModelStatus {
        let availability = self.model_availability.read().await;
        let is_available = availability.get(&model).copied().unwrap_or(true);

        ModelStatus {
            model,
            is_available,
            last_check: None,
            error_count: 0,
            last_error: None,
        }
    }

    /// 批量获取模型状态
    pub async fn get_all_model_status(&self) -> HashMap<Model, ModelStatus> {
        let mut statuses = HashMap::new();

        for model in Model::all() {
            statuses.insert(model, self.get_model_status(model).await);
        }

        statuses
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_route_model() {
        let router = ModelRouter::new();
        let result = router.route(Model::GPT4Turbo).unwrap();

        assert_eq!(result.model, Model::GPT4Turbo);
        assert_eq!(result.provider, ModelProvider::OpenAI);
        assert!(!result.is_fallback);
    }

    #[tokio::test]
    async fn test_route_with_alias() {
        let router = ModelRouter::new();
        let result = router.route_with_fallback("gpt4t").await.unwrap();

        assert_eq!(result.model, Model::GPT4Turbo);
    }

    #[tokio::test]
    async fn test_fallback() {
        let router = ModelRouter::new();

        // 设置模型不可用
        router.set_model_available(Model::GPT4Turbo, false).await;

        // 应该降级
        let result = router.route_with_fallback("gpt-4-turbo").await.unwrap();
        assert!(result.is_fallback);
        assert_eq!(result.original_model, Some(Model::GPT4Turbo));
        assert_ne!(result.model, Model::GPT4Turbo);
    }

    #[tokio::test]
    async fn test_provider_unavailable() {
        let router = ModelRouter::new();

        // 设置整个提供商不可用
        router
            .set_provider_available(ModelProvider::OpenAI, false)
            .await;

        // 所有 OpenAI 模型应该不可用
        assert!(!router.is_model_available(Model::GPT4).await);
        assert!(!router.is_model_available(Model::GPT35Turbo).await);
    }

    #[test]
    fn test_list_models() {
        let router = ModelRouter::new();
        let models = router.list_available_models();

        assert!(models.len() >= 12);
    }

    #[test]
    fn test_map_params_openai() {
        let router = ModelRouter::new();
        let mut params = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 100,
        });

        router.map_request_params(Model::GPT4, &mut params).unwrap();

        assert_eq!(params["model"], "gpt-4");
        assert!(
            params.get("max_completion_tokens").is_some() || params.get("max_tokens").is_some()
        );
    }

    #[test]
    fn test_map_params_anthropic() {
        let router = ModelRouter::new();
        let mut params = serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}],
            "max_completion_tokens": 100,
        });

        router
            .map_request_params(Model::Claude35Sonnet, &mut params)
            .unwrap();

        assert_eq!(params["model"], "claude-3-5-sonnet-20241022");
        assert!(params.get("max_tokens").is_some());
    }

    #[tokio::test]
    async fn test_fallback_history() {
        let router = ModelRouter::new();

        router.set_model_available(Model::GPT4Turbo, false).await;
        router.route_with_fallback("gpt-4-turbo").await.unwrap();

        let history = router.get_fallback_history().await;
        assert!(!history.is_empty());
    }

    #[test]
    fn test_get_model_info() {
        let router = ModelRouter::new();
        let info = router.get_model_info(Model::Claude3Opus);

        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "Claude 3 Opus");
    }

    #[tokio::test]
    async fn test_model_status() {
        let router = ModelRouter::new();
        router.set_model_available(Model::GPT4o, false).await;

        let status = router.get_model_status(Model::GPT4o).await;
        assert!(!status.is_available);
    }
}
