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
    #[serde(default)]
    pub metadata: Option<RequestMetadata>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// 请求元数据（包含 user_id，内含 session_id）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub user_id: Option<String>,
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
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
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
    concurrency: Option<Arc<crate::service::concurrency::ConcurrencyController>>,
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
            concurrency: None,
            max_retries: 3,
        }
    }

    /// 设置并发控制器
    pub fn with_concurrency(
        mut self,
        concurrency: Arc<crate::service::concurrency::ConcurrencyController>,
    ) -> Self {
        self.concurrency = Some(concurrency);
        self
    }

    /// 转发 Anthropic Messages 请求
    pub async fn forward(
        &self,
        request: AnthropicMessagesRequest,
        user_id: Uuid,
        api_key_id: Uuid,
        mut hints: crate::service::session_key::RequestSessionHints,
    ) -> Result<ForwardResult> {
        let start_time = std::time::Instant::now();
        let original_model = request.model.clone();
        let is_stream = request.stream;

        // 从 metadata.user_id 补充 hints.metadata_session_id
        if hints.metadata_session_id.is_none() {
            hints.metadata_session_id = request
                .metadata
                .as_ref()
                .and_then(|m| m.user_id.as_deref())
                .and_then(|uid| {
                    crate::gateway::claude::parse_metadata_user_id(uid)
                        .map(|(_, _, sid)| sid)
                        .or_else(|| {
                            crate::gateway::middleware::telemetry::ParsedUserID::parse(uid)
                                .map(|p| p.session_id)
                        })
                });
        }

        let session_id = hints.resolve();

        // 1. 选择账号
        let account = self
            .select_account(&original_model, session_id.as_deref())
            .await?;

        // 1.5 并发控制（如果启用）
        let _concurrency_slot = if let Some(ref cc) = self.concurrency {
            Some(
                cc.try_acquire_with_timeout(
                    &user_id.to_string(),
                    &account.id.to_string(),
                    &api_key_id.to_string(),
                    std::time::Duration::from_secs(30),
                )
                .await
                .map_err(|e| anyhow::anyhow!("Concurrency limit: {e}"))?,
            )
        } else {
            None
        };

        // 2-4. 发送请求（带 failover 重试）
        let mut excluded_accounts = std::collections::HashSet::new();
        let mut last_error: Option<anyhow::Error> = None;
        let max_attempts = self.max_retries.min(3) as usize;

        for attempt in 0..max_attempts {
            // 重试时重新选择账号（排除已失败的）
            let (current_account, credential, mapped_model) = if attempt == 0 {
                let cred = self.get_account_credential(account.id).await?;
                let mapped = self.map_model(&original_model, &account.provider);
                (account.clone(), cred, mapped)
            } else {
                // 重新选择账号，排除已失败的
                let retry_account = {
                    let scheduler = self.scheduler.read().await;
                    let mut candidates = scheduler
                        .get_available_accounts_for_model(&original_model)
                        .await
                        .unwrap_or_default();
                    candidates.retain(|a| !excluded_accounts.contains(&a.id));
                    candidates.into_iter().next()
                };

                match retry_account {
                    Some(acc) => {
                        let cred = match self.get_account_credential(acc.id).await {
                            Ok(c) => c,
                            Err(_) => break,
                        };
                        let mapped = self.map_model(&original_model, &acc.provider);
                        (acc, cred, mapped)
                    }
                    None => break, // 无可用账号，退出重试
                }
            };

            match self
                .send_request(
                    &current_account.provider,
                    &credential,
                    &request,
                    &mapped_model,
                    is_stream,
                    &original_model,
                    start_time,
                )
                .await
            {
                Ok(result) => {
                    // 记录成功使用量
                    self.record_usage(&result, user_id, api_key_id, current_account.id)
                        .await?;
                    return Ok(result);
                }
                Err(e) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    self.record_failure_usage(
                        &original_model,
                        user_id,
                        api_key_id,
                        current_account.id,
                        &e.to_string(),
                        duration_ms,
                    )
                    .await;
                    excluded_accounts.insert(current_account.id);
                    tracing::warn!(
                        "Attempt {}/{} failed for account {}: {}",
                        attempt + 1,
                        max_attempts,
                        current_account.id,
                        e
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts exhausted")))
    }

    /// 选择账号
    async fn select_account(
        &self,
        model: &str,
        session_id: Option<&str>,
    ) -> Result<crate::entity::accounts::Model> {
        let scheduler = self.scheduler.read().await;

        let account = scheduler
            .select_account(model, session_id, 5)
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
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: resp["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64().unwrap_or(0) as u32,
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
                    // message_start 包含初始 usage（含 cache tokens）
                    if event["type"] == "message_start" {
                        if let Some(u) = event["message"]["usage"].as_object() {
                            usage.input_tokens =
                                u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                            usage.cache_creation_input_tokens =
                                u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                            usage.cache_read_input_tokens =
                                u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        }
                    }
                    if event["type"] == "message_delta" {
                        if let Some(u) = event["usage"].as_object() {
                            // message_delta 的 usage 包含最终 output_tokens
                            usage.output_tokens =
                                u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                            // 也可能更新 input_tokens
                            if let Some(it) = u.get("input_tokens").and_then(|v| v.as_u64()) {
                                usage.input_tokens = it as u32;
                            }
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
                "cache_creation_input_tokens": result.usage.cache_creation_input_tokens,
                "cache_read_input_tokens": result.usage.cache_read_input_tokens,
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

    /// 记录失败使用量
    async fn record_failure_usage(
        &self,
        model: &str,
        user_id: Uuid,
        api_key_id: Uuid,
        account_id: Uuid,
        error_message: &str,
        duration_ms: u64,
    ) {
        use crate::entity::usages;
        use sea_orm::ActiveModelTrait;
        use sea_orm::Set;

        let usage = usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            api_key_id: Set(api_key_id),
            account_id: Set(Some(account_id)),
            model: Set(model.to_string()),
            input_tokens: Set(0),
            output_tokens: Set(0),
            cost: Set(0),
            request_id: Set(Some(Uuid::new_v4().to_string())),
            success: Set(false),
            error_message: Set(Some(error_message.chars().take(500).collect())),
            metadata: Set(Some(serde_json::json!({
                "api_type": "anthropic_messages",
                "duration_ms": duration_ms,
            }))),
            created_at: Set(chrono::Utc::now()),
        };

        if let Err(e) = usage.insert(&self.db).await {
            tracing::warn!("Failed to record failure usage: {}", e);
        }
    }
}
