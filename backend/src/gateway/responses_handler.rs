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
use std::sync::Arc;

use crate::entity::accounts;
use crate::gateway::{
    responses::ResponsesRequest,
    responses_converter::{
        anthropic_event_to_responses_events, anthropic_to_responses, responses_event_to_sse,
        responses_to_anthropic, ResponsesConverterState,
    },
    SharedState,
};
use crate::service::{LegacyAccountService as AccountService, SchedulerService};

/// Responses API 处理器
pub async fn handle_responses(
    Extension(state): Extension<Arc<SharedState>>,
    body: Bytes,
) -> Result<Response, ApiError> {
    // 1. 解析 Responses 请求
    let req: ResponsesRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid request: {e}")))?;

    let client_stream = req.stream;
    let original_model = req.model.clone();

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
        .select_account(&original_model, None, 5)
        .await
        .map_err(|e| ApiError::ServiceUnavailable(format!("Failed to select account: {e}")))?
        .ok_or_else(|| ApiError::ServiceUnavailable("No available account".to_string()))?;

    // 5. 获取上游配置
    let (base_url, credential) = get_upstream_config(&account)?;

    // 6. 创建 HTTP 客户端
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| ApiError::Internal(format!("Failed to create HTTP client: {e}")))?;

    // 7. 发送到 Anthropic 上游
    let url = format!("{base_url}/v1/messages");

    let response = http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &credential)
        .header("anthropic-version", "2023-06-01")
        .header("Accept", "text/event-stream")
        .json(&anthropic_req)
        .send()
        .await
        .map_err(|e| ApiError::UpstreamError(format!("Request failed: {e}")))?;

    // 8. 处理响应
    if client_stream {
        handle_streaming_response(response, &original_model).await
    } else {
        handle_buffered_response(response, &original_model).await
    }
}

/// 处理流式响应
async fn handle_streaming_response(
    response: reqwest::Response,
    model: &str,
) -> Result<Response, ApiError> {
    use futures::StreamExt;
    use tokio_stream::wrappers::ReceiverStream;

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let _model = model.to_string();

    // 启动后台任务处理 SSE 流
    tokio::spawn(async move {
        let mut state = ResponsesConverterState::new();
        let mut stream = response.bytes_stream();

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
                    let _ = tx.send(Err(format!("Stream error: {e}"))).await;
                    break;
                }
            }
        }
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
    let response = final_response
        .ok_or_else(|| ApiError::UpstreamError("No response received".to_string()))?;

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
    // 根据提供商返回不同的配置
    match account.provider.as_str() {
        "anthropic" => {
            let base_url = std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
            Ok((base_url, account.credential.clone()))
        }
        _ => Err(ApiError::BadRequest(format!(
            "Unsupported provider: {}",
            account.provider
        ))),
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "BadRequest: {}", msg),
            Self::ServiceUnavailable(msg) => write!(f, "ServiceUnavailable: {}", msg),
            Self::UpstreamError(msg) => write!(f, "UpstreamError: {}", msg),
            Self::Internal(msg) => write!(f, "Internal: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}
