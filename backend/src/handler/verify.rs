//! 验证相关处理器
//!
//! 提供验证码发送、优惠码验证、邀请码验证等端点

#![allow(dead_code)]

use axum::{http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::SharedState;

/// 发送验证码请求
#[derive(Debug, Deserialize)]
pub struct SendVerifyCodeRequest {
    pub email: String,
    #[serde(rename = "type")]
    pub verify_type: String, // "register", "reset_password", "change_email"
}

/// 发送验证码响应
#[derive(Debug, Serialize)]
pub struct SendVerifyCodeResponse {
    pub message: String,
    pub expires_in: i64,
}

/// 验证优惠码请求
#[derive(Debug, Deserialize)]
pub struct ValidatePromoCodeRequest {
    pub code: String,
}

/// 验证邀请码请求
#[derive(Debug, Deserialize)]
pub struct ValidateInvitationCodeRequest {
    pub code: String,
}

/// POST /api/v1/auth/send-verify-code - 发送验证码
pub async fn send_verify_code(
    Extension(_state): Extension<SharedState>,
    Json(req): Json<SendVerifyCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    // 验证邮箱格式
    if !req.email.contains('@') {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid email format".into(),
        ));
    }

    // 验证类型
    let valid_types = ["register", "reset_password", "change_email"];
    if !valid_types.contains(&req.verify_type.as_str()) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("Invalid verify type. Must be one of: {:?}", valid_types),
        ));
    }

    // 生成验证码
    let code = generate_verify_code();
    let expires_in = 300i64; // 5分钟

    // TODO: 存储验证码到 Redis 并发送邮件
    tracing::info!("Generated verify code for {}: {}", req.email, code);

    Ok(Json(json!({
        "success": true,
        "message": "Verification code sent",
        "expires_in": expires_in,
    })))
}

/// POST /api/v1/auth/validate-promo-code - 验证优惠码
pub async fn validate_promo_code(
    Extension(_state): Extension<SharedState>,
    Json(req): Json<ValidatePromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    // TODO: 实现优惠码验证
    let code = req.code.trim();

    // 模拟验证
    if code.starts_with("PROMO-") {
        Ok(Json(json!({
            "valid": true,
            "code": code,
            "discount_type": "percent",
            "discount_value": 10,
            "description": "10% discount",
        })))
    } else {
        Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid promo code".into(),
        ))
    }
}

/// POST /api/v1/auth/validate-invitation-code - 验证邀请码
pub async fn validate_invitation_code(
    Extension(_state): Extension<SharedState>,
    Json(req): Json<ValidateInvitationCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    // 邀请码验证逻辑
    let code = req.code.trim();

    // 检查格式（邀请码格式：INV-XXXXXXXX）
    if !code.starts_with("INV-") || code.len() != 12 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid invitation code format".into(),
        ));
    }

    // TODO: 查询数据库验证邀请码

    // 模拟验证成功
    Ok(Json(json!({
        "valid": true,
        "code": code,
        "benefits": {
            "bonus_quota": 100.0,
            "discount_percent": 10,
        }
    })))
}

/// 生成 6 位数字验证码
fn generate_verify_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1000000))
}
