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

/// 订阅项响应
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SubscriptionItem {
    /// 订阅ID
    pub id: String,
    /// 计划名称
    pub plan_name: String,
    /// 计划类型
    pub plan_type: String,
    /// 订阅状态
    pub status: String,
    /// 配额上限
    pub quota_limit: i64,
    /// 已使用配额
    pub quota_used: i64,
    /// 剩余配额
    pub quota_remaining: i64,
    /// 每日请求上限
    pub daily_limit: i64,
    /// 今日已使用
    pub daily_used: i64,
    /// 是否自动续费
    pub auto_renew: bool,
    /// 创建时间
    pub created_at: String,
}

/// 订阅列表响应
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SubscriptionListResponse {
    /// 对象类型
    pub object: String,
    /// 订阅列表
    pub data: Vec<SubscriptionItem>,
}

/// 订阅详情响应
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SubscriptionDetailResponse {
    /// 订阅ID
    pub id: String,
    /// 计划名称
    pub plan_name: String,
    /// 计划类型
    pub plan_type: String,
    /// 订阅状态
    pub status: String,
    /// 配额上限
    pub quota_limit: i64,
    /// 已使用配额
    pub quota_used: i64,
    /// 剩余配额
    pub quota_remaining: i64,
    /// 每日请求上限
    pub daily_limit: i64,
    /// 今日已使用
    pub daily_used: i64,
    /// 允许的模型列表
    pub allowed_models: Vec<String>,
    /// 优先级
    pub priority: i32,
    /// 速率限制
    pub rate_limit: i32,
    /// 是否自动续费
    pub auto_renew: bool,
    /// 创建时间
    pub created_at: String,
}

/// GET /api/v1/subscriptions - 获取用户订阅列表
///
/// 获取当前用户的所有订阅信息，包括配额使用情况。
///
/// ## 认证
/// 需要 Bearer Token 认证
#[utoipa::path(
    get,
    path = "/api/v1/subscriptions",
    tag = "订阅",
    responses(
        (status = 200, description = "获取成功", body = SubscriptionListResponse),
        (status = 401, description = "未认证"),
        (status = 500, description = "服务器内部错误")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
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
///
/// 获取指定订阅的详细信息，包括允许的模型、优先级等。
///
/// ## 认证
/// 需要 Bearer Token 认证
#[utoipa::path(
    get,
    path = "/api/v1/subscriptions/{id}",
    tag = "订阅",
    params(
        ("id" = String, Path, description = "订阅ID")
    ),
    responses(
        (status = 200, description = "获取成功", body = SubscriptionDetailResponse),
        (status = 400, description = "无效的订阅ID"),
        (status = 401, description = "未认证"),
        (status = 500, description = "服务器内部错误")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
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
