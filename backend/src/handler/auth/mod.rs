//! 认证处理器模块 v0.2.0
//!
//! 包含：
//! - 注册和登录
//! - Token 刷新和登出
//! - 密码重置
//! - TOTP 两步验证

#![allow(dead_code)]
pub mod password;
pub mod refresh;
pub mod totp;

// 重新导出刷新相关函数
#[allow(unused_imports)]
pub use refresh::{logout, logout_all, refresh};

// 重新导出 TOTP 相关函数
pub use totp::{
    backup_code_login, confirm_enable_totp, disable_totp, enable_totp, get_totp_status,
    regenerate_backup_codes, totp_login, verify_totp,
};

use crate::gateway::SharedState;
use crate::service::user::UserService;
use axum::{
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa::OpenApi;

/// 用户注册请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// 用户邮箱
    pub email: String,
    /// 密码（至少 8 个字符）
    pub password: String,
}

/// 用户登录请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// 用户邮箱
    pub email: String,
    /// 密码
    pub password: String,
}

/// 登录响应（包含 refresh token，支持 TOTP）
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum LoginResponse {
    /// 直接登录成功（未启用 TOTP）
    Success {
        access_token: String,
        refresh_token: String,
        token_type: String,
        expires_in: i64,
        refresh_expires_in: i64,
        user: UserInfo,
    },
    /// 需要 TOTP 验证
    RequiresTotp {
        temp_token: String,
        expires_in: i64,
        message: String,
    },
}

/// 登录响应（兼容旧接口）
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_expires_in: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_totp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// 注册响应
#[derive(Debug, Serialize, ToSchema)]
pub struct RegisterResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
}

/// API 错误类型
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
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
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip) = forwarded_str.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.to_string());
        }
    }

    None
}

/// 注册
///
/// 创建新用户账号
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "注册成功", body = RegisterResponse),
        (status = 400, description = "无效的请求参数")
    ),
    tag = "认证"
)]
pub async fn register(
    Extension(state): Extension<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ApiError> {
    if !req.email.contains('@') {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Invalid email".into()));
    }

    if req.password.len() < 8 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters".into(),
        ));
    }

    let _user_agent = extract_user_agent(&headers);
    let _ip_address = extract_ip_address(&headers);

    let user_service = UserService::with_redis(
        state.db.clone(),
        std::sync::Arc::clone(&state.redis),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let user = user_service
        .register(&req.email, &req.password)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let token = user_service
        .generate_token_for(&user)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RegisterResponse {
        access_token: token,
        refresh_token: String::new(),
        token_type: "Bearer".to_string(),
        expires_in: (state.config.jwt.expire_hours * 3600) as i64,
        refresh_expires_in: 0,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
        },
    }))
}

/// 登录（返回 access_token 和 refresh_token，支持 TOTP）
///
/// 用户登录接口，支持TOTP两步验证
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "登录成功", body = AuthResponse),
        (status = 401, description = "认证失败")
    ),
    tag = "认证"
)]
pub async fn login(
    Extension(state): Extension<SharedState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user_agent = extract_user_agent(&headers);
    let ip_address = extract_ip_address(&headers);

    let user_service = UserService::with_redis(
        state.db.clone(),
        std::sync::Arc::clone(&state.redis),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let response = user_service
        .login(&req.email, &req.password, user_agent, ip_address)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("Invalid credentials") {
                ApiError(StatusCode::UNAUTHORIZED, "Invalid email or password".into())
            } else if msg.contains("not active") {
                ApiError(StatusCode::FORBIDDEN, msg)
            } else {
                ApiError(StatusCode::UNAUTHORIZED, msg)
            }
        })?;

    match response {
        crate::service::user::LoginResponse::Success {
            user,
            access_token,
            refresh_token,
            access_token_expires_in,
            refresh_token_expires_in,
        } => Ok(Json(AuthResponse {
            access_token: Some(access_token),
            refresh_token: Some(refresh_token),
            token_type: "Bearer".to_string(),
            expires_in: Some(access_token_expires_in),
            refresh_expires_in: Some(refresh_token_expires_in),
            user: Some(UserInfo {
                id: user.id.to_string(),
                email: user.email,
                role: user.role,
                status: user.status,
                balance: user.balance,
            }),
            temp_token: None,
            requires_totp: None,
            message: None,
        })),
        crate::service::user::LoginResponse::RequiresTotp {
            temp_token,
            expires_in: _,
            message,
        } => Ok(Json(AuthResponse {
            access_token: None,
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_in: None,
            refresh_expires_in: None,
            user: None,
            temp_token: Some(temp_token),
            requires_totp: Some(true),
            message: Some(message),
        })),
    }
}

/// 获取当前用户信息
///
/// 获取当前登录用户的详细信息
#[utoipa::path(
    get,
    path = "/api/v1/user/me",
    responses(
        (status = 200, description = "用户信息", body = UserInfo),
        (status = 401, description = "未授权")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "用户"
)]
pub async fn get_me(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<UserInfo>, ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let user = user_service
        .get_by_id(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "User not found".into()))?;

    Ok(Json(UserInfo {
        id: user.id.to_string(),
        email: user.email,
        role: user.role,
        status: user.status,
        balance: user.balance,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_info() {
        let info = UserInfo {
            id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            role: "user".to_string(),
            status: "active".to_string(),
            balance: 100,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "TestAgent/1.0".parse().unwrap());

        let ua = extract_user_agent(&headers);
        assert_eq!(ua, Some("TestAgent/1.0".to_string()));
    }

    #[test]
    fn test_extract_ip_address() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());

        let ip = extract_ip_address(&headers);
        assert_eq!(ip, Some("10.0.0.1".to_string()));
    }
}
