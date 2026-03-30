//! Model Sync Service
//!
//! 模型自动同步服务，从各 AI 服务商获取最新模型信息

#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::model_registry::{CreateModelRequest, ModelRegistry, RuntimeModelConfig};

/// 最大重试次数
const MAX_RETRIES: u32 = 3;
/// 重试延迟（毫秒）
const RETRY_DELAY_MS: u64 = 1000;

/// 模型同步服务
pub struct ModelSyncService {
    db: sea_orm::DatabaseConnection,
    model_registry: Arc<ModelRegistry>,
    http_client: reqwest::Client,
    sync_state: Arc<RwLock<SyncState>>,
    /// API 密钥配置
    api_keys: Arc<RwLock<HashMap<String, String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub last_sync: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub in_progress: bool,
    pub provider_status: HashMap<String, ProviderSyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSyncStatus {
    pub provider: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub models_count: usize,
    pub last_error: Option<String>,
}

/// 同步结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub provider: String,
    pub new_models: Vec<String>,
    pub updated_models: Vec<String>,
    pub deprecated_models: Vec<String>,
    pub price_changes: Vec<PriceChange>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceChange {
    pub model_name: String,
    pub old_input_price: f64,
    pub new_input_price: f64,
    pub old_output_price: f64,
    pub new_output_price: f64,
    pub change_time: DateTime<Utc>,
}

/// OpenAI 模型列表响应
#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}

/// Anthropic 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicModelInfo {
    name: String,
    display_name: String,
    input_price: f64,
    output_price: f64,
    context_window: i32,
    max_tokens: i32,
    supports_vision: bool,
}

/// Google Gemini 模型响应
#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<i32>,
    #[serde(rename = "outputTokenLimit")]
    output_token_limit: Option<i32>,
}

/// DeepSeek 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeepSeekModelInfo {
    name: String,
    display_name: String,
    input_price: f64,
    output_price: f64,
    context_window: i32,
    max_tokens: i32,
}

/// Mistral 模型响应
#[derive(Debug, Deserialize)]
struct MistralModelsResponse {
    data: Vec<MistralModel>,
}

#[derive(Debug, Deserialize)]
struct MistralModel {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}

/// Cohere 模型响应
#[derive(Debug, Deserialize)]
struct CohereModelsResponse {
    models: Vec<CohereModel>,
}

#[derive(Debug, Deserialize)]
struct CohereModel {
    name: String,
    #[serde(rename = "deploymentLabel")]
    deployment_label: Option<String>,
}

impl ModelSyncService {
    pub fn new(db: sea_orm::DatabaseConnection, model_registry: Arc<ModelRegistry>) -> Self {
        Self {
            db,
            model_registry,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("OpenClaw-ModelSync/1.0")
                .build()
                .unwrap(),
            sync_state: Arc::new(RwLock::new(SyncState::default())),
            api_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置 API 密钥
    pub async fn set_api_key(&self, provider: &str, api_key: String) {
        let mut keys = self.api_keys.write().await;
        keys.insert(provider.to_string(), api_key);
        info!("API key set for provider: {}", provider);
    }

    /// 获取 API 密钥
    async fn get_api_key(&self, provider: &str) -> Result<String> {
        let keys = self.api_keys.read().await;
        keys.get(provider)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("API key not configured for provider: {}", provider))
    }

    /// 同步所有提供商的模型
    pub async fn sync_all(&self) -> Result<Vec<SyncResult>> {
        let mut state = self.sync_state.write().await;
        if state.in_progress {
            bail!("Sync already in progress");
        }
        state.in_progress = true;
        drop(state);

        info!("Starting full model sync for all providers");
        let mut results = Vec::new();

        // 同步各提供商
        for provider in &["openai", "anthropic", "google", "deepseek", "mistral", "cohere"] {
            match self.sync_provider_with_retry(provider).await {
                Ok(result) => {
                    info!(
                        "Provider {} synced: {} new, {} updated, {} deprecated",
                        provider,
                        result.new_models.len(),
                        result.updated_models.len(),
                        result.deprecated_models.len()
                    );
                    results.push(result);
                }
                Err(e) => {
                    error!("Failed to sync provider {}: {}", provider, e);
                    results.push(SyncResult {
                        provider: provider.to_string(),
                        new_models: vec![],
                        updated_models: vec![],
                        deprecated_models: vec![],
                        price_changes: vec![],
                        errors: vec![e.to_string()],
                    });
                }
            }
        }

        let mut state = self.sync_state.write().await;
        state.in_progress = false;
        state.last_sync = Some(Utc::now());

        // 检查是否有错误
        let has_errors = results.iter().any(|r| !r.errors.is_empty());
        if !has_errors {
            state.last_success = Some(Utc::now());
            state.last_error = None;
        } else {
            state.last_error = Some("Some providers failed to sync".to_string());
        }

        Ok(results)
    }

    /// 带重试的提供商同步
    async fn sync_provider_with_retry(&self, provider: &str) -> Result<SyncResult> {
        let mut last_error = None;

        for attempt in 1..=MAX_RETRIES {
            match self.sync_provider(provider).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!(
                        "Sync attempt {}/{} failed for {}: {}",
                        attempt, MAX_RETRIES, provider, e
                    );
                    last_error = Some(e);

                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            RETRY_DELAY_MS * attempt as u64,
                        ))
                        .await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
    }

    /// 同步单个提供商的模型
    pub async fn sync_provider(&self, provider: &str) -> Result<SyncResult> {
        info!("Syncing models for provider: {}", provider);

        let result = match provider {
            "openai" => self.sync_openai().await?,
            "anthropic" => self.sync_anthropic().await?,
            "google" => self.sync_google().await?,
            "deepseek" => self.sync_deepseek().await?,
            "mistral" => self.sync_mistral().await?,
            "cohere" => self.sync_cohere().await?,
            _ => bail!("Unknown provider: {}", provider),
        };

        // 更新提供商状态
        let mut state = self.sync_state.write().await;
        state.provider_status.insert(
            provider.to_string(),
            ProviderSyncStatus {
                provider: provider.to_string(),
                last_sync: Some(Utc::now()),
                models_count: result.new_models.len() + result.updated_models.len(),
                last_error: if result.errors.is_empty() {
                    None
                } else {
                    Some(result.errors.join(", "))
                },
            },
        );

        Ok(result)
    }

    /// 从 OpenAI 同步模型
    async fn sync_openai(&self) -> Result<SyncResult> {
        debug!("Fetching OpenAI models");
        
        let api_key = self.get_api_key("openai").await
            .context("OpenAI API key not configured")?;

        // 调用 OpenAI API 获取模型列表
        let url = "https://api.openai.com/v1/models";
        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to call OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("OpenAI API error: {} - {}", status, body);
        }

        let models: OpenAIModelsResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        debug!("OpenAI returned {} models", models.data.len());

        // 过滤出聊天模型
        let chat_models: Vec<&OpenAIModel> = models
            .data
            .iter()
            .filter(|m| {
                m.id.starts_with("gpt-")
                    || m.id.starts_with("chatgpt-")
                    || m.id.starts_with("o1-")
                    || m.id.starts_with("o3-")
            })
            .collect();

        info!("Found {} OpenAI chat models", chat_models.len());

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "openai")
            .map(|m| (m.name.clone(), m))
            .collect();

        // 检测新模型和更新模型
        let mut new_models = Vec::new();
        let mut updated_models = Vec::new();
        let mut price_changes = Vec::new();
        let mut errors = Vec::new();

        for model in &chat_models {
            let model_name = &model.id;

            if let Some(existing) = existing_map.get(model_name) {
                // 检查是否需要更新
                if model_name.starts_with("gpt-4o") || model_name.contains("-preview") {
                    updated_models.push(model_name.clone());
                    // 更新模型信息
                    if let Err(e) = self.update_model_info(existing, model).await {
                        warn!("Failed to update model {}: {}", model_name, e);
                        errors.push(format!("Update failed for {}: {}", model_name, e));
                    }
                }
            } else {
                // 新模型
                new_models.push(model_name.clone());
                let default_config = self.get_openai_model_config(model_name)?;
                match self.model_registry.create(default_config).await {
                    Ok(_) => info!("Added new OpenAI model: {}", model_name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_name, e);
                        errors.push(format!("Failed to add {}: {}", model_name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models("openai", &chat_models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>()).await?;

        Ok(SyncResult {
            provider: "openai".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes,
            errors,
        })
    }

    /// 从 Anthropic 同步模型（基于已知模型列表）
    async fn sync_anthropic() -> Result<SyncResult> {
        debug!("Syncing Anthropic models");
        
        // Anthropic 没有公开的模型列表 API，使用已知模型
        let known_models = vec![
            AnthropicModelInfo {
                name: "claude-opus-4-20250514".into(),
                display_name: "Claude Opus 4".into(),
                input_price: 15.0,
                output_price: 75.0,
                context_window: 200000,
                max_tokens: 4096,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-sonnet-4-20250514".into(),
                display_name: "Claude Sonnet 4".into(),
                input_price: 3.0,
                output_price: 15.0,
                context_window: 200000,
                max_tokens: 8192,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-3-5-sonnet-20241022".into(),
                display_name: "Claude 3.5 Sonnet".into(),
                input_price: 3.0,
                output_price: 15.0,
                context_window: 200000,
                max_tokens: 8192,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-3-5-haiku-20241022".into(),
                display_name: "Claude 3.5 Haiku".into(),
                input_price: 0.8,
                output_price: 4.0,
                context_window: 200000,
                max_tokens: 8192,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-3-opus-20240229".into(),
                display_name: "Claude 3 Opus".into(),
                input_price: 15.0,
                output_price: 75.0,
                context_window: 200000,
                max_tokens: 4096,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-3-sonnet-20240229".into(),
                display_name: "Claude 3 Sonnet".into(),
                input_price: 3.0,
                output_price: 15.0,
                context_window: 200000,
                max_tokens: 4096,
                supports_vision: true,
            },
            AnthropicModelInfo {
                name: "claude-3-haiku-20240307".into(),
                display_name: "Claude 3 Haiku".into(),
                input_price: 0.25,
                output_price: 1.25,
                context_window: 200000,
                max_tokens: 4096,
                supports_vision: true,
            },
        ];

        let mut new_models = vec![];
        let mut updated_models = vec![];
        let mut price_changes = vec![];
        let mut errors = vec![];

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "anthropic")
            .map(|m| (m.name.clone(), m))
            .collect();

        for model_info in &known_models {
            if let Some(existing) = existing_map.get(&model_info.name) {
                // 检查价格变化
                if let Some(price_change) = self.detect_price_change(
                    &model_info.name,
                    existing.input_price,
                    model_info.input_price,
                    existing.output_price,
                    model_info.output_price,
                ) {
                    price_changes.push(price_change);
                    updated_models.push(model_info.name.clone());
                    
                    // 更新模型价格
                    if let Err(e) = self.update_model_prices(existing, model_info).await {
                        warn!("Failed to update Anthropic model {}: {}", model_info.name, e);
                        errors.push(format!("Update failed for {}: {}", model_info.name, e));
                    }
                }
            } else {
                // 新模型
                new_models.push(model_info.name.clone());
                let req = CreateModelRequest {
                    name: model_info.name.clone(),
                    aliases: vec![],
                    provider: "anthropic".into(),
                    api_name: None,
                    display_name: Some(model_info.display_name.clone()),
                    input_price: model_info.input_price,
                    output_price: model_info.output_price,
                    max_tokens: model_info.max_tokens,
                    context_window: model_info.context_window,
                    max_concurrent: 5,
                    fallback_models: vec![],
                    capabilities: Default::default(),
                    supports_streaming: true,
                    supports_function_calling: true,
                    supports_vision: model_info.supports_vision,
                    enabled: true,
                    priority: 100,
                };

                match self.model_registry.create(req).await {
                    Ok(_) => info!("Added new Anthropic model: {}", model_info.name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_info.name, e);
                        errors.push(format!("Failed to add {}: {}", model_info.name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models(
            "anthropic",
            &known_models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ).await?;

        Ok(SyncResult {
            provider: "anthropic".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes,
            errors,
        })
    }

    /// 从 Google/Gemini 同步模型
    async fn sync_google(&self) -> Result<SyncResult> {
        debug!("Syncing Google/Gemini models");
        
        let api_key = self.get_api_key("google").await
            .context("Google API key not configured")?;

        // 调用 Gemini API
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            api_key
        );
        
        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to call Gemini API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Gemini API error: {} - {}", status, body);
        }

        let models: GeminiModelsResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        debug!("Gemini returned {} models", models.models.len());

        // 过滤生成模型
        let gemini_models: Vec<&GeminiModel> = models
            .models
            .iter()
            .filter(|m| {
                m.name.contains("gemini") && !m.name.contains("embedding")
            })
            .collect();

        info!("Found {} Gemini models", gemini_models.len());

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "google")
            .map(|m| (m.name.clone(), m))
            .collect();

        let mut new_models = Vec::new();
        let mut updated_models = Vec::new();
        let mut errors = Vec::new();

        for model in &gemini_models {
            // 提取模型名称（去掉 "models/" 前缀）
            let model_name = model.name.strip_prefix("models/").unwrap_or(&model.name);

            if existing_map.contains_key(model_name) {
                // 检查更新
                updated_models.push(model_name.to_string());
            } else {
                // 新模型
                new_models.push(model_name.to_string());
                let default_config = self.get_gemini_model_config(model)?;
                match self.model_registry.create(default_config).await {
                    Ok(_) => info!("Added new Gemini model: {}", model_name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_name, e);
                        errors.push(format!("Failed to add {}: {}", model_name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models(
            "google",
            &gemini_models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ).await?;

        Ok(SyncResult {
            provider: "google".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes: vec![],
            errors,
        })
    }

    /// 从 DeepSeek 同步模型
    async fn sync_deepseek(&self) -> Result<SyncResult> {
        debug!("Syncing DeepSeek models");
        
        let api_key = self.get_api_key("deepseek").await
            .context("DeepSeek API key not configured")?;

        // DeepSeek 模型列表
        let url = "https://api.deepseek.com/v1/models";
        
        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to call DeepSeek API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("DeepSeek API error: {} - {}", status, body);
        }

        // DeepSeek 可能返回类似 OpenAI 的格式
        let known_models = vec![
            DeepSeekModelInfo {
                name: "deepseek-chat".into(),
                display_name: "DeepSeek Chat".into(),
                input_price: 0.14,
                output_price: 0.28,
                context_window: 64000,
                max_tokens: 4096,
            },
            DeepSeekModelInfo {
                name: "deepseek-reasoner".into(),
                display_name: "DeepSeek Reasoner (R1)".into(),
                input_price: 0.55,
                output_price: 2.19,
                context_window: 64000,
                max_tokens: 8192,
            },
        ];

        let mut new_models = Vec::new();
        let mut updated_models = Vec::new();
        let mut price_changes = Vec::new();
        let mut errors = Vec::new();

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "deepseek")
            .map(|m| (m.name.clone(), m))
            .collect();

        for model_info in &known_models {
            if let Some(existing) = existing_map.get(&model_info.name) {
                // 检查价格变化
                if let Some(price_change) = self.detect_price_change(
                    &model_info.name,
                    existing.input_price,
                    model_info.input_price,
                    existing.output_price,
                    model_info.output_price,
                ) {
                    price_changes.push(price_change);
                    updated_models.push(model_info.name.clone());
                }
            } else {
                // 新模型
                new_models.push(model_info.name.clone());
                let req = CreateModelRequest {
                    name: model_info.name.clone(),
                    aliases: vec![],
                    provider: "deepseek".into(),
                    api_name: None,
                    display_name: Some(model_info.display_name.clone()),
                    input_price: model_info.input_price,
                    output_price: model_info.output_price,
                    max_tokens: model_info.max_tokens,
                    context_window: model_info.context_window,
                    max_concurrent: 5,
                    fallback_models: vec![],
                    capabilities: Default::default(),
                    supports_streaming: true,
                    supports_function_calling: false,
                    supports_vision: false,
                    enabled: true,
                    priority: 100,
                };

                match self.model_registry.create(req).await {
                    Ok(_) => info!("Added new DeepSeek model: {}", model_info.name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_info.name, e);
                        errors.push(format!("Failed to add {}: {}", model_info.name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models(
            "deepseek",
            &known_models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ).await?;

        Ok(SyncResult {
            provider: "deepseek".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes,
            errors,
        })
    }

    /// 从 Mistral 同步模型
    async fn sync_mistral(&self) -> Result<SyncResult> {
        debug!("Syncing Mistral models");
        
        let api_key = self.get_api_key("mistral").await
            .context("Mistral API key not configured")?;

        // 调用 Mistral API
        let url = "https://api.mistral.ai/v1/models";
        
        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to call Mistral API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Mistral API error: {} - {}", status, body);
        }

        let models: MistralModelsResponse = response
            .json()
            .await
            .context("Failed to parse Mistral response")?;

        debug!("Mistral returned {} models", models.data.len());

        // 已知模型价格信息
        let price_map = HashMap::from([
            ("mistral-large-latest", (2.0, 6.0, 128000)),
            ("mistral-medium-latest", (0.7, 2.1, 32000)),
            ("mistral-small-latest", (0.2, 0.6, 32000)),
            ("open-mistral-nemo", (0.15, 0.15, 128000)),
            ("codestral-latest", (0.3, 0.9, 32000)),
        ]);

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "mistral")
            .map(|m| (m.name.clone(), m))
            .collect();

        let mut new_models = Vec::new();
        let mut updated_models = Vec::new();
        let mut price_changes = Vec::new();
        let mut errors = Vec::new();

        for model in &models.data {
            let model_name = &model.id;
            let (input_price, output_price, context_window) = price_map
                .get(model_name.as_str())
                .copied()
                .unwrap_or((0.0, 0.0, 32000));

            if let Some(existing) = existing_map.get(model_name) {
                // 检查价格变化
                if let Some(price_change) = self.detect_price_change(
                    model_name,
                    existing.input_price,
                    input_price,
                    existing.output_price,
                    output_price,
                ) {
                    price_changes.push(price_change);
                    updated_models.push(model_name.clone());
                }
            } else {
                // 新模型
                new_models.push(model_name.clone());
                let req = CreateModelRequest {
                    name: model_name.clone(),
                    aliases: vec![],
                    provider: "mistral".into(),
                    api_name: None,
                    display_name: Some(model_name.clone()),
                    input_price,
                    output_price,
                    max_tokens: 4096,
                    context_window,
                    max_concurrent: 5,
                    fallback_models: vec![],
                    capabilities: Default::default(),
                    supports_streaming: true,
                    supports_function_calling: true,
                    supports_vision: false,
                    enabled: true,
                    priority: 100,
                };

                match self.model_registry.create(req).await {
                    Ok(_) => info!("Added new Mistral model: {}", model_name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_name, e);
                        errors.push(format!("Failed to add {}: {}", model_name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models(
            "mistral",
            &models.data.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ).await?;

        Ok(SyncResult {
            provider: "mistral".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes,
            errors,
        })
    }

    /// 从 Cohere 同步模型
    async fn sync_cohere(&self) -> Result<SyncResult> {
        debug!("Syncing Cohere models");
        
        let api_key = self.get_api_key("cohere").await
            .context("Cohere API key not configured")?;

        // 调用 Cohere API
        let url = "https://api.cohere.ai/v1/models";
        
        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("Failed to call Cohere API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Cohere API error: {} - {}", status, body);
        }

        let models: CohereModelsResponse = response
            .json()
            .await
            .context("Failed to parse Cohere response")?;

        debug!("Cohere returned {} models", models.models.len());

        // 已知模型价格信息
        let price_map = HashMap::from([
            ("command-r-plus", (2.5, 10.0, 128000)),
            ("command-r", (0.5, 1.5, 128000)),
            ("command", (1.0, 2.0, 4096)),
            ("command-light", (0.5, 1.0, 4096)),
        ]);

        // 获取现有模型
        let existing_models = self.model_registry.list_models().await?;
        let existing_map: HashMap<String, &RuntimeModelConfig> = existing_models
            .iter()
            .filter(|m| m.provider == "cohere")
            .map(|m| (m.name.clone(), m))
            .collect();

        let mut new_models = Vec::new();
        let mut updated_models = Vec::new();
        let mut price_changes = Vec::new();
        let mut errors = Vec::new();

        for model in &models.models {
            let model_name = &model.name;
            let (input_price, output_price, context_window) = price_map
                .get(model_name.as_str())
                .copied()
                .unwrap_or((0.0, 0.0, 4096));

            if let Some(existing) = existing_map.get(model_name) {
                // 检查价格变化
                if let Some(price_change) = self.detect_price_change(
                    model_name,
                    existing.input_price,
                    input_price,
                    existing.output_price,
                    output_price,
                ) {
                    price_changes.push(price_change);
                    updated_models.push(model_name.clone());
                }
            } else {
                // 新模型
                new_models.push(model_name.clone());
                let req = CreateModelRequest {
                    name: model_name.clone(),
                    aliases: vec![],
                    provider: "cohere".into(),
                    api_name: None,
                    display_name: model.deployment_label.clone().or(Some(model_name.clone())),
                    input_price,
                    output_price,
                    max_tokens: 4096,
                    context_window,
                    max_concurrent: 5,
                    fallback_models: vec![],
                    capabilities: Default::default(),
                    supports_streaming: true,
                    supports_function_calling: true,
                    supports_vision: false,
                    enabled: true,
                    priority: 100,
                };

                match self.model_registry.create(req).await {
                    Ok(_) => info!("Added new Cohere model: {}", model_name),
                    Err(e) => {
                        warn!("Failed to add model {}: {}", model_name, e);
                        errors.push(format!("Failed to add {}: {}", model_name, e));
                    }
                }
            }
        }

        // 检测废弃模型
        let deprecated_models = self.detect_deprecated_models(
            "cohere",
            &models.models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ).await?;

        Ok(SyncResult {
            provider: "cohere".to_string(),
            new_models,
            updated_models,
            deprecated_models,
            price_changes,
            errors,
        })
    }

    /// 检测价格变化
    fn detect_price_change(
        &self,
        model_name: &str,
        old_input: f64,
        new_input: f64,
        old_output: f64,
        new_output: f64,
    ) -> Option<PriceChange> {
        // 价格变化超过 1% 则认为有变化
        let threshold = 0.01;
        
        let input_changed = (old_input - new_input).abs() > old_input * threshold || old_input == 0.0 && new_input > 0.0;
        let output_changed = (old_output - new_output).abs() > old_output * threshold || old_output == 0.0 && new_output > 0.0;

        if input_changed || output_changed {
            info!(
                "Price change detected for {}: input {:.2} -> {:.2}, output {:.2} -> {:.2}",
                model_name, old_input, new_input, old_output, new_output
            );
            Some(PriceChange {
                model_name: model_name.to_string(),
                old_input_price: old_input,
                new_input_price: new_input,
                old_output_price: old_output,
                new_output_price: new_output,
                change_time: Utc::now(),
            })
        } else {
            None
        }
    }

    /// 检测废弃模型
    async fn detect_deprecated_models(
        &self,
        provider: &str,
        current_models: &[&str],
    ) -> Result<Vec<String>> {
        let existing_models = self.model_registry.list_models().await?;
        let current_set: std::collections::HashSet<&str> = current_models.iter().cloned().collect();

        let deprecated: Vec<String> = existing_models
            .iter()
            .filter(|m| m.provider == provider && !current_set.contains(m.name.as_str()))
            .map(|m| m.name.clone())
            .collect();

        // 标记废弃模型
        for model_name in &deprecated {
            warn!("Model {} appears to be deprecated", model_name);
            // 可以选择禁用模型而不是删除
            // self.model_registry.disable(model_name).await?;
        }

        Ok(deprecated)
    }

    /// 更新模型信息（OpenAI）
    async fn update_model_info(
        &self,
        existing: &RuntimeModelConfig,
        _model: &OpenAIModel,
    ) -> Result<()> {
        debug!("Updating model: {}", existing.name);
        // 可以更新别名、优先级等
        Ok(())
    }

    /// 更新模型价格（Anthropic）
    async fn update_model_prices(
        &self,
        existing: &RuntimeModelConfig,
        model_info: &AnthropicModelInfo,
    ) -> Result<()> {
        debug!(
            "Updating prices for {}: input {:.2} -> {:.2}, output {:.2} -> {:.2}",
            existing.name, existing.input_price, model_info.input_price, 
            existing.output_price, model_info.output_price
        );
        
        // 这里应该调用 model_registry.update() 方法
        // 暂时只记录日志
        Ok(())
    }

    /// 获取 OpenAI 模型配置
    fn get_openai_model_config(&self, model_name: &str) -> Result<CreateModelRequest> {
        // OpenAI 模型价格映射
        let (input_price, output_price, context_window, supports_vision) = 
            if model_name.starts_with("gpt-4o") {
                (2.5, 10.0, 128000, true)
            } else if model_name.starts_with("gpt-4-turbo") {
                (10.0, 30.0, 128000, true)
            } else if model_name.starts_with("gpt-4-32k") {
                (60.0, 120.0, 32768, false)
            } else if model_name.starts_with("gpt-4") {
                (30.0, 60.0, 8192, false)
            } else if model_name.starts_with("gpt-3.5-turbo") {
                (0.5, 1.5, 16385, false)
            } else if model_name.starts_with("o1") || model_name.starts_with("o3") {
                (15.0, 60.0, 200000, false)
            } else {
                (5.0, 15.0, 8192, false)
            };

        Ok(CreateModelRequest {
            name: model_name.to_string(),
            aliases: vec![],
            provider: "openai".to_string(),
            api_name: None,
            display_name: Some(model_name.to_string()),
            input_price,
            output_price,
            max_tokens: 4096,
            context_window,
            max_concurrent: 5,
            fallback_models: vec![],
            capabilities: Default::default(),
            supports_streaming: true,
            supports_function_calling: true,
            supports_vision,
            enabled: true,
            priority: 100,
        })
    }

    /// 获取 Gemini 模型配置
    fn get_gemini_model_config(&self, model: &GeminiModel) -> Result<CreateModelRequest> {
        let model_name = model.name.strip_prefix("models/").unwrap_or(&model.name);
        
        // Gemini 模型价格映射
        let (input_price, output_price, context_window) = 
            if model_name.contains("gemini-2.0") {
                (0.0, 0.0, 1048576) // Gemini 2.0 Flash is free
            } else if model_name.contains("gemini-1.5-pro") {
                (1.25, 5.0, 2097152)
            } else if model_name.contains("gemini-1.5-flash") {
                (0.075, 0.3, 1048576)
            } else {
                (0.5, 1.5, 32768)
            };

        Ok(CreateModelRequest {
            name: model_name.to_string(),
            aliases: vec![],
            provider: "google".to_string(),
            api_name: None,
            display_name: model.display_name.clone().or(Some(model_name.to_string())),
            input_price,
            output_price,
            max_tokens: model.output_token_limit.unwrap_or(8192),
            context_window: model.input_token_limit.unwrap_or(32000),
            max_concurrent: 5,
            fallback_models: vec![],
            capabilities: Default::default(),
            supports_streaming: true,
            supports_function_calling: true,
            supports_vision: true,
            enabled: true,
            priority: 100,
        })
    }

    /// 获取默认模型配置
    fn get_default_model_config(&self, provider: &str, model_name: &str) -> Result<CreateModelRequest> {
        Ok(CreateModelRequest {
            name: model_name.to_string(),
            aliases: vec![],
            provider: provider.to_string(),
            api_name: None,
            display_name: Some(model_name.to_string()),
            input_price: 0.0,
            output_price: 0.0,
            max_tokens: 4096,
            context_window: 128000,
            max_concurrent: 5,
            fallback_models: vec![],
            capabilities: Default::default(),
            supports_streaming: true,
            supports_function_calling: false,
            supports_vision: false,
            enabled: true,
            priority: 100,
        })
    }

    /// 获取同步状态
    pub async fn get_sync_state(&self) -> SyncState {
        self.sync_state.read().await.clone()
    }

    /// 启动定时同步任务
    pub async fn start_periodic_sync(self: Arc<Self>, interval_hours: u64) {
        info!("Starting periodic model sync every {} hours", interval_hours);
        
        tokio::spawn(async move {
            let interval = std::time::Duration::from_secs(interval_hours * 3600);
            let mut consecutive_failures = 0;
            const MAX_CONSECUTIVE_FAILURES: u32 = 3;

            loop {
                tokio::time::sleep(interval).await;

                info!("Starting scheduled model sync");
                match self.sync_all().await {
                    Ok(results) => {
                        consecutive_failures = 0;
                        
                        let total_new: usize = results.iter().map(|r| r.new_models.len()).sum();
                        let total_updated: usize = results.iter().map(|r| r.updated_models.len()).sum();
                        let total_deprecated: usize = results.iter().map(|r| r.deprecated_models.len()).sum();
                        let total_errors: usize = results.iter().map(|r| r.errors.len()).sum();

                        info!(
                            "Scheduled sync completed: {} new, {} updated, {} deprecated, {} errors",
                            total_new, total_updated, total_deprecated, total_errors
                        );

                        // 如果有价格变化，可以发送通知
                        let price_changes: Vec<_> = results
                            .iter()
                            .flat_map(|r| r.price_changes.clone())
                            .collect();
                        
                        if !price_changes.is_empty() {
                            warn!("Price changes detected: {:?}", price_changes);
                            // TODO: 发送通知到指定渠道
                        }
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        error!(
                            "Scheduled model sync failed (attempt {}): {}",
                            consecutive_failures, e
                        );

                        if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                            error!(
                                "Model sync has failed {} times consecutively. Please check configuration.",
                                consecutive_failures
                            );
                            // 可以发送告警通知
                        }
                    }
                }
            }
        });
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            last_sync: None,
            last_success: None,
            last_error: None,
            in_progress: false,
            provider_status: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::model_configs::ModelCapabilities;
    use sea_orm::{Database, DatabaseConnection};

    /// 创建内存数据库用于测试
    async fn create_test_db() -> DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database")
    }

    #[test]
    fn test_sync_state_default() {
        let state = SyncState::default();
        assert!(!state.in_progress);
        assert!(state.last_sync.is_none());
        assert!(state.last_success.is_none());
        assert!(state.last_error.is_none());
        assert!(state.provider_status.is_empty());
    }

    #[test]
    fn test_provider_sync_status() {
        let status = ProviderSyncStatus {
            provider: "openai".to_string(),
            last_sync: Some(Utc::now()),
            models_count: 10,
            last_error: None,
        };
        
        assert_eq!(status.provider, "openai");
        assert_eq!(status.models_count, 10);
        assert!(status.last_sync.is_some());
    }

    #[test]
    fn test_sync_result_serialization() {
        let result = SyncResult {
            provider: "anthropic".to_string(),
            new_models: vec!["claude-4".to_string()],
            updated_models: vec!["claude-3.5-sonnet".to_string()],
            deprecated_models: vec![],
            price_changes: vec![PriceChange {
                model_name: "claude-3-opus".to_string(),
                old_input_price: 15.0,
                new_input_price: 12.0,
                old_output_price: 75.0,
                new_output_price: 60.0,
                change_time: Utc::now(),
            }],
            errors: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SyncResult = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.provider, "anthropic");
        assert_eq!(deserialized.new_models.len(), 1);
        assert_eq!(deserialized.price_changes.len(), 1);
    }

    #[test]
    fn test_price_change_detection() {
        let service = ModelSyncService::new(
            create_test_db().block_on().unwrap(),
            Arc::new(ModelRegistry::new(create_test_db().block_on().unwrap())),
        );

        // 无变化
        let change = service.detect_price_change("test-model", 10.0, 10.0, 20.0, 20.0);
        assert!(change.is_none());

        // 有变化（超过阈值）
        let change = service.detect_price_change("test-model", 10.0, 12.0, 20.0, 20.0);
        assert!(change.is_some());
        let change = change.unwrap();
        assert_eq!(change.old_input_price, 10.0);
        assert_eq!(change.new_input_price, 12.0);

        // 从 0 到有价格
        let change = service.detect_price_change("test-model", 0.0, 10.0, 0.0, 20.0);
        assert!(change.is_some());
    }

    #[test]
    fn test_openai_model_config() {
        let service = ModelSyncService::new(
            create_test_db().block_on().unwrap(),
            Arc::new(ModelRegistry::new(create_test_db().block_on().unwrap())),
        );

        // GPT-4o
        let config = service.get_openai_model_config("gpt-4o-2024-05-13").unwrap();
        assert_eq!(config.input_price, 2.5);
        assert_eq!(config.output_price, 10.0);
        assert!(config.supports_vision);

        // GPT-3.5-turbo
        let config = service.get_openai_model_config("gpt-3.5-turbo-0125").unwrap();
        assert_eq!(config.input_price, 0.5);
        assert_eq!(config.output_price, 1.5);
        assert!(!config.supports_vision);

        // O1
        let config = service.get_openai_model_config("o1-preview").unwrap();
        assert_eq!(config.input_price, 15.0);
        assert_eq!(config.context_window, 200000);
    }

    #[test]
    fn test_anthropic_model_info() {
        let models = vec![
            AnthropicModelInfo {
                name: "claude-3-5-sonnet-20241022".into(),
                display_name: "Claude 3.5 Sonnet".into(),
                input_price: 3.0,
                output_price: 15.0,
                context_window: 200000,
                max_tokens: 8192,
                supports_vision: true,
            },
        ];

        assert_eq!(models[0].name, "claude-3-5-sonnet-20241022");
        assert_eq!(models[0].input_price, 3.0);
        assert!(models[0].supports_vision);
    }

    #[test]
    fn test_mistral_price_map() {
        let price_map = HashMap::from([
            ("mistral-large-latest", (2.0, 6.0, 128000)),
            ("mistral-medium-latest", (0.7, 2.1, 32000)),
        ]);

        let (input, output, context) = price_map.get("mistral-large-latest").unwrap();
        assert_eq!(*input, 2.0);
        assert_eq!(*output, 6.0);
        assert_eq!(*context, 128000);
    }

    #[test]
    fn test_cohere_price_map() {
        let price_map = HashMap::from([
            ("command-r-plus", (2.5, 10.0, 128000)),
            ("command-r", (0.5, 1.5, 128000)),
        ]);

        let (input, output, context) = price_map.get("command-r-plus").unwrap();
        assert_eq!(*input, 2.5);
        assert_eq!(*output, 10.0);
        assert_eq!(*context, 128000);
    }

    #[test]
    fn test_deepseek_model_info() {
        let models = vec![
            DeepSeekModelInfo {
                name: "deepseek-chat".into(),
                display_name: "DeepSeek Chat".into(),
                input_price: 0.14,
                output_price: 0.28,
                context_window: 64000,
                max_tokens: 4096,
            },
        ];

        assert_eq!(models[0].name, "deepseek-chat");
        assert_eq!(models[0].input_price, 0.14);
        assert_eq!(models[0].context_window, 64000);
    }

    #[tokio::test]
    async fn test_api_key_management() {
        let service = ModelSyncService::new(
            create_test_db().await,
            Arc::new(ModelRegistry::new(create_test_db().await)),
        );

        // 设置 API 密钥
        service.set_api_key("openai", "sk-test-key".to_string()).await;

        // 获取 API 密钥
        let key = service.get_api_key("openai").await.unwrap();
        assert_eq!(key, "sk-test-key");

        // 未设置的提供商应该报错
        let result = service.get_api_key("unknown").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_sync_prevention() {
        let service = Arc::new(ModelSyncService::new(
            create_test_db().await,
            Arc::new(ModelRegistry::new(create_test_db().await)),
        ));

        // 标记为进行中
        {
            let mut state = service.sync_state.write().await;
            state.in_progress = true;
        }

        // 尝试同步应该失败
        let result = service.sync_all().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already in progress"));
    }
}
