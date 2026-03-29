//! 管理端分组扩展处理器
//!
//! 提供分组使用摘要、容量摘要、统计等扩展端点

#![allow(dead_code)]

use axum::{extract::Path, http::StatusCode, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::group::{GroupService, SortOrderItem};
use crate::service::permission::Permission;
use crate::service::user::Claims;

/// 更新排序请求
#[derive(Debug, Deserialize)]
pub struct UpdateSortOrderRequest {
    pub orders: Vec<SortOrderItem>,
}

/// GET /api/v1/admin/groups/usage-summary - 获取分组使用摘要
pub async fn get_groups_usage_summary(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let summary = group_service
        .get_usage_summary()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": summary.iter().map(|s| json!({
            "group_id": s.group_id,
            "group_name": s.group_name,
            "platform": s.platform,
            "daily_used_usd": s.daily_used_usd,
            "daily_limit_usd": s.daily_limit_usd,
            "daily_usage_percent": if s.daily_limit_usd > 0.0 {
                (s.daily_used_usd / s.daily_limit_usd * 100.0).round() as i32
            } else {
                0
            },
            "monthly_used_usd": s.monthly_used_usd,
            "monthly_limit_usd": s.monthly_limit_usd,
            "monthly_usage_percent": if s.monthly_limit_usd > 0.0 {
                (s.monthly_used_usd / s.monthly_limit_usd * 100.0).round() as i32
            } else {
                0
            },
            "account_count": s.account_count,
            "active_account_count": s.active_account_count,
        })).collect::<Vec<_>>()
    })))
}

/// GET /api/v1/admin/groups/capacity-summary - 获取分组容量摘要
pub async fn get_groups_capacity_summary(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let summary = group_service
        .get_capacity_summary()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": summary.iter().map(|s| json!({
            "group_id": s.group_id,
            "group_name": s.group_name,
            "platform": s.platform,
            "total_capacity": s.total_capacity,
            "used_capacity": s.used_capacity,
            "available_capacity": s.total_capacity - s.used_capacity,
            "capacity_percent": if s.total_capacity > 0.0 {
                (s.used_capacity / s.total_capacity * 100.0).round() as i32
            } else {
                0
            },
            "account_count": s.account_count,
        })).collect::<Vec<_>>()
    })))
}

/// PUT /api/v1/admin/groups/sort-order - 更新分组排序
pub async fn update_groups_sort_order(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<UpdateSortOrderRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    group_service
        .update_sort_order(&req.orders)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "updated_count": req.orders.len(),
    })))
}

/// GET /api/v1/admin/groups/:id/stats - 获取分组统计
pub async fn get_group_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let stats = group_service
        .get_group_stats(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Group not found".into()))?;

    Ok(Json(json!({
        "group_id": id,
        "stats": stats,
    })))
}

/// GET /api/v1/admin/groups/:id/rate-multipliers - 获取分组费率倍数
pub async fn get_group_rate_multipliers(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let multipliers = group_service
        .get_rate_multipliers(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "group_id": id,
        "multipliers": multipliers,
    })))
}

/// GET /api/v1/admin/groups/:id/api-keys - 获取分组 API Keys
pub async fn get_group_api_keys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let keys = group_service
        .get_group_api_keys(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "group_id": id,
        "api_keys": keys.iter().map(|k| json!({
            "id": k.id.to_string(),
            "key": k.key_masked,
            "name": k.name,
            "status": k.status,
            "created_at": k.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// GET /api/v1/admin/groups/all - 获取所有分组（简化列表）
pub async fn list_all_groups(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());
    let groups = group_service
        .list_all_groups()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": groups.iter().map(|g| json!({
            "id": g.id,
            "name": g.name,
            "platform": g.platform,
            "status": g.status,
            "sort_order": g.sort_order,
        })).collect::<Vec<_>>()
    })))
}
