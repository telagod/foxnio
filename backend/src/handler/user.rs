//! 用户端点处理器
//!
//! 提供用户个人信息管理、密码修改等端点

#![allow(dead_code)]

use axum::{http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::user::{Claims, UserService};

/// 更新用户信息请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
}

/// 修改密码请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// 用户信息响应
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfoResponse {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub balance_yuan: f64,
    pub created_at: String,
}

/// PUT /api/v1/user - 更新个人信息
///
/// 更新当前用户的个人信息
#[utoipa::path(
    put,
    path = "/api/v1/user",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "更新成功"),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "用户"
)]
pub async fn update_user_info(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {}", e)))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 验证邮箱格式（如果提供）
    if let Some(ref email) = req.email {
        if !email.contains('@') {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Invalid email format".into(),
            ));
        }
    }

    // 更新用户信息
    let user = user_service
        .update_profile(user_id, req.email.as_deref(), None, None)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "User not found".into()))?;

    Ok(Json(json!({
        "success": true,
        "user": {
            "id": user.id.to_string(),
            "email": user.email,
        }
    })))
}

/// PUT /api/v1/user/password - 修改密码
///
/// 修改当前用户的密码
#[utoipa::path(
    put,
    path = "/api/v1/user/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "密码修改成功"),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "用户"
)]
pub async fn change_password(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {}", e)))?;

    // 验证新密码长度
    if req.new_password.len() < 8 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "New password must be at least 8 characters".into(),
        ));
    }

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 验证当前密码并更新
    user_service
        .change_password(user_id, &req.current_password, &req.new_password)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("invalid") || msg.contains("Incorrect") {
                ApiError(
                    StatusCode::BAD_REQUEST,
                    "Current password is incorrect".into(),
                )
            } else {
                ApiError(StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        })?;

    Ok(Json(json!({
        "success": true,
        "message": "Password changed successfully"
    })))
}
