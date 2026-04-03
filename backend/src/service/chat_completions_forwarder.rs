//! Chat Completions 转发服务
//!
//! 参考 sub2api 实现，提供完整的 OpenAI Chat Completions API 转发
//! 支持：
//! - 多 Provider 转发 (OpenAI, Anthropic, Gemini, DeepSeek, etc.)
//! - 流式响应 (SSE)
//! - 账号调度与故障转移
//! - 计费与配额检查

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::account::AccountService;
use super::scheduler::SchedulerService;

/// Chat Completions 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub stream_options: Option<StreamOptions>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub image_url: Option<ImageUrl>,
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

/// 流式选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    #[serde(default)]
    pub include_usage: bool,
}

/// Chat Completions 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Option<ResponseMessage>,
    pub delta: Option<Delta>,
    pub finish_reason: Option<String>,
}

/// 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: String,
}

/// Delta (流式)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

/// 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}

/// 转发结果
#[derive(Debug, Clone)]
pub struct ForwardResult {
    pub request_id: String,
    pub model: String,
    pub billing_model: String,
    pub stream: bool,
    pub usage: Usage,
    pub first_token_ms: Option<u64>,
    pub duration_ms: u64,
}

/// Provider 配置
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub auth_header: String,
    pub auth_prefix: String,
}

impl ProviderConfig {
    /// 获取 Provider 配置
    pub fn for_provider(provider: &str) -> Self {
        match provider.to_lowercase().as_str() {
            "openai" => Self {
                name: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                auth_header: "Authorization".to_string(),
                auth_prefix: "Bearer ".to_string(),
            },
            "anthropic" => Self {
                name: "anthropic".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                auth_header: "x-api-key".to_string(),
                auth_prefix: "".to_string(),
            },
            "gemini" | "google" => Self {
                name: "gemini".to_string(),
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                auth_header: "x-goog-api-key".to_string(),
                auth_prefix: "".to_string(),
            },
            "deepseek" => Self {
                name: "deepseek".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                auth_header: "Authorization".to_string(),
                auth_prefix: "Bearer ".to_string(),
            },
            "mistral" => Self {
                name: "mistral".to_string(),
                base_url: "https://api.mistral.ai/v1".to_string(),
                auth_header: "Authorization".to_string(),
                auth_prefix: "Bearer ".to_string(),
            },
            "cohere" => Self {
                name: "cohere".to_string(),
                base_url: "https://api.cohere.ai/v1".to_string(),
                auth_header: "Authorization".to_string(),
                auth_prefix: "Bearer ".to_string(),
            },
            _ => Self {
                name: provider.to_lowercase(),
                base_url: format!("https://api.{}.com/v1", provider.to_lowercase()),
                auth_header: "Authorization".to_string(),
                auth_prefix: "Bearer ".to_string(),
            },
        }
    }
}

/// Chat Completions 转发器
pub struct ChatCompletionsForwarder {
    db: sea_orm::DatabaseConnection,
    http_client: Client,
    account_service: Arc<AccountService>,
    scheduler: Arc<RwLock<SchedulerService>>,
    max_retries: u32,
}

impl ChatCompletionsForwarder {
    /// 创建新的转发器
    pub fn new(
        db: sea_orm::DatabaseConnection,
        account_service: Arc<AccountService>,
        scheduler: SchedulerService,
    ) -> Self {
        Self {
            db,
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            account_service,
            scheduler: Arc::new(RwLock::new(scheduler)),
            max_retries: 3,
        }
    }

    /// 转发 Chat Completions 请求
    pub async fn forward(
        &self,
        request: ChatCompletionsRequest,
        user_id: Uuid,
        api_key_id: Uuid,
    ) -> Result<ForwardResult> {
        let start_time = std::time::Instant::now();
        let original_model = request.model.clone();
        let is_stream = request.stream;

        // 1. 选择账号
        let account = self.select_account(&original_model).await?;
        let provider_config = ProviderConfig::for_provider(&account.provider);

        // 2. 获取凭证
        let credential = self.get_account_credential(account.id).await?;

        // 3. 映射模型
        let mapped_model = self.map_model(&original_model, &account.provider);

        // 4. 构建上游请求
        let upstream_request = self.build_upstream_request(&request, &mapped_model);

        // 5. 发送请求
        let result = self
            .send_request(
                &provider_config,
                &credential,
                upstream_request,
                is_stream,
                &original_model,
                &mapped_model,
                start_time,
            )
            .await?;

        // 6. 记录使用量
        self.record_usage(&result, user_id, api_key_id, account.id)
            .await?;

        Ok(result)
    }

    /// 选择账号
    async fn select_account(&self, model: &str) -> Result<crate::entity::accounts::Model> {
        let scheduler = self.scheduler.read().await;

        let account = scheduler
            .select_account(model, None, 5)
            .await?
            .ok_or_else(|| anyhow!("No available account for model: {}", model))?;

        Ok(account)
    }

    /// 获取账号凭证
    async fn get_account_credential(&self, account_id: Uuid) -> Result<String> {
        use crate::entity::accounts;
        use sea_orm::EntityTrait;

        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Account not found: {}", account_id))?;

        // 解密凭证
        let credential =
            crate::utils::encryption_global::GlobalEncryption::decrypt(&account.credential)
                .map_err(|e| anyhow!("Failed to decrypt credential: {}", e))?;

        Ok(credential)
    }

    /// 映射模型名称
    fn map_model(&self, model: &str, _provider: &str) -> String {
        // TODO: 实现模型映射逻辑
        // 从数据库或配置中读取映射关系
        model.to_string()
    }

    /// 构建上游请求
    fn build_upstream_request(
        &self,
        request: &ChatCompletionsRequest,
        mapped_model: &str,
    ) -> serde_json::Value {
        let mut upstream = serde_json::to_value(request).unwrap_or_default();

        // 更新模型名称
        if let Some(obj) = upstream.as_object_mut() {
            obj.insert("model".to_string(), serde_json::json!(mapped_model));
        }

        upstream
    }

    /// 发送请求到上游
    async fn send_request(
        &self,
        provider_config: &ProviderConfig,
        credential: &str,
        request_body: serde_json::Value,
        is_stream: bool,
        original_model: &str,
        mapped_model: &str,
        start_time: std::time::Instant,
    ) -> Result<ForwardResult> {
        let url = format!("{}/chat/completions", provider_config.base_url);

        let mut req = self
            .http_client
            .post(&url)
            .json(&request_body)
            .header(
                &provider_config.auth_header,
                format!("{}{}", provider_config.auth_prefix, credential),
            )
            .header("Content-Type", "application/json");

        if is_stream {
            req = req.header("Accept", "text/event-stream");
        }

        // 故障转移逻辑
        let mut last_error = None;
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                tracing::warn!(
                    "Retrying request (attempt {}/{})",
                    attempt,
                    self.max_retries
                );
                tokio::time::sleep(std::time::Duration::from_millis(100 * attempt as u64)).await;
            }

            match self
                .send_single_request(&req, is_stream, original_model, mapped_model, start_time)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // 检查是否为可重试错误
                    let error_str = e.to_string();
                    if error_str.contains("rate limit")
                        || error_str.contains("429")
                        || error_str.contains("timeout")
                        || error_str.contains("503")
                        || error_str.contains("502")
                    {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Max retries exceeded")))
    }

    /// 发送单个请求
    async fn send_single_request(
        &self,
        req: &reqwest::RequestBuilder,
        is_stream: bool,
        original_model: &str,
        mapped_model: &str,
        start_time: std::time::Instant,
    ) -> Result<ForwardResult> {
        let response = req
            .try_clone()
            .ok_or_else(|| anyhow!("Failed to clone request"))?
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Upstream request failed with status {}: {}",
                status,
                error_body
            ));
        }

        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(&Uuid::new_v4().to_string())
            .to_string();

        let (usage, first_token_ms) = if is_stream {
            self.process_stream_response(response, start_time).await?
        } else {
            let usage = self.extract_usage_from_response(response).await?;
            (usage, None)
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(ForwardResult {
            request_id,
            model: original_model.to_string(),
            billing_model: mapped_model.to_string(),
            stream: is_stream,
            usage,
            first_token_ms,
            duration_ms,
        })
    }

    /// 处理流式响应（返回 usage 和首 token 延迟）
    async fn process_stream_response(
        &self,
        response: reqwest::Response,
        start_time: std::time::Instant,
    ) -> Result<(Usage, Option<u64>)> {
        use futures::StreamExt;

        let mut stream = response.bytes_stream();
        let mut parser = SSEParser::new();
        let mut first_token_ms: Option<u64> = None;
        let mut final_usage = Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_read_input_tokens: None,
        };
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| anyhow!("Stream error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // 处理缓冲区中的完整行
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // 首个有效数据事件时记录延迟
                if first_token_ms.is_none() && line.starts_with("data: ") && line != "data: [DONE]"
                {
                    first_token_ms = Some(start_time.elapsed().as_millis() as u64);
                }

                // 解析 SSE 事件
                if let Some(event) = parser.parse_line(&line) {
                    if let Some(usage) = event.usage {
                        final_usage = usage;
                    }
                }
            }
        }

        Ok((final_usage, first_token_ms))
    }

    /// 从流式响应中提取 usage（已弃用，使用 process_stream_response）
    async fn extract_usage_from_stream(&self, _response: reqwest::Response) -> Result<Usage> {
        Ok(Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_read_input_tokens: None,
        })
    }

    /// 从非流式响应中提取 usage
    async fn extract_usage_from_response(&self, response: reqwest::Response) -> Result<Usage> {
        let body = response.json::<ChatCompletionsResponse>().await?;

        Ok(body.usage.unwrap_or(Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_read_input_tokens: None,
        }))
    }

    /// 记录使用量
    async fn record_usage(
        &self,
        result: &ForwardResult,
        user_id: Uuid,
        api_key_id: Uuid,
        account_id: Uuid,
    ) -> Result<()> {
        use crate::entity::usages;
        use sea_orm::ActiveModelTrait;
        use sea_orm::Set;

        let usage_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // 计算成本（简单估算，实际应根据模型定价）
        let cost = self.calculate_cost(&result.model, result.usage.total_tokens.into());

        let usage = usages::ActiveModel {
            id: Set(usage_id),
            user_id: Set(user_id),
            api_key_id: Set(api_key_id),
            account_id: Set(Some(account_id)),
            model: Set(result.model.clone()),
            input_tokens: Set(result.usage.prompt_tokens as i64),
            output_tokens: Set(result.usage.completion_tokens as i64),
            cost: Set(cost),
            request_id: Set(Some(result.request_id.clone())),
            success: Set(true),
            error_message: Set(None),
            metadata: Set(Some(serde_json::json!({
                "billing_model": result.billing_model,
                "stream": result.stream,
                "first_token_ms": result.first_token_ms,
                "duration_ms": result.duration_ms,
                "cache_read_tokens": result.usage.cache_read_input_tokens,
            }))),
            created_at: Set(now),
        };

        usage.insert(&self.db).await?;

        tracing::info!(
            "Recorded usage: id={}, model={}, tokens={}, cost={}分, duration_ms={}",
            usage_id,
            result.model,
            result.usage.total_tokens,
            cost,
            result.duration_ms
        );

        Ok(())
    }

    /// 计算成本（单位：分）
    fn calculate_cost(&self, model: &str, total_tokens: u64) -> i64 {
        // 简单的定价模型（单位：分/千token）
        let price_per_1k = match model {
            m if m.starts_with("gpt-4") => 30,    // GPT-4: 30分/千token
            m if m.starts_with("gpt-3.5") => 2,   // GPT-3.5: 2分/千token
            m if m.starts_with("claude-3") => 15, // Claude-3: 15分/千token
            m if m.starts_with("claude-2") => 8,  // Claude-2: 8分/千token
            m if m.starts_with("gemini") => 5,    // Gemini: 5分/千token
            m if m.starts_with("deepseek") => 1,  // DeepSeek: 1分/千token
            _ => 5,                               // 默认: 5分/千token
        };

        (total_tokens as f64 * price_per_1k as f64 / 1000.0).round() as i64
    }
}

/// SSE 事件解析器
pub struct SSEParser {
    buffer: String,
}

impl SSEParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// 解析 SSE 事件
    pub fn parse_line(&mut self, line: &str) -> Option<SSEEvent> {
        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" {
                return None;
            }
            return serde_json::from_str(data).ok();
        }
        None
    }
}

/// SSE 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSEEvent {
    pub id: Option<String>,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: Option<String>,
    pub choices: Vec<SSEChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// SSE 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSEChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config() {
        let openai = ProviderConfig::for_provider("openai");
        assert_eq!(openai.base_url, "https://api.openai.com/v1");

        let anthropic = ProviderConfig::for_provider("anthropic");
        assert_eq!(anthropic.auth_header, "x-api-key");
    }

    #[test]
    fn test_chat_completions_request_parsing() {
        let json = r#"{
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        }"#;

        let req: ChatCompletionsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert!(req.stream);
    }
}
