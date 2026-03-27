//! Token 刷新和登出处理器
//!
//! 提供安全的 JWT token 轮换机制：
//! - POST /auth/refresh - 使用 refresh token 刷新 access token
//! - POST /auth/logout - 登出并撤销 token

use axum::{
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db::RedisPool;
use crate::gateway::SharedState;
use crate::service::user::UserService;

/// 刷新 Token 请求
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// 登出请求
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

/// Token 刷新响应
#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
}

/// 登出响应
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub message: String,
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_code: String,
}

/// API 错误类型
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(ErrorResponse {
            error: self.1.clone(),
            error_code: match self.0 {
                StatusCode::UNAUTHORIZED => "UNAUTHORIZED",
                StatusCode::BAD_REQUEST => "BAD_REQUEST",
                StatusCode::NOT_FOUND => "NOT_FOUND",
                StatusCode::INTERNAL_SERVER_ERROR => "INTERNAL_ERROR",
                _ => "ERROR",
            }
            .to_string(),
        });
        (self.0, body).into_response()
    }
}

/// 从请求头提取 User-Agent
fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// 从请求头提取 IP 地址
fn extract_ip_address(headers: &HeaderMap) -> Option<String> {
    // 尝试从 X-Forwarded-For 获取
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // 取第一个 IP
            if let Some(ip) = forwarded_str.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    // 尝试从 X-Real-IP 获取
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.to_string());
        }
    }

    None
}

/// 从 Authorization 头提取 token
fn extract_access_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if s.starts_with("Bearer ") {
                Some(s[7..].to_string())
            } else {
                None
            }
        })
}

/// POST /auth/refresh - 刷新 Access Token
///
/// 使用 refresh token 获取新的 access token 和 refresh token。
/// 旧的 refresh token 会被自动撤销（安全轮换）。
pub async fn refresh(
    Extension(state): Extension<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, ApiError> {
    // 提取请求元信息
    let user_agent = extract_user_agent(&headers);
    let ip_address = extract_ip_address(&headers);

    // 创建用户服务（带 Redis 支持）
    let user_service = UserService::with_redis(
        state.db.clone(),
        Arc::clone(&state.redis),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 刷新 token
    let token_pair = user_service
        .refresh_access_token(&req.refresh_token, user_agent, ip_address)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("revoked") || msg.contains("expired") || msg.contains("Invalid") {
                ApiError(StatusCode::UNAUTHORIZED, msg)
            } else {
                ApiError(StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        })?;

    Ok(Json(RefreshResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: token_pair.access_token_expires_in,
        refresh_expires_in: token_pair.refresh_token_expires_in,
    }))
}

/// POST /auth/logout - 登出
///
/// 撤销 refresh token 并将 access token 加入黑名单。
/// 客户端应该删除本地存储的所有 token。
pub async fn logout(
    Extension(state): Extension<SharedState>,
    headers: HeaderMap,
    Json(req): Json<LogoutRequest>,
) -> Result<Json<LogoutResponse>, ApiError> {
    // 提取 access token
    let access_token = extract_access_token(&headers)
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "Missing access token".into()))?;

    // 创建用户服务
    let user_service = UserService::with_redis(
        state.db.clone(),
        Arc::clone(&state.redis),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 执行登出
    user_service
        .logout(&access_token, req.refresh_token.as_deref())
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LogoutResponse {
        message: "Successfully logged out".to_string(),
    }))
}

/// POST /auth/logout-all - 登出所有设备
///
/// 撤销用户的所有 refresh token，强制所有设备重新登录。
pub async fn logout_all(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<LogoutResponse>, ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 创建用户服务
    let user_service = UserService::with_redis(
        state.db.clone(),
        Arc::clone(&state.redis),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 撤销所有 token
    let count = user_service
        .revoke_all_user_tokens(user_id, Some("Logout all devices".to_string()))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LogoutResponse {
        message: format!("Successfully logged out from {} devices", count),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "Mozilla/5.0".parse().unwrap());

        let ua = extract_user_agent(&headers);
        assert_eq!(ua, Some("Mozilla/5.0".to_string()));
    }

    #[test]
    fn test_extract_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

        let ip = extract_ip_address(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_extract_ip_from_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "192.168.1.2".parse().unwrap());

        let ip = extract_ip_address(&headers);
        assert_eq!(ip, Some("192.168.1.2".to_string()));
    }

    #[test]
    fn test_extract_access_token() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer my-token-123".parse().unwrap());

        let token = extract_access_token(&headers);
        assert_eq!(token, Some("my-token-123".to_string()));
    }

    #[test]
    fn test_extract_access_token_missing() {
        let headers = HeaderMap::new();
        let token = extract_access_token(&headers);
        assert!(token.is_none());
    }

    #[test]
    fn test_refresh_response_serialization() {
        let response = RefreshResponse {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_expires_in: 604800,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("access_token"));
        assert!(json.contains("Bearer"));
    }

    #[test]
    fn test_logout_response() {
        let response = LogoutResponse {
            message: "Success".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Success"));
    }

    #[test]
    fn test_error_response() {
        let response = ErrorResponse {
            error: "Invalid token".to_string(),
            error_code: "UNAUTHORIZED".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("UNAUTHORIZED"));
    }
}
