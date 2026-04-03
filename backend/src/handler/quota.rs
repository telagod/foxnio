//! API Key 配额管理 API Handler

#![allow(dead_code)]

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::permission::Permission;
use crate::service::quota::{QuotaService, UpdateQuotaRequest};
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct UpdateUserQuotaRequest {
    pub quota_limit: Option<f64>,
    pub rate_limit_5h: Option<f64>,
    pub rate_limit_1d: Option<f64>,
    pub rate_limit_7d: Option<f64>,
    pub ip_whitelist: Option<Vec<String>>,
    pub ip_blacklist: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct QuotaResponse {
    pub user_id: Uuid,
    pub total_quota: f64,
    pub used_quota: f64,
    pub remaining_quota: f64,
    pub rate_limits: Option<serde_json::Value>,
}

/// 获取用户配额信息
pub async fn get_user_quota(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let service = QuotaService::new(state.db.clone());

    match service.get_quota_config(user_id).await {
        Ok(Some(config)) => Ok(Json(json!({
            "user_id": user_id,
            "total_quota": config.quota_limit,
            "used_quota": config.quota_used,
            "remaining_quota": config.quota_limit - config.quota_used,
            "rate_limits": config.rate_limits,
            "ip_whitelist": config.ip_whitelist,
            "ip_blacklist": config.ip_blacklist,
        }))),
        Ok(None) => Ok(Json(json!({
            "user_id": user_id,
            "total_quota": null,
            "used_quota": 0.0,
            "remaining_quota": null,
            "message": "No quota configured"
        }))),
        Err(e) => Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// 更新用户配额
pub async fn update_user_quota(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<UpdateUserQuotaRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let service = QuotaService::new(state.db.clone());

    let req = UpdateQuotaRequest {
        quota_limit: body.quota_limit,
        rate_limit_5h: body.rate_limit_5h,
        rate_limit_1d: body.rate_limit_1d,
        rate_limit_7d: body.rate_limit_7d,
        ip_whitelist: body.ip_whitelist,
        ip_blacklist: body.ip_blacklist,
    };

    let config = service
        .update_quota(user_id, req)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "quota": {
            "total_quota": config.quota_limit,
            "used_quota": config.quota_used,
            "remaining_quota": config.quota_limit - config.quota_used,
        }
    })))
}

/// 重置用户配额
pub async fn reset_user_quota(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let service = QuotaService::new(state.db.clone());

    service
        .reset_quota(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        json!({ "success": true, "message": "Quota reset successfully" }),
    ))
}

/// 获取配额使用历史
pub async fn get_quota_history(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let service = QuotaService::new(state.db.clone());

    let history = service
        .get_usage_history(
            user_id,
            chrono::Utc::now() - chrono::Duration::days(30),
            chrono::Utc::now(),
        )
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "object": "list", "data": history })))
}

/// 获取配额统计
pub async fn get_quota_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let service = QuotaService::new(state.db.clone());
    let stats = service
        .get_stats()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "total_users": stats.total_users,
        "active_subscription_users": stats.active_subscription_users,
        "total_quota": stats.total_quota,
        "total_used": stats.total_used,
        "total_remaining": stats.total_remaining,
        "average_usage": stats.average_usage,
        "utilization_rate": stats.utilization_rate,
    })))
}

/// 检查配额 (内部 API)
pub async fn check_quota(
    Extension(state): Extension<SharedState>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Value>, ApiError> {
    let estimated_cost: f64 = body
        .get("estimated_cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let service = QuotaService::new(state.db.clone());

    let result = service
        .check_quota(user_id, estimated_cost)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!(result)))
}

/// 消费配额 (内部 API)
pub async fn consume_quota(
    Extension(state): Extension<SharedState>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<Value>, ApiError> {
    let amount: f64 = body.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let model: &str = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let tokens: i64 = body.get("tokens").and_then(|v| v.as_i64()).unwrap_or(0);

    let service = QuotaService::new(state.db.clone());

    service
        .consume_quota(user_id, amount, model, tokens)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}
