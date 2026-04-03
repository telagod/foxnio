//! Anthropic Messages API 转发服务
//!
//! 提供 Anthropic Messages API 格式的转发
//! 自动将请求转换为对应 Provider 的格式

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::account::AccountService;
use super::scheduler::SchedulerService;

/// Anthropic Messages 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Anthropic 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

/// Anthropic 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// 内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
    pub source: Option<ImageSource>,
}

/// 图片源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// Anthropic 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ResponseContent>,
    pub model: String,
    #[serde(default)]
    pub usage: AnthropicUsage,
}

/// 响应内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

/// Anthropic 使用量
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// 转发结果
#[derive(Debug, Clone)]
pub struct ForwardResult {
    pub request_id: String,
    pub model: String,
    pub billing_model: String,
    pub stream: bool,
    pub usage: AnthropicUsage,
    pub first_token_ms: Option<u64>,
    pub duration_ms: u64,
    pub content: String,
}

/// Anthropic Messages 转发器
pub struct AnthropicMessagesForwarder {
    db: sea_orm::DatabaseConnection,
    http_client: Client,
    account_service: Arc<AccountService>,
    scheduler: Arc<RwLock<SchedulerService>>,
    max_retries: u32,
}

impl AnthropicMessagesForwarder {
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

    /// 转发 Anthropic Messages 请求
    pub async fn forward(
        &self,
        request: AnthropicMessagesRequest,
        user_id: Uuid,
        api_key_id: Uuid,
    ) -> Result<ForwardResult> {
        let start_time = std::time::Instant::now();
        let original_model = request.model.clone();
        let is_stream = request.stream;

        // 1. 选择账号（优先使用 Anthropic Provider）
        let account = self.select_account(&original_model).await?;

        // 2. 获取凭证
        let credential = self.get_account_credential(account.id).await?;

        // 3. 映射模型
        let mapped_model = self.map_model(&original_model, &account.provider);

        // 4. 发送请求
        let result = self
            .send_request(
                &account.provider,
                &credential,
                &request,
                &mapped_model,
                is_stream,
                &original_model,
                start_time,
            )
            .await?;

        // 5. 记录使用量
        self.record_usage(&result, user_id, api_key_id, account.id)
            .await?;

        Ok(result)
    }

    /// 选择账号
    async fn select_account(&self, model: &str) -> Result<crate::entity::accounts::Model> {
        let scheduler = self.scheduler.read().await;

        let account = scheduler
            .select_account(model, Some("anthropic"), 5)
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

        let credential =
            crate::utils::encryption_global::GlobalEncryption::decrypt(&account.credential)
                .map_err(|e| anyhow!("Failed to decrypt credential: {}", e))?;

        Ok(credential)
    }

    /// 映射模型名称
    fn map_model(&self, model: &str, provider: &str) -> String {
        // 如果是 Anthropic，直接返回
        if provider.to_lowercase() == "anthropic" {
            return model.to_string();
        }

        // 否则需要映射到对应 Provider 的模型
        match (provider.to_lowercase().as_str(), model) {
            // OpenAI 映射
            ("openai", m) if m.starts_with("claude") => {
                // Anthropic 模型在 OpenAI 上可能需要映射
                "gpt-4o".to_string()
            }
            _ => model.to_string(),
        }
    }

    /// 发送请求
    async fn send_request(
        &self,
        provider: &str,
        credential: &str,
        request: &AnthropicMessagesRequest,
        mapped_model: &str,
        is_stream: bool,
        original_model: &str,
        start_time: std::time::Instant,
    ) -> Result<ForwardResult> {
        match provider.to_lowercase().as_str() {
            "anthropic" => {
                self.send_to_anthropic(
                    credential,
                    request,
                    mapped_model,
                    is_stream,
                    original_model,
                    start_time,
                )
                .await
            }
            "openai" => {
                self.send_to_openai(
                    credential,
                    request,
                    mapped_model,
                    is_stream,
                    original_model,
                    start_time,
                )
                .await
            }
            _ => Err(anyhow!(
                "Unsupported provider for Anthropic Messages: {}",
                provider
            )),
        }
    }

    /// 发送到 Anthropic API
    async fn send_to_anthropic(
        &self,
        credential: &str,
        request: &AnthropicMessagesRequest,
        mapped_model: &str,
        is_stream: bool,
        original_model: &str,
        start_time: std::time::Instant,
    ) -> Result<ForwardResult> {
        let url = "https://api.anthropic.com/v1/messages";

        let mut req_builder = self
            .http_client
            .post(url)
            .header("x-api-key", credential)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json");

        if is_stream {
            req_builder = req_builder.header("Accept", "text/event-stream");
        }

        // 构建请求体
        let mut body = serde_json::to_value(request)?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), serde_json::json!(mapped_model));
        }

        let response = req_builder.json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {}: {}", status, error_body));
        }

        let request_id = response
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(&Uuid::new_v4().to_string())
            .to_string();

        let (usage, first_token_ms, content) = if is_stream {
            self.process_anthropic_stream(response, start_time).await?
        } else {
            let resp = response.json::<AnthropicResponse>().await?;
            let content = resp
                .content
                .iter()
                .filter_map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            (resp.usage, None, content)
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
            content,
        })
    }

    /// 发送到 OpenAI（转换格式）
    async fn send_to_openai(
        &self,
        credential: &str,
        request: &AnthropicMessagesRequest,
        mapped_model: &str,
        is_stream: bool,
        original_model: &str,
        start_time: std::time::Instant,
    ) -> Result<ForwardResult> {
        let url = "https://api.openai.com/v1/chat/completions";

        // 转换 Anthropic 格式到 OpenAI 格式
        let openai_request = self.convert_to_openai_request(request, mapped_model);

        let mut req_builder = self
            .http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", credential))
            .header("Content-Type", "application/json");

        if is_stream {
            req_builder = req_builder.header("Accept", "text/event-stream");
        }

        let response = req_builder.json(&openai_request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, error_body));
        }

        let request_id = Uuid::new_v4().to_string();

        // 处理响应并转换回 Anthropic 格式
        let (usage, first_token_ms, content) = if is_stream {
            self.process_openai_stream(response, start_time).await?
        } else {
            let resp: serde_json::Value = response.json().await?;
            let usage = AnthropicUsage {
                input_tokens: resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: resp["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            };
            let content = resp["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();
            (usage, None, content)
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
            content,
        })
    }

    /// 转换为 OpenAI 请求格式
    fn convert_to_openai_request(
        &self,
        request: &AnthropicMessagesRequest,
        mapped_model: &str,
    ) -> serde_json::Value {
        let mut messages = Vec::new();

        // 添加系统消息
        if let Some(system) = &request.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system
            }));
        }

        // 转换消息
        for msg in &request.messages {
            let content = match &msg.content {
                AnthropicContent::Text(text) => text.clone(),
                AnthropicContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| b.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": content
            }));
        }

        serde_json::json!({
            "model": mapped_model,
            "messages": messages,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "stream": request.stream
        })
    }

    /// 处理 Anthropic 流式响应
    async fn process_anthropic_stream(
        &self,
        response: reqwest::Response,
        start_time: std::time::Instant,
    ) -> Result<(AnthropicUsage, Option<u64>, String)> {
        use futures::StreamExt;

        let mut stream = response.bytes_stream();
        let mut first_token_ms: Option<u64> = None;
        let mut usage = AnthropicUsage::default();
        let mut content = String::new();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| anyhow!("Stream error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    continue;
                }

                if first_token_ms.is_none() && data.contains("\"type\":\"content_block_delta\"") {
                    first_token_ms = Some(start_time.elapsed().as_millis() as u64);
                }

                // 解析 SSE 事件
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    if event["type"] == "message_delta" {
                        if let Some(u) = event["usage"].as_object() {
                            usage.input_tokens =
                                u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                            usage.output_tokens =
                                u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        }
                    }
                    if event["type"] == "content_block_delta" {
                        if let Some(delta) = event["delta"]["text"].as_str() {
                            content.push_str(delta);
                        }
                    }
                }
            }
        }

        Ok((usage, first_token_ms, content))
    }

    /// 处理 OpenAI 流式响应
    async fn process_openai_stream(
        &self,
        response: reqwest::Response,
        start_time: std::time::Instant,
    ) -> Result<(AnthropicUsage, Option<u64>, String)> {
        use futures::StreamExt;

        let mut stream = response.bytes_stream();
        let mut first_token_ms: Option<u64> = None;
        let mut usage = AnthropicUsage::default();
        let mut content = String::new();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| anyhow!("Stream error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    continue;
                }

                if first_token_ms.is_none() {
                    first_token_ms = Some(start_time.elapsed().as_millis() as u64);
                }

                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = event["choices"][0]["delta"]["content"].as_str() {
                        content.push_str(delta);
                    }
                    if let Some(u) = event["usage"].as_object() {
                        usage.input_tokens =
                            u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        usage.output_tokens = u
                            .get("completion_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                    }
                }
            }
        }

        Ok((usage, first_token_ms, content))
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

        let cost = self.calculate_cost(
            &result.model,
            result.usage.input_tokens + result.usage.output_tokens,
        );

        let usage = usages::ActiveModel {
            id: Set(usage_id),
            user_id: Set(user_id),
            api_key_id: Set(api_key_id),
            account_id: Set(Some(account_id)),
            model: Set(result.model.clone()),
            input_tokens: Set(result.usage.input_tokens as i64),
            output_tokens: Set(result.usage.output_tokens as i64),
            cost: Set(cost),
            request_id: Set(Some(result.request_id.clone())),
            success: Set(true),
            error_message: Set(None),
            metadata: Set(Some(serde_json::json!({
                "billing_model": result.billing_model,
                "stream": result.stream,
                "first_token_ms": result.first_token_ms,
                "duration_ms": result.duration_ms,
                "api_type": "anthropic_messages",
            }))),
            created_at: Set(now),
        };

        usage.insert(&self.db).await?;

        tracing::info!(
            "Recorded Anthropic usage: id={}, model={}, tokens={}, cost={}分",
            usage_id,
            result.model,
            result.usage.input_tokens + result.usage.output_tokens,
            cost
        );

        Ok(())
    }

    /// 计算成本
    fn calculate_cost(&self, model: &str, total_tokens: u32) -> i64 {
        let price_per_1k = match model {
            m if m.contains("opus") => 75,
            m if m.contains("sonnet") => 15,
            m if m.contains("haiku") => 5,
            _ => 15,
        };
        (total_tokens as f64 * price_per_1k as f64 / 1000.0).round() as i64
    }
}
