//! 用户端订阅管理处理器
//!
//! 提供用户查看订阅列表、订阅详情等端点

#![allow(dead_code)]

use axum::{http::StatusCode, Extension, Json};
use serde_json::{json, Value};
use uuid::Uuid;

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::subscription::{SubscriptionConfig, SubscriptionService};
use crate::service::user::Claims;

/// GET /api/v1/subscriptions - 获取用户订阅列表
pub async fn list_user_subscriptions(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {}", e)))?;

    let service = SubscriptionService::new(SubscriptionConfig::default());

    // 获取用户配额信息
    let quota = service
        .get_user_quota(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": [{
            "id": Uuid::nil().to_string(),
            "plan_name": quota.plan_name,
            "plan_type": "default",
            "status": "active",
            "quota_limit": quota.monthly_tokens_limit,
            "quota_used": quota.monthly_tokens_used,
            "quota_remaining": quota.monthly_remaining(),
            "daily_limit": quota.daily_requests_limit,
            "daily_used": quota.daily_requests_used,
            "auto_renew": false,
            "created_at": chrono::Utc::now().to_rfc3339(),
        }]
    })))
}

/// GET /api/v1/subscriptions/:id - 获取订阅详情
pub async fn get_subscription_detail(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {}", e)))?;

    let _subscription_id = Uuid::parse_str(&id).map_err(|e| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("Invalid subscription ID: {}", e),
        )
    })?;

    let service = SubscriptionService::new(SubscriptionConfig::default());

    // 获取用户配额信息
    let quota = service
        .get_user_quota(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "id": id,
        "plan_name": quota.plan_name,
        "plan_type": "default",
        "status": "active",
        "quota_limit": quota.monthly_tokens_limit,
        "quota_used": quota.monthly_tokens_used,
        "quota_remaining": quota.monthly_remaining(),
        "daily_limit": quota.daily_requests_limit,
        "daily_used": quota.daily_requests_used,
        "allowed_models": quota.allowed_models,
        "priority": quota.priority,
        "rate_limit": quota.rate_limit,
        "auto_renew": false,
        "created_at": chrono::Utc::now().to_rfc3339(),
    })))
}
