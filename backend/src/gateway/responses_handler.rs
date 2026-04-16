//! Responses API Handler
//!
//! 处理 POST /v1/responses 请求

use axum::{
    body::Body,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use reqwest::Client;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;
use uuid::Uuid;

use crate::entity::{accounts, usages};
use crate::gateway::providers::default_provider_registry;
use crate::gateway::{
    responses::ResponsesRequest,
    responses_converter::{
        anthropic_event_to_responses_events, anthropic_to_responses, responses_event_to_sse,
        responses_to_anthropic, ResponsesConverterState,
    },
    SharedState,
};
use crate::service::session_key::RequestSessionHints;
use crate::service::{LegacyAccountService as AccountService, SchedulerService};

/// Responses API 处理器
pub async fn handle_responses(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: Bytes,
) -> Result<Response, ApiError> {
    // 提取 session hints（从 body 中解析，不依赖 headers）
    let mut hints = RequestSessionHints::default();

    // 提取 user_id
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError::Internal(format!("Invalid user_id: {e}")))?;
    let api_key_id = uuid::Uuid::nil();

    // 1. 解析 Responses 请求
    let req: ResponsesRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid request: {e}")))?;

    let client_stream = req.stream;
    let original_model = req.model.clone();

    // 配额预检
    let quota_gate = crate::service::quota_gate::QuotaGate::new(
        state.db.clone(),
        state.config.gateway.rate_multiplier,
    );
    let _permit = quota_gate
        .pre_check(user_id, api_key_id, &original_model, None)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // 从 user 字段补充 hints.metadata_session_id
    if hints.metadata_session_id.is_none() {
        hints.metadata_session_id = req.user.as_deref().and_then(|uid| {
            crate::gateway::claude::parse_metadata_user_id(uid)
                .map(|(_, _, sid)| sid)
                .or_else(|| {
                    crate::gateway::middleware::telemetry::ParsedUserID::parse(uid)
                        .map(|p| p.session_id)
                })
        });
    }

    let session_id = hints.resolve();

    // 2. 转换 Responses → Anthropic
    let anthropic_req = responses_to_anthropic(&req)
        .map_err(|e| ApiError::BadRequest(format!("Conversion failed: {e}")))?;

    // 3. 强制使用流式（Anthropic 最佳实践）
    let mut anthropic_req = anthropic_req;
    anthropic_req.stream = true;

    // 4. 获取可用账号
    let account_service = AccountService::new(state.db.clone());
    let scheduler = SchedulerService::new(
        state.db.clone(),
        account_service,
        crate::service::scheduler::SchedulingStrategy::HealthAware,
    );
    let account = scheduler
        .select_account(&original_model, session_id.as_deref(), 5)
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Failed to select account: {e}")))?
        .ok_or_else(|| ApiError::ServiceUnavailable("No available account".to_string()))?;

    let account_id = account.id;

    // 5. 获取上游配置
    let (base_url, credential) = get_upstream_config(&account)?;

    // 6. 创建 HTTP 客户端
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| ApiError::Internal(format!("Failed to create HTTP client: {e}")))?;

    // 7. 发送到 Anthropic 上游
    let url = format!("{base_url}/v1/messages");

    let response = match http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &credential)
        .header("anthropic-version", "2023-06-01")
        .header("Accept", "text/event-stream")
        .json(&anthropic_req)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            // Record failed usage on request error
            record_usage(
                &state.db, user_id, api_key_id, account_id,
                &original_model, 0, 0, false,
                Some(format!("Request failed: {e}")),
            ).await;
            return Err(ApiError::UpstreamError(format!("Request failed: {e}")));
        }
    };

    // Check for upstream error status
    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        record_usage(
            &state.db, user_id, api_key_id, account_id,
            &original_model, 0, 0, false,
            Some(format!("HTTP {}: {}", status.as_u16(), &error_body)),
        ).await;
        return Err(ApiError::UpstreamError(format!(
            "Upstream returned HTTP {}: {}", status.as_u16(), error_body
        )));
    }

    // 8. 处理响应
    if client_stream {
        handle_streaming_response(
            response, &original_model,
            state.db.clone(), user_id, api_key_id, account_id,
        ).await
    } else {
        handle_buffered_response(
            response, &original_model,
            state.db.clone(), user_id, api_key_id, account_id,
        ).await
    }
}

/// 处理流式响应
async fn handle_streaming_response(
    response: reqwest::Response,
    model: &str,
    db: DatabaseConnection,
    user_id: Uuid,
    api_key_id: Uuid,
    account_id: Uuid,
) -> Result<Response, ApiError> {
    use futures::StreamExt;
    use tokio_stream::wrappers::ReceiverStream;

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let model_owned = model.to_string();

    // 启动后台任务处理 SSE 流
    tokio::spawn(async move {
        let mut state = ResponsesConverterState::new();
        let mut stream = response.bytes_stream();
        let mut input_tokens: i64 = 0;
        let mut output_tokens: i64 = 0;
        let mut had_error = false;
        let mut error_msg: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    // 解析 SSE 事件
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if let Ok(event) = serde_json::from_str::<
                                    crate::gateway::responses::AnthropicStreamEvent,
                                >(data)
                                {
                                    // Track usage from message_start and message_delta
                                    if event.event_type == "message_start" {
                                        if let Some(ref msg) = event.message {
                                            input_tokens = msg.usage.input_tokens as i64;
                                            output_tokens = msg.usage.output_tokens as i64;
                                        }
                                    }
                                    if event.event_type == "message_delta" {
                                        if let Some(ref usage) = event.usage {
                                            output_tokens = usage.output_tokens as i64;
                                        }
                                    }

                                    let events =
                                        anthropic_event_to_responses_events(&event, &mut state);
                                    for evt in events {
                                        if let Ok(sse) = responses_event_to_sse(&evt) {
                                            let _ = tx.send(Ok(sse)).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    had_error = true;
                    error_msg = Some(format!("Stream error: {e}"));
                    let _ = tx.send(Err(format!("Stream error: {e}"))).await;
                    break;
                }
            }
        }

        // Record usage after stream completes
        record_usage(
            &db, user_id, api_key_id, account_id,
            &model_owned, input_tokens, output_tokens,
            !had_error, error_msg,
        ).await;
    });

    // 返回 SSE 流
    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(body)
        .unwrap())
}

/// 处理缓冲响应（收集所有流式事件后返回）
async fn handle_buffered_response(
    response: reqwest::Response,
    model: &str,
    db: DatabaseConnection,
    user_id: Uuid,
    api_key_id: Uuid,
    account_id: Uuid,
) -> Result<Response, ApiError> {
    use futures::StreamExt;

    let _state = ResponsesConverterState::new();
    let mut final_response: Option<crate::gateway::responses::AnthropicResponse> = None;
    let mut stream = response.bytes_stream();

    // 收集所有事件
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ApiError::UpstreamError(e.to_string()))?;

        if let Ok(text) = std::str::from_utf8(&bytes) {
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(event) = serde_json::from_str::<
                        crate::gateway::responses::AnthropicStreamEvent,
                    >(data)
                    {
                        match event.event_type.as_str() {
                            "message_start" => {
                                final_response = event.message.clone();
                            }
                            "content_block_start" => {
                                if let (Some(ref mut resp), Some(block)) =
                                    (&mut final_response, &event.content_block)
                                {
                                    resp.content.push(block.clone());
                                }
                            }
                            "content_block_delta" => {
                                // 处理增量内容
                                if let (Some(ref mut resp), Some(delta)) =
                                    (&mut final_response, &event.delta)
                                {
                                    if let Some(index) = event.index {
                                        let idx = index as usize;
                                        if idx < resp.content.len() {
                                            match delta.delta_type.as_deref() {
                                                Some("text_delta") => {
                                                    if let Some(text) = &delta.text {
                                                        resp.content[idx].text = Some(
                                                            resp.content[idx]
                                                                .text
                                                                .as_deref()
                                                                .unwrap_or("")
                                                                .to_string()
                                                                + text,
                                                        );
                                                    }
                                                }
                                                Some("thinking_delta") => {
                                                    if let Some(thinking) = &delta.thinking {
                                                        resp.content[idx].thinking = Some(
                                                            resp.content[idx]
                                                                .thinking
                                                                .as_deref()
                                                                .unwrap_or("")
                                                                .to_string()
                                                                + thinking,
                                                        );
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            "message_delta" => {
                                if let (Some(ref mut resp), Some(usage)) =
                                    (&mut final_response, &event.usage)
                                {
                                    resp.usage = usage.clone();
                                }
                                if let (Some(ref mut resp), Some(delta)) =
                                    (&mut final_response, &event.delta)
                                {
                                    if let Some(stop_reason) = &delta.stop_reason {
                                        resp.stop_reason = stop_reason.clone();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // 转换为 Responses 格式
    let response = match final_response {
        Some(resp) => resp,
        None => {
            record_usage(
                &db, user_id, api_key_id, account_id,
                model, 0, 0, false,
                Some("No response received".to_string()),
            ).await;
            return Err(ApiError::UpstreamError("No response received".to_string()));
        }
    };

    // Record successful usage
    let input_tokens = response.usage.input_tokens as i64;
    let output_tokens = response.usage.output_tokens as i64;
    record_usage(
        &db, user_id, api_key_id, account_id,
        model, input_tokens, output_tokens, true, None,
    ).await;

    let responses_resp = anthropic_to_responses(&response, model);

    Ok(serde_json::to_string(&responses_resp)
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_response())
}

/// API 错误
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    ServiceUnavailable(String),
    UpstreamError(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
            Self::UpstreamError(msg) => (StatusCode::BAD_GATEWAY, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = serde_json::json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
            }
        });

        (status, serde_json::to_string(&body).unwrap_or_default()).into_response()
    }
}

/// 获取上游配置
fn get_upstream_config(account: &accounts::Model) -> Result<(String, String), ApiError> {
    let provider_key = account.provider.as_str();
    let adapter = default_provider_registry()
        .get(provider_key)
        .ok_or_else(|| ApiError::BadRequest(format!("Unsupported provider: {}", provider_key)))?;

    let base_url = match provider_key {
        "anthropic" => std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| adapter.base_url().to_string()),
        _ => adapter.base_url().to_string(),
    };

    Ok((base_url, account.credential.clone()))
}

/// 计算 Responses API 费用
fn calculate_responses_cost(model: &str, total_tokens: i64) -> i64 {
    let price_per_1k = match model {
        m if m.contains("opus") => 75,
        m if m.contains("sonnet") => 15,
        m if m.contains("haiku") => 5,
        _ => 15,
    };
    (total_tokens as f64 * price_per_1k as f64 / 1000.0).round() as i64
}

/// 记录使用量到数据库
async fn record_usage(
    db: &DatabaseConnection,
    user_id: Uuid,
    api_key_id: Uuid,
    account_id: Uuid,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    success: bool,
    error_message: Option<String>,
) {
    let total_tokens = input_tokens + output_tokens;
    let cost = calculate_responses_cost(model, total_tokens);

    let usage_record = usages::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        api_key_id: Set(api_key_id),
        account_id: Set(Some(account_id)),
        model: Set(model.to_string()),
        input_tokens: Set(input_tokens),
        output_tokens: Set(output_tokens),
        cost: Set(cost),
        request_id: Set(Some(Uuid::new_v4().to_string())),
        success: Set(success),
        error_message: Set(error_message),
        metadata: Set(Some(serde_json::json!({"api_type": "responses"}))),
        created_at: Set(chrono::Utc::now()),
    };
    if let Err(e) = usage_record.insert(db).await {
        tracing::warn!("Failed to record responses usage: {e}");
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "BadRequest: {msg}"),
            Self::ServiceUnavailable(msg) => write!(f, "ServiceUnavailable: {msg}"),
            Self::UpstreamError(msg) => write!(f, "UpstreamError: {msg}"),
            Self::Internal(msg) => write!(f, "Internal: {msg}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// 从 HTTP headers 提取 session hints
fn extract_response_session_hints(
    headers: &axum::http::HeaderMap,
) -> RequestSessionHints {
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let x_session_id = headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    RequestSessionHints {
        metadata_session_id: None,
        x_session_id,
        client_ip,
        user_agent,
    }
}
