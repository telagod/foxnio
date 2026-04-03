//! 验证相关处理器
//!
//! 提供验证码发送、优惠码验证、邀请码验证等端点

#![allow(dead_code)]

use axum::{http::StatusCode, Extension, Json};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use super::ApiError;
use crate::entity::users;
use crate::gateway::SharedState;
use crate::service::email::SmtpEmailSender;
use crate::service::promo_code::{PromoCodeService, VerifyPromoCodeRequest};
use crate::service::redeem_code::{RedeemCodeService, RedeemType};
use crate::utils::validator::is_valid_email;

const VERIFY_CODE_TTL_SECS: u64 = 300;
const VERIFY_CODE_COOLDOWN_SECS: u64 = 60;

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

#[derive(Debug, Serialize, Deserialize)]
struct StoredVerifyCode {
    code: String,
    email: String,
    verify_type: String,
}

#[derive(Debug, Clone, Copy)]
enum VerifyType {
    Register,
    ResetPassword,
    ChangeEmail,
}

impl VerifyType {
    fn parse(value: &str) -> Result<Self, ApiError> {
        match value {
            "register" => Ok(Self::Register),
            "reset_password" => Ok(Self::ResetPassword),
            "change_email" => Ok(Self::ChangeEmail),
            _ => Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Invalid verify type. Must be one of: [\"register\", \"reset_password\", \"change_email\"]".into(),
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::ResetPassword => "reset_password",
            Self::ChangeEmail => "change_email",
        }
    }
}

/// POST /api/v1/auth/send-verify-code - 发送验证码
pub async fn send_verify_code(
    Extension(state): Extension<SharedState>,
    Json(req): Json<SendVerifyCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let email = req.email.trim().to_lowercase();
    if !is_valid_email(&email) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid email format".into(),
        ));
    }

    let verify_type = VerifyType::parse(req.verify_type.trim().to_lowercase().as_str())?;
    let email_exists = user_exists(&state, &email).await?;

    match verify_type {
        VerifyType::Register | VerifyType::ChangeEmail if email_exists => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Email already registered".into(),
            ));
        }
        VerifyType::ResetPassword if !email_exists => {
            return Ok(Json(json!({
                "success": true,
                "message": "If the email exists, a verification code has been sent",
                "expires_in": VERIFY_CODE_TTL_SECS,
            })));
        }
        _ => {}
    }

    let verify_key = build_verify_key(&email, verify_type);
    let cooldown_key = build_cooldown_key(&email, verify_type);

    if state
        .redis
        .exists(&cooldown_key)
        .await
        .map_err(internal_error)?
    {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Verification code requested too frequently".into(),
        ));
    }

    let code = generate_verify_code();
    let expires_in = VERIFY_CODE_TTL_SECS as i64;
    let payload = serde_json::to_string(&StoredVerifyCode {
        code: code.clone(),
        email: email.clone(),
        verify_type: verify_type.as_str().to_string(),
    })
    .map_err(internal_error)?;

    state
        .redis
        .set(
            &verify_key,
            &payload,
            Some(Duration::from_secs(VERIFY_CODE_TTL_SECS)),
        )
        .await
        .map_err(internal_error)?;

    state
        .redis
        .set(
            &cooldown_key,
            "1",
            Some(Duration::from_secs(VERIFY_CODE_COOLDOWN_SECS)),
        )
        .await
        .map_err(internal_error)?;

    let email_sender = SmtpEmailSender::new(state.config.email.clone().unwrap_or_default())
        .map_err(internal_error)?;
    if let Err(error) =
        email_sender.send_verification_code_email(&email, verify_type.as_str(), &code, expires_in)
    {
        let _ = state.redis.del(&verify_key).await;
        let _ = state.redis.del(&cooldown_key).await;
        return Err(internal_error(error));
    }

    Ok(Json(json!({
        "success": true,
        "message": "Verification code sent",
        "expires_in": expires_in,
    })))
}

/// POST /api/v1/auth/validate-promo-code - 验证优惠码
pub async fn validate_promo_code(
    Extension(state): Extension<SharedState>,
    Json(req): Json<ValidatePromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let code = req.code.trim();
    if code.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Promo code is required".into(),
        ));
    }

    let result = PromoCodeService::verify(
        &state.db,
        VerifyPromoCodeRequest {
            code: code.to_string(),
        },
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(json!({
        "valid": result.valid,
        "code": code,
        "bonus_amount": result.bonus_amount,
        "message": result.message,
    })))
}

/// POST /api/v1/auth/validate-invitation-code - 验证邀请码
pub async fn validate_invitation_code(
    Extension(state): Extension<SharedState>,
    Json(req): Json<ValidateInvitationCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let code = req.code.trim();
    if code.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invitation code is required".into(),
        ));
    }

    let redeem_service = RedeemCodeService::new(state.db.clone());
    let preview = redeem_service
        .preview_code(code)
        .await
        .map_err(internal_error)?;

    Ok(Json(json!({
        "valid": preview.as_ref().map(|p| p.valid).unwrap_or(false),
        "code": code,
        "redeem_type": preview.as_ref().map(|p| p.code_type.to_string()),
        "benefits": preview.as_ref().map(invitation_benefits),
        "expires_at": preview.as_ref().and_then(|p| p.expires_at.map(|t| t.to_rfc3339())),
        "message": preview
            .as_ref()
            .and_then(|p| p.message.clone())
            .or_else(|| Some("Invitation code not found".to_string()).filter(|_| preview.is_none())),
    })))
}

/// 生成 6 位数字验证码
fn generate_verify_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1000000))
}

fn invitation_benefits(preview: &crate::service::redeem_code::RedeemCodePreview) -> Value {
    match preview.code_type {
        RedeemType::Balance => json!({ "bonus_balance": preview.value }),
        RedeemType::Subscription => json!({ "subscription_days": preview.value as i64 }),
        RedeemType::Quota => json!({ "bonus_quota": preview.value as i64 }),
    }
}

async fn user_exists(state: &SharedState, email: &str) -> Result<bool, ApiError> {
    users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(&state.db)
        .await
        .map(|user| user.is_some())
        .map_err(internal_error)
}

fn build_verify_key(email: &str, verify_type: VerifyType) -> String {
    format!("verify_code:{}:{}", verify_type.as_str(), email)
}

fn build_cooldown_key(email: &str, verify_type: VerifyType) -> String {
    format!("verify_code_cooldown:{}:{}", verify_type.as_str(), email)
}

fn internal_error(error: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
