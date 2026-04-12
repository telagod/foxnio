//! Model Registry Service
//!
//! 动态模型注册中心，支持数据库持久化和运行时热加载

#![allow(dead_code)]
use anyhow::{bail, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::entity::model_configs::{
    self, CreateModelRequest, ModelCapabilities, ModelInfoResponse, UpdateModelRequest,
};
use crate::gateway::providers::default_provider_registry;

/// 模型配置（运行时）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeModelConfig {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
    pub provider: String,
    pub api_name: String,
    pub display_name: String,
    pub input_price: f64,
    pub output_price: f64,
    pub max_tokens: i32,
    pub context_window: i32,
    pub max_concurrent: i32,
    pub fallback_models: Vec<String>,
    pub capabilities: ModelCapabilities,
    pub supports_streaming: bool,
    pub supports_function_calling: bool,
    pub supports_vision: bool,
    pub enabled: bool,
    pub priority: i32,
}

impl From<model_configs::Model> for RuntimeModelConfig {
    fn from(m: model_configs::Model) -> Self {
        let caps = m
            .capabilities
            .as_ref()
            .and_then(|v| serde_json::from_value::<ModelCapabilities>(v.clone()).ok())
            .unwrap_or_default();

        let aliases = m.get_aliases();
        let fallback_models = m.get_fallback_models();

        Self {
            id: m.id,
            name: m.name,
            aliases,
            provider: m.provider,
            api_name: m.api_name,
            display_name: m.display_name,
            input_price: m.input_price,
            output_price: m.output_price,
            max_tokens: m.max_tokens,
            context_window: m.context_window,
            max_concurrent: m.max_concurrent,
            fallback_models,
            capabilities: caps,
            supports_streaming: m.supports_streaming,
            supports_function_calling: m.supports_function_calling,
            supports_vision: m.supports_vision,
            enabled: m.enabled,
            priority: m.priority,
        }
    }
}

/// 提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub base_url: String,
    pub auth_header: String,
    pub requires_version_header: bool,
    pub api_version: Option<String>,
}

impl ProviderConfig {
    /// 获取所有提供商配置
    pub fn get(provider: &str) -> Option<Self> {
        let normalized = provider.to_lowercase();
        default_provider_registry()
            .get(&normalized)
            .map(|adapter| adapter.descriptor())
            .map(|descriptor| Self {
                provider: normalized,
                base_url: descriptor.base_url,
                auth_header: descriptor.auth_header,
                requires_version_header: descriptor.requires_version_header,
                api_version: descriptor.api_version,
            })
    }

    /// 获取所有提供商
    pub fn all() -> HashMap<String, Self> {
        default_provider_registry()
            .descriptors()
            .into_iter()
            .map(|descriptor| {
                (
                    descriptor.key.clone(),
                    Self {
                        provider: descriptor.key,
                        base_url: descriptor.base_url,
                        auth_header: descriptor.auth_header,
                        requires_version_header: descriptor.requires_version_header,
                        api_version: descriptor.api_version,
                    },
                )
            })
            .collect()
    }
}

/// 路由结果
#[derive(Debug, Clone)]
pub struct RouteResult {
    pub model: RuntimeModelConfig,
    pub provider_config: ProviderConfig,
    pub is_fallback: bool,
    pub original_model: Option<String>,
}

/// 降级事件
#[derive(Debug, Clone)]
pub struct FallbackEvent {
    pub original_model: String,
    pub fallback_model: String,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 模型注册中心
#[derive(Clone)]
pub struct ModelRegistry {
    db: DatabaseConnection,
    /// 模型缓存（按名称索引）
    models: Arc<RwLock<HashMap<String, RuntimeModelConfig>>>,
    /// 别名索引（别名 -> 模型名）
    alias_index: Arc<RwLock<HashMap<String, String>>>,
    /// 模型可用性状态
    availability: Arc<RwLock<HashMap<String, bool>>>,
    /// 降级历史
    fallback_history: Arc<RwLock<Vec<FallbackEvent>>>,
}

impl ModelRegistry {
    /// 创建新的模型注册中心
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            models: Arc::new(RwLock::new(HashMap::new())),
            alias_index: Arc::new(RwLock::new(HashMap::new())),
            availability: Arc::new(RwLock::new(HashMap::new())),
            fallback_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 从数据库加载所有模型配置
    pub async fn load_from_db(&self) -> Result<()> {
        let models = model_configs::Entity::find()
            .filter(model_configs::Column::Enabled.eq(true))
            .order_by_desc(model_configs::Column::Priority)
            .all(&self.db)
            .await?;

        let mut model_map = HashMap::new();
        let mut alias_map = HashMap::new();

        for m in models {
            let config: RuntimeModelConfig = m.clone().into();
            let name = config.name.clone();

            // 索引模型名
            model_map.insert(name.clone(), config.clone());

            // 索引别名
            for alias in &config.aliases {
                alias_map.insert(alias.to_lowercase(), name.clone());
            }
            // 模型名本身也作为别名
            alias_map.insert(name.to_lowercase(), name.clone());
        }

        let mut models_cache = self.models.write().await;
        let mut aliases_cache = self.alias_index.write().await;

        *models_cache = model_map;
        *aliases_cache = alias_map;

        info!("Loaded {} models from database", models_cache.len());
        Ok(())
    }

    /// 热加载（重新从数据库加载）
    pub async fn reload(&self) -> Result<()> {
        info!("Reloading model configurations...");
        self.load_from_db().await
    }

    /// 解析模型名称（支持别名）
    pub async fn resolve(&self, name: &str) -> Option<RuntimeModelConfig> {
        let alias_map = self.alias_index.read().await;
        let model_name = alias_map.get(&name.to_lowercase())?;

        let models = self.models.read().await;
        models.get(model_name).cloned()
    }

    /// 获取模型配置
    pub async fn get(&self, name: &str) -> Option<RuntimeModelConfig> {
        let models = self.models.read().await;
        models.get(name).cloned()
    }

    /// 列出所有模型
    pub async fn list_all(&self) -> Vec<RuntimeModelConfig> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// 列出所有启用的模型
    pub async fn list_enabled(&self) -> Vec<RuntimeModelConfig> {
        let models = self.models.read().await;
        models.values().filter(|m| m.enabled).cloned().collect()
    }

    /// 列出所有模型（API 响应格式）
    pub async fn list_models_info(&self) -> Vec<ModelInfoResponse> {
        let models = self.models.read().await;
        models
            .values()
            .map(|m| ModelInfoResponse {
                id: m.name.clone(),
                name: m.display_name.clone(),
                provider: m.provider.clone(),
                context_window: m.context_window as u32,
                max_tokens: m.max_tokens as u32,
                input_price: m.input_price,
                output_price: m.output_price,
                capabilities: m.get_capabilities(),
                enabled: m.enabled,
            })
            .collect()
    }

    /// 按提供商列出模型
    pub async fn list_by_provider(&self, provider: &str) -> Vec<RuntimeModelConfig> {
        let models = self.models.read().await;
        models
            .values()
            .filter(|m| m.provider.to_lowercase() == provider.to_lowercase())
            .cloned()
            .collect()
    }

    /// 路由模型（支持降级）
    pub async fn route(&self, model_name: &str) -> Result<RouteResult> {
        // 解析模型
        let model = self
            .resolve(model_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;

        // 检查可用性
        if self.is_available(&model.name).await {
            let provider_config = ProviderConfig::get(&model.provider)
                .ok_or_else(|| anyhow::anyhow!("Unknown provider: {}", model.provider))?;

            return Ok(RouteResult {
                model,
                provider_config,
                is_fallback: false,
                original_model: None,
            });
        }

        // 尝试降级
        self.try_fallback(model_name, &model).await
    }

    /// 尝试降级
    async fn try_fallback(
        &self,
        original_name: &str,
        original: &RuntimeModelConfig,
    ) -> Result<RouteResult> {
        for fallback_name in &original.fallback_models {
            if let Some(fallback) = self.resolve(fallback_name).await {
                if self.is_available(&fallback.name).await {
                    // 记录降级事件
                    self.record_fallback(FallbackEvent {
                        original_model: original_name.to_string(),
                        fallback_model: fallback.name.clone(),
                        reason: "Original model unavailable".to_string(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;

                    let provider_config =
                        ProviderConfig::get(&fallback.provider).ok_or_else(|| {
                            anyhow::anyhow!("Unknown provider: {}", fallback.provider)
                        })?;

                    return Ok(RouteResult {
                        model: fallback,
                        provider_config,
                        is_fallback: true,
                        original_model: Some(original_name.to_string()),
                    });
                }
            }
        }

        bail!("No available fallback model for: {}", original_name)
    }

    /// 检查模型是否可用
    pub async fn is_available(&self, model_name: &str) -> bool {
        let availability = self.availability.read().await;
        availability.get(model_name).copied().unwrap_or(true)
    }

    /// 设置模型可用性
    pub async fn set_available(&self, model_name: &str, available: bool) {
        let mut availability = self.availability.write().await;
        availability.insert(model_name.to_string(), available);
        debug!("Model {} availability: {}", model_name, available);
    }

    /// 记录降级事件
    async fn record_fallback(&self, event: FallbackEvent) {
        let mut history = self.fallback_history.write().await;
        history.push(event);

        // 只保留最近 100 条
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// 获取降级历史
    pub async fn get_fallback_history(&self) -> Vec<FallbackEvent> {
        self.fallback_history.read().await.clone()
    }

    // ============ CRUD 操作 ============

    /// 创建模型
    pub async fn create(&self, req: CreateModelRequest) -> Result<RuntimeModelConfig> {
        // 检查模型是否已存在
        if self.resolve(&req.name).await.is_some() {
            bail!("Model already exists: {}", req.name);
        }

        let now = Utc::now();
        let api_name = req.api_name.unwrap_or_else(|| req.name.clone());
        let display_name = req.display_name.unwrap_or_else(|| req.name.clone());

        let model = model_configs::ActiveModel {
            id: Set(0), // Auto-increment
            name: Set(req.name.clone()),
            aliases: Set(Some(serde_json::to_value(&req.aliases)?)),
            provider: Set(req.provider.clone()),
            api_name: Set(api_name),
            display_name: Set(display_name),
            input_price: Set(req.input_price),
            output_price: Set(req.output_price),
            max_tokens: Set(req.max_tokens),
            context_window: Set(req.context_window),
            max_concurrent: Set(req.max_concurrent),
            fallback_models: Set(Some(serde_json::to_value(&req.fallback_models)?)),
            capabilities: Set(Some(serde_json::to_value(&req.capabilities)?)),
            supports_streaming: Set(req.supports_streaming),
            supports_function_calling: Set(req.supports_function_calling),
            supports_vision: Set(req.supports_vision),
            enabled: Set(req.enabled),
            priority: Set(req.priority),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let inserted = model.insert(&self.db).await?;

        // 更新缓存
        self.load_from_db().await?;

        info!("Created model: {}", inserted.name);
        Ok(inserted.into())
    }

    /// 更新模型
    pub async fn update(&self, id: i64, req: UpdateModelRequest) -> Result<RuntimeModelConfig> {
        let existing = model_configs::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", id))?;

        let now = Utc::now();
        let mut model: model_configs::ActiveModel = existing.into();

        if let Some(name) = req.name {
            model.name = Set(name);
        }
        if let Some(aliases) = req.aliases {
            model.aliases = Set(Some(serde_json::to_value(&aliases)?));
        }
        if let Some(provider) = req.provider {
            model.provider = Set(provider);
        }
        if let Some(api_name) = req.api_name {
            model.api_name = Set(api_name);
        }
        if let Some(display_name) = req.display_name {
            model.display_name = Set(display_name);
        }
        if let Some(input_price) = req.input_price {
            model.input_price = Set(input_price);
        }
        if let Some(output_price) = req.output_price {
            model.output_price = Set(output_price);
        }
        if let Some(max_tokens) = req.max_tokens {
            model.max_tokens = Set(max_tokens);
        }
        if let Some(context_window) = req.context_window {
            model.context_window = Set(context_window);
        }
        if let Some(max_concurrent) = req.max_concurrent {
            model.max_concurrent = Set(max_concurrent);
        }
        if let Some(fallback_models) = req.fallback_models {
            model.fallback_models = Set(Some(serde_json::to_value(&fallback_models)?));
        }
        if let Some(capabilities) = req.capabilities {
            model.capabilities = Set(Some(serde_json::to_value(&capabilities)?));
        }
        if let Some(supports_streaming) = req.supports_streaming {
            model.supports_streaming = Set(supports_streaming);
        }
        if let Some(supports_function_calling) = req.supports_function_calling {
            model.supports_function_calling = Set(supports_function_calling);
        }
        if let Some(supports_vision) = req.supports_vision {
            model.supports_vision = Set(supports_vision);
        }
        if let Some(enabled) = req.enabled {
            model.enabled = Set(enabled);
        }
        if let Some(priority) = req.priority {
            model.priority = Set(priority);
        }

        model.updated_at = Set(now);

        let updated = model.update(&self.db).await?;

        // 更新缓存
        self.load_from_db().await?;

        info!("Updated model: {}", updated.name);
        Ok(updated.into())
    }

    /// 删除模型
    pub async fn delete(&self, id: i64) -> Result<()> {
        let model = model_configs::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", id))?;

        model_configs::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;

        // 更新缓存
        self.load_from_db().await?;

        info!("Deleted model: {}", model.name);
        Ok(())
    }

    /// 根据 ID 获取模型
    pub async fn get_by_id(&self, id: i64) -> Result<RuntimeModelConfig> {
        let model = model_configs::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", id))?;

        Ok(model.into())
    }

    /// 批量导入默认模型
    pub async fn import_defaults(&self) -> Result<usize> {
        let defaults = Self::get_default_models();
        let mut count = 0;

        for model in defaults {
            match self.create(model).await {
                Ok(_) => count += 1,
                Err(e) => warn!("Failed to import model: {}", e),
            }
        }

        info!("Imported {} default models", count);
        Ok(count)
    }

    /// 获取默认模型列表
    fn get_default_models() -> Vec<CreateModelRequest> {
        vec![
            // OpenAI 模型
            CreateModelRequest {
                name: "gpt-4-turbo".to_string(),
                aliases: vec!["gpt4t".into(), "gpt-4-turbo-preview".into()],
                provider: "openai".to_string(),
                api_name: Some("gpt-4-turbo-preview".to_string()),
                display_name: Some("GPT-4 Turbo".to_string()),
                input_price: 10.0,
                output_price: 30.0,
                max_tokens: 4096,
                context_window: 128000,
                max_concurrent: 5,
                fallback_models: vec!["gpt-4".into(), "gpt-4o".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 100,
            },
            CreateModelRequest {
                name: "gpt-4o".to_string(),
                aliases: vec!["gpt4o".into()],
                provider: "openai".to_string(),
                api_name: Some("gpt-4o".to_string()),
                display_name: Some("GPT-4o".to_string()),
                input_price: 5.0,
                output_price: 15.0,
                max_tokens: 4096,
                context_window: 128000,
                max_concurrent: 5,
                fallback_models: vec!["gpt-4-turbo".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 90,
            },
            CreateModelRequest {
                name: "gpt-4o-mini".to_string(),
                aliases: vec!["gpt4o-mini".into()],
                provider: "openai".to_string(),
                api_name: Some("gpt-4o-mini".to_string()),
                display_name: Some("GPT-4o Mini".to_string()),
                input_price: 0.15,
                output_price: 0.6,
                max_tokens: 4096,
                context_window: 128000,
                max_concurrent: 10,
                fallback_models: vec!["gpt-3.5-turbo".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 80,
            },
            CreateModelRequest {
                name: "gpt-3.5-turbo".to_string(),
                aliases: vec!["gpt35".into(), "gpt-3.5".into()],
                provider: "openai".to_string(),
                api_name: Some("gpt-3.5-turbo".to_string()),
                display_name: Some("GPT-3.5 Turbo".to_string()),
                input_price: 0.5,
                output_price: 1.5,
                max_tokens: 4096,
                context_window: 16385,
                max_concurrent: 10,
                fallback_models: vec!["gpt-4o-mini".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
                enabled: true,
                priority: 70,
            },
            // Anthropic 模型
            CreateModelRequest {
                name: "claude-3-opus".to_string(),
                aliases: vec!["claude-opus".into()],
                provider: "anthropic".to_string(),
                api_name: Some("claude-3-opus-20240229".to_string()),
                display_name: Some("Claude 3 Opus".to_string()),
                input_price: 15.0,
                output_price: 75.0,
                max_tokens: 4096,
                context_window: 200000,
                max_concurrent: 5,
                fallback_models: vec!["claude-3.5-sonnet".into(), "claude-3-haiku".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 100,
            },
            CreateModelRequest {
                name: "claude-3.5-sonnet".to_string(),
                aliases: vec!["claude-sonnet".into(), "claude-3-5-sonnet".into()],
                provider: "anthropic".to_string(),
                api_name: Some("claude-3-5-sonnet-20241022".to_string()),
                display_name: Some("Claude 3.5 Sonnet".to_string()),
                input_price: 3.0,
                output_price: 15.0,
                max_tokens: 8192,
                context_window: 200000,
                max_concurrent: 10,
                fallback_models: vec!["claude-3-haiku".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 95,
            },
            CreateModelRequest {
                name: "claude-3-haiku".to_string(),
                aliases: vec!["claude-haiku".into()],
                provider: "anthropic".to_string(),
                api_name: Some("claude-3-haiku-20240307".to_string()),
                display_name: Some("Claude 3 Haiku".to_string()),
                input_price: 0.25,
                output_price: 1.25,
                max_tokens: 4096,
                context_window: 200000,
                max_concurrent: 15,
                fallback_models: vec![],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 85,
            },
            // DeepSeek 模型
            CreateModelRequest {
                name: "deepseek-v3".to_string(),
                aliases: vec!["deepseek".into(), "ds-v3".into()],
                provider: "deepseek".to_string(),
                api_name: Some("deepseek-chat".to_string()),
                display_name: Some("DeepSeek V3".to_string()),
                input_price: 0.14,
                output_price: 0.28,
                max_tokens: 4096,
                context_window: 64000,
                max_concurrent: 10,
                fallback_models: vec!["deepseek-coder".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
                enabled: true,
                priority: 80,
            },
            CreateModelRequest {
                name: "deepseek-coder".to_string(),
                aliases: vec!["dscoder".into()],
                provider: "deepseek".to_string(),
                api_name: Some("deepseek-coder".to_string()),
                display_name: Some("DeepSeek Coder".to_string()),
                input_price: 0.14,
                output_price: 0.28,
                max_tokens: 4096,
                context_window: 16000,
                max_concurrent: 10,
                fallback_models: vec!["deepseek-v3".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: false,
                },
                supports_streaming: true,
                supports_function_calling: false,
                supports_vision: false,
                enabled: true,
                priority: 75,
            },
            // Google 模型
            CreateModelRequest {
                name: "gemini-1.5-pro".to_string(),
                aliases: vec!["gemini-pro".into(), "gemini".into()],
                provider: "google".to_string(),
                api_name: Some("gemini-1.5-pro".to_string()),
                display_name: Some("Gemini 1.5 Pro".to_string()),
                input_price: 3.5,
                output_price: 10.5,
                max_tokens: 8192,
                context_window: 1000000,
                max_concurrent: 5,
                fallback_models: vec!["gemini-pro".into()],
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
                enabled: true,
                priority: 90,
            },
        ]
    }
}

/// RuntimeModelConfig 的能力方法
impl RuntimeModelConfig {
    pub fn get_capabilities(&self) -> Vec<String> {
        let mut capabilities = Vec::new();
        if self.capabilities.chat {
            capabilities.push("chat".to_string());
        }
        if self.capabilities.code {
            capabilities.push("code".to_string());
        }
        if self.capabilities.math {
            capabilities.push("math".to_string());
        }
        if self.capabilities.long_context {
            capabilities.push("long_context".to_string());
        }
        if self.capabilities.multilingual {
            capabilities.push("multilingual".to_string());
        }
        if self.capabilities.tools {
            capabilities.push("tools".to_string());
        }
        if self.supports_vision {
            capabilities.push("vision".to_string());
        }
        if self.supports_function_calling {
            capabilities.push("function_calling".to_string());
        }
        capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config() {
        let openai = ProviderConfig::get("openai").unwrap();
        assert_eq!(openai.base_url, "https://api.openai.com");
        assert_eq!(openai.auth_header, "Authorization");

        let anthropic = ProviderConfig::get("anthropic").unwrap();
        assert_eq!(anthropic.auth_header, "x-api-key");
        assert!(anthropic.requires_version_header);
    }

    #[test]
    fn test_default_models() {
        let defaults = ModelRegistry::get_default_models();
        assert!(!defaults.is_empty());
        assert!(defaults.iter().any(|m| m.name == "gpt-4o"));
        assert!(defaults.iter().any(|m| m.name == "claude-3.5-sonnet"));
    }
}
