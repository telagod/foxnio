//! TOTP 两步验证处理器

use axum::{
    Extension,
    Json,
    http::StatusCode,
    extract::Path,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::gateway::SharedState;
use crate::service::{UserService, TotpService};
use super::ApiError;

// ============================================================================
// 请求/响应结构体
// ============================================================================

/// 启用 TOTP 响应
#[derive(Debug, Serialize)]
pub struct EnableTotpResponse {
    pub secret: String,
    pub qr_code_url: String,
    pub backup_codes: Vec<String>,
    pub message: String,
}

/// 确认启用 TOTP 请求
#[derive(Debug, Deserialize)]
pub struct ConfirmTotpRequest {
    pub code: String,
}

/// 禁用 TOTP 请求
#[derive(Debug, Deserialize)]
pub struct DisableTotpRequest {
    pub code: String,
}

/// TOTP 验证请求
#[derive(Debug, Deserialize)]
pub struct VerifyTotpRequest {
    pub code: String,
}

/// TOTP 登录请求
#[derive(Debug, Deserialize)]
pub struct TotpLoginRequest {
    pub temp_token: String,
    pub code: String,
}

/// 备用码登录请求
#[derive(Debug, Deserialize)]
pub struct BackupCodeLoginRequest {
    pub temp_token: String,
    pub backup_code: String,
}

/// TOTP 状态响应
#[derive(Debug, Serialize)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub has_secret: bool,
    pub backup_codes_remaining: usize,
}

/// 重新生成备用码请求
#[derive(Debug, Deserialize)]
pub struct RegenerateBackupCodesRequest {
    pub code: String,
}

/// 重新生成备用码响应
#[derive(Debug, Serialize)]
pub struct RegenerateBackupCodesResponse {
    pub backup_codes: Vec<String>,
    pub message: String,
}

/// TOTP 登录响应
#[derive(Debug, Serialize)]
pub struct TotpLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_in: i64,
    pub refresh_token_expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub totp_enabled: bool,
}

// ============================================================================
// TOTP 端点处理器
// ============================================================================

/// 启用 TOTP 两步验证
/// 
/// POST /api/v1/auth/totp/enable
/// 
/// 返回：
/// - secret: TOTP 密钥（Base32 编码）
/// - qr_code_url: QR 码 URL（Data URL 格式）
/// - backup_codes: 10 个备用码
pub async fn enable_totp(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<EnableTotpResponse>, ApiError> {
    // 验证不是临时 token
    if claims.is_temp {
        return Err(ApiError(StatusCode::FORBIDDEN, "Cannot enable TOTP with temporary token".into()));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let setup = user_service.enable_totp(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(EnableTotpResponse {
        secret: setup.secret,
        qr_code_url: setup.qr_code_url,
        backup_codes: setup.backup_codes,
        message: "TOTP setup initiated. Please verify with a code to complete setup.".to_string(),
    }))
}

/// 确认启用 TOTP（验证代码后正式启用）
/// 
/// POST /api/v1/auth/totp/confirm
pub async fn confirm_enable_totp(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Json(req): Json<ConfirmTotpRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 验证不是临时 token
    if claims.is_temp {
        return Err(ApiError(StatusCode::FORBIDDEN, "Cannot confirm TOTP with temporary token".into()));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    user_service.confirm_enable_totp(user_id, &req.code)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "TOTP two-factor authentication has been enabled successfully"
    })))
}

/// 禁用 TOTP 两步验证
/// 
/// POST /api/v1/auth/totp/disable
pub async fn disable_totp(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Json(req): Json<DisableTotpRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 验证不是临时 token
    if claims.is_temp {
        return Err(ApiError(StatusCode::FORBIDDEN, "Cannot disable TOTP with temporary token".into()));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    user_service.disable_totp(user_id, &req.code)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "TOTP two-factor authentication has been disabled"
    })))
}

/// 验证 TOTP 代码
/// 
/// POST /api/v1/auth/totp/verify
pub async fn verify_totp(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Json(req): Json<VerifyTotpRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let is_valid = user_service.verify_totp(user_id, &req.code)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "valid": is_valid,
        "message": if is_valid { "TOTP code is valid" } else { "Invalid TOTP code" }
    })))
}

/// 获取 TOTP 状态
/// 
/// GET /api/v1/auth/totp/status
pub async fn get_totp_status(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<TotpStatusResponse>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let status = user_service.get_totp_status(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TotpStatusResponse {
        enabled: status.enabled,
        has_secret: status.has_secret,
        backup_codes_remaining: status.backup_codes_remaining,
    }))
}

/// 重新生成备用码
/// 
/// POST /api/v1/auth/totp/backup-codes/regenerate
pub async fn regenerate_backup_codes(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Json(req): Json<RegenerateBackupCodesRequest>,
) -> Result<Json<RegenerateBackupCodesResponse>, ApiError> {
    // 验证不是临时 token
    if claims.is_temp {
        return Err(ApiError(StatusCode::FORBIDDEN, "Cannot regenerate backup codes with temporary token".into()));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let backup_codes = user_service.regenerate_backup_codes(user_id, &req.code)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(RegenerateBackupCodesResponse {
        backup_codes,
        message: "New backup codes generated. Store them securely.".to_string(),
    }))
}

/// TOTP 登录验证
/// 
/// POST /api/v1/auth/totp/login
/// 
/// 使用临时 token + TOTP 代码完成登录
pub async fn totp_login(
    Extension(state): Extension<SharedState>,
    Json(req): Json<TotpLoginRequest>,
) -> Result<Json<TotpLoginResponse>, ApiError> {
    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let (user, token_pair) = user_service.login_with_totp(
        &req.temp_token,
        &req.code,
        None, // TODO: 从请求头获取 user_agent
        None, // TODO: 从连接信息获取 ip_address
    ).await
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(Json(TotpLoginResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        access_token_expires_in: token_pair.access_token_expires_in,
        refresh_token_expires_in: token_pair.refresh_token_expires_in,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
            totp_enabled: user.totp_enabled,
        },
    }))
}

/// 备用码登录
/// 
/// POST /api/v1/auth/totp/backup-login
/// 
/// 使用临时 token + 备用码完成登录
pub async fn backup_code_login(
    Extension(state): Extension<SharedState>,
    Json(req): Json<BackupCodeLoginRequest>,
) -> Result<Json<TotpLoginResponse>, ApiError> {
    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let (user, token_pair) = user_service.login_with_backup_code(
        &req.temp_token,
        &req.backup_code,
        None, // TODO: 从请求头获取 user_agent
        None, // TODO: 从连接信息获取 ip_address
    ).await
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(Json(TotpLoginResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        access_token_expires_in: token_pair.access_token_expires_in,
        refresh_token_expires_in: token_pair.refresh_token_expires_in,
        user: UserInfo {
            id: user.id.to_string(),
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
            totp_enabled: user.totp_enabled,
        },
    }))
}
