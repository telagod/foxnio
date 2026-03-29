//! Gemini Native API 支持模块
//!
//! 提供 Google Gemini SDK/CLI 兼容的原生 API 端点支持
//!
//! ## 功能
//!
//! - `/v1beta/models` 路由支持
//! - Gemini SDK 兼容的请求/响应格式
//! - 流式响应支持 (SSE)
//! - 模型列表和详情查询
//!
//! ## 参考
//!
//! 基于Sub2API的实现，支持 Gemini SDK 直连

pub mod client;
pub mod types;

pub use client::{GeminiClient, GeminiClientConfig};
pub use types::*;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use futures::StreamExt;
use serde::Deserialize;
use std::sync::Arc;

use crate::gateway::SharedState;

/// Gemini v1beta 路由处理器
pub struct GeminiHandler {
    client: GeminiClient,
}

impl GeminiHandler {
    /// 创建新的处理器
    pub fn new() -> Self {
        Self {
            client: GeminiClient::with_defaults(),
        }
    }

    /// 使用自定义配置创建处理器
    pub fn with_config(config: GeminiClientConfig) -> Self {
        Self {
            client: GeminiClient::new(config),
        }
    }

    /// 获取客户端引用
    pub fn client(&self) -> &GeminiClient {
        &self.client
    }
}

impl Default for GeminiHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ============ 请求参数 ============

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct GeminiQueryParams {
    /// API Key（可选，也可以通过 header 提供）
    #[serde(rename = "key")]
    pub api_key: Option<String>,
    /// 流式响应格式
    #[serde(rename = "alt")]
    pub alt: Option<String>,
}

// ============ 路由处理器 ============

/// GET /v1beta/models - 列出所有模型
pub async fn list_models(
    State(_state): State<SharedState>,
    Query(params): Query<GeminiQueryParams>,
) -> impl IntoResponse {
    // 如果没有 API key，返回默认模型列表
    let api_key = match params.api_key {
        Some(key) => key,
        None => {
            // 返回静态模型列表
            return Json(GeminiModelsListResponse::default_models()).into_response();
        }
    };

    let client = GeminiClient::with_defaults();
    match client.list_models(&api_key).await {
        Ok(models) => Json(models).into_response(),
        Err(e) => {
            tracing::warn!("Failed to list models: {}", e);
            // 失败时返回默认模型列表
            Json(GeminiModelsListResponse::default_models()).into_response()
        }
    }
}

/// GET /v1beta/models/:model - 获取单个模型信息
pub async fn get_model(
    State(_state): State<SharedState>,
    Path(model): Path<String>,
    Query(params): Query<GeminiQueryParams>,
) -> impl IntoResponse {
    let model_name = client::extract_model_name(&model);

    let api_key = match params.api_key {
        Some(key) => key,
        None => {
            // 返回默认模型信息
            return Json(client::normalize_model_name(model_name)).into_response();
        }
    };

    let client = GeminiClient::with_defaults();
    match client.get_model(model_name, &api_key).await {
        Ok(model_info) => Json(model_info).into_response(),
        Err(e) => {
            tracing::warn!("Failed to get model {}: {}", model_name, e);
            // 返回默认模型信息
            let fallback = GeminiModel {
                name: client::normalize_model_name(model_name),
                display_name: Some(model_name.to_string()),
                description: None,
                supported_generation_methods: Some(vec![
                    "generateContent".to_string(),
                    "streamGenerateContent".to_string(),
                ]),
            };
            Json(fallback).into_response()
        }
    }
}

/// POST /v1beta/models/:model:generateContent - 生成内容（非流式）
pub async fn generate_content(
    State(state): State<SharedState>,
    Path(model_action): Path<String>,
    Query(params): Query<GeminiQueryParams>,
    body: Bytes,
) -> impl IntoResponse {
    // 解析模型和动作
    let (model_name, action) = match client::parse_model_action(&model_action) {
        Ok(result) => result,
        Err(e) => {
            let error = client::build_error_response(
                404,
                &format!("Invalid model action: {}", e),
                "NOT_FOUND",
            );
            return (StatusCode::NOT_FOUND, Json(error)).into_response();
        }
    };

    // 检查是否为流式请求
    let is_stream = action == "streamGenerateContent" || params.alt.as_deref() == Some("sse");

    // 解析请求体
    let request: GenerateContentRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            let error = client::build_error_response(
                400,
                &format!("Invalid request body: {}", e),
                "INVALID_ARGUMENT",
            );
            return (StatusCode::BAD_REQUEST, Json(error)).into_response();
        }
    };

    // 获取 API key
    let api_key = match params.api_key {
        Some(key) => key,
        None => {
            let error = client::build_error_response(
                401,
                "API key is required",
                "UNAUTHENTICATED",
            );
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    };

    // 获取账户和转发（简化版本，实际需要调度器）
    // TODO: 实现完整的账户选择和请求转发逻辑
    let client = GeminiClient::with_defaults();

    if is_stream {
        // 流式响应
        match client.stream_generate_content(&model_name, &request, &api_key).await {
            Ok(stream) => {
                let body = Body::from_stream(stream);
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(body)
                    .unwrap()
            }
            Err(e) => {
                let error =
                    client::build_error_response(502, &format!("Upstream error: {}", e), "UNAVAILABLE");
                (StatusCode::BAD_GATEWAY, Json(error)).into_response()
            }
        }
    } else {
        // 非流式响应
        match client.generate_content(&model_name, &request, &api_key).await {
            Ok(response) => Json(response).into_response(),
            Err(e) => {
                let error =
                    client::build_error_response(502, &format!("Upstream error: {}", e), "UNAVAILABLE");
                (StatusCode::BAD_GATEWAY, Json(error)).into_response()
            }
        }
    }
}

/// POST /v1beta/models/:model:streamGenerateContent - 流式生成内容
pub async fn stream_generate_content(
    state: State<SharedState>,
    Path(model_action): Path<String>,
    Query(params): Query<GeminiQueryParams>,
    body: Bytes,
) -> impl IntoResponse {
    generate_content(state, Path(model_action), Query(params), body).await
}

/// POST /v1beta/models/:model:countTokens - 计算 Token 数量
pub async fn count_tokens(
    State(_state): State<SharedState>,
    Path(model): Path<String>,
    Query(params): Query<GeminiQueryParams>,
    body: Bytes,
) -> impl IntoResponse {
    let model_name = client::extract_model_name(&model);

    let request: GenerateContentRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            let error = client::build_error_response(
                400,
                &format!("Invalid request body: {}", e),
                "INVALID_ARGUMENT",
            );
            return (StatusCode::BAD_REQUEST, Json(error)).into_response();
        }
    };

    let api_key = match params.api_key {
        Some(key) => key,
        None => {
            let error = client::build_error_response(
                401,
                "API key is required",
                "UNAUTHENTICATED",
            );
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    };

    let client = GeminiClient::with_defaults();
    match client.count_tokens(model_name, &request, &api_key).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => {
            let error =
                client::build_error_response(502, &format!("Upstream error: {}", e), "UNAVAILABLE");
            (StatusCode::BAD_GATEWAY, Json(error)).into_response()
        }
    }
}

/// POST /v1beta/models/:model:embedContent - 内容嵌入
pub async fn embed_content(
    State(_state): State<SharedState>,
    Path(model): Path<String>,
    Query(params): Query<GeminiQueryParams>,
    body: Bytes,
) -> impl IntoResponse {
    let model_name = client::extract_model_name(&model);

    let request: client::EmbedContentRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            let error = client::build_error_response(
                400,
                &format!("Invalid request body: {}", e),
                "INVALID_ARGUMENT",
            );
            return (StatusCode::BAD_REQUEST, Json(error)).into_response();
        }
    };

    let api_key = match params.api_key {
        Some(key) => key,
        None => {
            let error = client::build_error_response(
                401,
                "API key is required",
                "UNAUTHENTICATED",
            );
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    };

    let client = GeminiClient::with_defaults();
    match client.embed_content(model_name, &request, &api_key).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => {
            let error =
                client::build_error_response(502, &format!("Upstream error: {}", e), "UNAVAILABLE");
            (StatusCode::BAD_GATEWAY, Json(error)).into_response()
        }
    }
}

// ============ 辅助函数 ============

/// 构建 Gemini 路由
pub fn build_gemini_routes() -> axum::Router<SharedState> {
    use axum::routing::{get, post};

    axum::Router::new()
        // 模型列表
        .route("/v1beta/models", get(list_models))
        // 单个模型
        .route("/v1beta/models/{model}", get(get_model))
        // 生成内容（支持动态 action）
        .route("/v1beta/models/{model_action}", post(generate_content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_handler_creation() {
        let handler = GeminiHandler::new();
        assert!(!handler.client().base_url().is_empty());
    }

    #[test]
    fn test_default_models_not_empty() {
        let models = GeminiModelsListResponse::default_models();
        assert!(!models.models.is_empty());
        assert!(models
            .models
            .iter()
            .any(|m| m.name.contains("gemini-2.0-flash")));
    }
}
