//! 卡密兑换 API Handler

#![allow(dead_code)]

use axum::{extract::Extension, http::StatusCode, Json};
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::redeem_code::{GenerateCodesRequest, RedeemCodeRequest, RedeemCodeService};
use crate::service::user::Claims;

/// 用户兑换卡密
pub async fn redeem_code(
    Extension(state): Extension<SharedState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<RedeemCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let redeem_service = RedeemCodeService::new(state.db.clone());

    let result = redeem_service
        .redeem(req)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "success": result.success,
        "type": result.code_type.to_string(),
        "value": result.value,
        "message": result.message,
        "redeemed_at": result.redeemed_at.to_rfc3339(),
    })))
}

/// 用户兑换历史
pub async fn get_redemption_history(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let redeem_service = RedeemCodeService::new(state.db.clone());

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let redemptions = redeem_service
        .get_user_redemptions(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": redemptions.iter().map(|r| json!({
            "id": r.id,
            "code": r.code,
            "type": r.code_type.to_string(),
            "value": r.value,
            "status": r.status.to_string(),
            "redeemed_at": r.used_at.map(|t| t.to_rfc3339()),
            "created_at": r.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// 管理员批量生成卡密
pub async fn admin_generate_codes(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<GenerateCodesRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let redeem_service = RedeemCodeService::new(state.db.clone());

    let codes = redeem_service
        .generate_batch(req)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "count": codes.len(),
        "data": codes.iter().map(|c| json!({
            "id": c.id,
            "code": c.code,
            "type": c.code_type.to_string(),
            "value": c.value,
            "status": c.status.to_string(),
            "expires_at": c.expires_at.map(|t| t.to_rfc3339()),
            "created_at": c.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// 管理员获取卡密统计
pub async fn admin_get_redeem_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let redeem_service = RedeemCodeService::new(state.db.clone());

    let stats = redeem_service
        .get_stats()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "total_codes": stats.total_codes,
        "unused_codes": stats.unused_codes,
        "used_codes": stats.used_codes,
        "expired_codes": stats.expired_codes,
        "total_value": stats.total_value,
        "used_value": stats.used_value,
    })))
}

/// 管理员取消卡密
pub async fn admin_cancel_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let code_id = body
        .get("code_id")
        .and_then(|v| v.as_i64())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing or invalid code_id".into(),
        ))?;

    let redeem_service = RedeemCodeService::new(state.db.clone());

    redeem_service
        .cancel(code_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}
