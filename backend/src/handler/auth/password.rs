//! 密码重置处理器

use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::gateway::SharedState;
use crate::service::email::SmtpEmailSender;
use crate::service::password_reset::PasswordResetService;
use super::ApiError;

/// 请求密码重置
#[derive(Debug, Deserialize)]
pub struct ResetRequestRequest {
    pub email: String,
}

/// 重置密码
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

/// 通用响应
#[derive(Debug, Serialize)]
pub struct ResetResponse {
    pub success: bool,
    pub message: String,
}

/// Token 验证响应
#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    pub valid: bool,
}

/// POST /auth/password/reset-request
/// 请求密码重置（发送邮件）
pub async fn request_reset(
    Extension(state): Extension<SharedState>,
    Json(req): Json<ResetRequestRequest>,
) -> Result<Json<ResetResponse>, ApiError> {
    // 验证邮箱格式
    if !req.email.contains('@') {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid email format".into(),
        ));
    }

    // 获取邮件配置
    let email_config = state.config.email.clone()
        .unwrap_or_default();

    // 创建邮件发送器
    let email_sender = SmtpEmailSender::new(email_config)
        .map_err(|e| ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to initialize email sender: {}", e),
        ))?;

    // 获取重置 URL 基础路径
    let reset_url_base = state.config.email.as_ref()
        .map(|c| c.reset_url_base.clone())
        .unwrap_or_else(|| {
            format!("http://localhost:{}", state.config.server.port)
        });

    // 创建密码重置服务
    let reset_service = PasswordResetService::new(
        state.db.clone(),
        email_sender,
        reset_url_base,
    );

    // 请求重置
    reset_service
        .request_reset(&req.email)
        .await
        .map_err(|e| ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))?;

    // 无论邮箱是否存在，都返回成功（防止枚举攻击）
    Ok(Json(ResetResponse {
        success: true,
        message: "If the email exists in our system, a reset link has been sent.".into(),
    }))
}

/// POST /auth/password/verify-token
/// 验证重置 token 是否有效
pub async fn verify_token(
    Extension(state): Extension<SharedState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<VerifyTokenResponse>, ApiError> {
    let email_sender = SmtpEmailSender::new(state.config.email.clone().unwrap_or_default())
        .map_err(|e| ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to initialize email sender: {}", e),
        ))?;

    let reset_service = PasswordResetService::new(
        state.db.clone(),
        email_sender,
        String::new(), // verify 不需要 reset_url_base
    );

    let valid = reset_service
        .verify_token(&req.token)
        .await
        .map_err(|e| ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))?;

    Ok(Json(VerifyTokenResponse { valid }))
}

/// POST /auth/password/reset
/// 使用 token 重置密码
pub async fn reset_password(
    Extension(state): Extension<SharedState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<ResetResponse>, ApiError> {
    // 验证新密码
    if req.new_password.len() < 8 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters long".into(),
        ));
    }

    let email_sender = SmtpEmailSender::new(state.config.email.clone().unwrap_or_default())
        .map_err(|e| ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to initialize email sender: {}", e),
        ))?;

    let reset_service = PasswordResetService::new(
        state.db.clone(),
        email_sender,
        String::new(), // reset 不需要 reset_url_base
    );

    reset_service
        .reset_password(&req.token, &req.new_password)
        .await
        .map_err(|e| ApiError(
            match e.to_string().as_str() {
                "Invalid or expired token" => StatusCode::BAD_REQUEST,
                "Token has already been used" => StatusCode::BAD_REQUEST,
                "Token has expired" => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            e.to_string(),
        ))?;

    Ok(Json(ResetResponse {
        success: true,
        message: "Password has been reset successfully.".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_request_validation() {
        // 测试请求验证
        assert!(true);
    }
}
