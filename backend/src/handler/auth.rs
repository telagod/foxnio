//! 认证处理器 - 完整实现

use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::gateway::SharedState;
use crate::service::user::UserService;
use super::ApiError;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
}

/// 注册
pub async fn register(
    Extension(state): Extension<SharedState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    // 验证邮箱格式
    if !req.email.contains('@') {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Invalid email".into()));
    }

    // 验证密码长度
    if req.password.len() < 8 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Password must be at least 8 characters".into()));
    }

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let user = user_service.register(&req.email, &req.password)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 生成 token
    let token = user_service.generate_token_for(&user)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
        },
    }))
}

/// 登录
pub async fn login(
    Extension(state): Extension<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let (user, token) = user_service.login(&req.email, &req.password)
        .await
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
        },
    }))
}

/// 获取当前用户信息
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

    let user = user_service.get_by_id(user_id)
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
