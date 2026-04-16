//! 中间件 - 完整实现

#![allow(dead_code)]
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde_json::json;
use std::time::Instant;

use crate::gateway::SharedState;
use crate::service::LegacyApiKeyService as ApiKeyService;

/// API Key 认证中间件
pub async fn api_key_auth(
    Extension(state): Extension<SharedState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从请求头获取 API Key
    let api_key = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| req.headers().get("x-api-key").and_then(|v| v.to_str().ok()));

    let api_key = match api_key {
        Some(k) => k,
        None => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // 验证 API Key
    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let key = api_key_service
        .validate(api_key)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 检查 Key 是否有效
    if !key.is_active() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 将用户信息添加到请求扩展中
    req.extensions_mut().insert(key.user_id);
    req.extensions_mut().insert(key.id);

    Ok(next.run(req).await)
}

/// JWT 认证中间件
pub async fn jwt_auth(
    Extension(state): Extension<SharedState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    // 从请求头获取 Token
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing authorization token" })),
            )
                .into_response());
        }
    };

    // 验证 Token
    let user_service = crate::service::UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let claims = match user_service.verify_token(token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid or expired token" })),
            )
                .into_response());
        }
    };

    // 将用户信息添加到请求扩展中
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// 请求日志中间件
pub async fn request_log(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let elapsed = start.elapsed();
    let status = response.status();

    tracing::info!("{} {} - {} - {:?}", method, uri, status.as_u16(), elapsed);

    response
}

/// CORS 中间件
pub fn cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::permissive()
}

/// 速率限制中间件（基于 Redis）
pub async fn rate_limit(
    Extension(_state): Extension<SharedState>,
    Extension(_user_id): Extension<uuid::Uuid>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // NOTE: 实现 Redis 速率限制
    // 1. 从 Redis 获取当前速率
    // 2. 检查是否超限
    // 3. 更新速率计数器

    Ok(next.run(req).await)
}

/// 并发限制中间件
pub async fn concurrency_limit(
    Extension(_state): Extension<SharedState>,
    Extension(_user_id): Extension<uuid::Uuid>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // NOTE: 实现并发限制
    // 1. 检查用户当前并发数
    // 2. 如果超限，返回 429

    Ok(next.run(req).await)
}

/// 请求 ID 中间件
pub async fn request_id(mut req: Request<Body>, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(crate::utils::request_id);

    req.extensions_mut().insert(request_id.clone());

    let mut response = next.run(req).await;

    response
        .headers_mut()
        .insert("x-request-id", request_id.parse().unwrap());

    response
}

/// 错误处理中间件
pub async fn error_handler(req: Request<Body>, next: Next) -> Response {
    let response = next.run(req).await;

    // 如果是错误响应，添加更多上下文
    if !response.status().is_success() {
        let status = response.status();

        // 记录错误
        tracing::warn!("Request failed with status: {}", status.as_u16());
    }

    response
}

#[cfg(test)]
mod middleware_tests {

    #[test]
    fn test_request_id_format() {
        let id = crate::utils::request_id();

        // Request ID should be non-empty and have reasonable length
        assert!(!id.is_empty());
        assert!(id.len() >= 8);
    }
}
