//! 分组管理 API Handler
//!
//! 提供分组的 CRUD 操作和账号分配管理
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::group::{CreateGroupRequest, GroupService, UpdateGroupRequest};
use crate::service::permission::Permission;
use crate::service::user::Claims;

// ============ 查询参数 ============

#[derive(Debug, Deserialize)]
pub struct ListGroupsQuery {
    pub platform: Option<String>,
}

// ============ API Handlers ============

/// 列出所有分组 - 需要 GroupRead 权限
pub async fn list_groups(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListGroupsQuery>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    let groups = group_service
        .list_groups(query.platform.as_deref())
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": groups.iter().map(|g| json!({
            "id": g.id,
            "name": g.name,
            "description": g.description,
            "platform": g.platform,
            "status": g.status,
            "daily_limit_usd": g.daily_limit_usd,
            "monthly_limit_usd": g.monthly_limit_usd,
            "rate_multiplier": g.rate_multiplier,
            "model_routing_enabled": g.model_routing_enabled,
            "claude_code_only": g.claude_code_only,
            "is_exclusive": g.is_exclusive,
            "sort_order": g.sort_order,
            "account_count": g.account_count,
            "created_at": g.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// 创建分组 - 需要 GroupWrite 权限
pub async fn create_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateGroupRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    // 验证必填字段
    if req.name.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Group name is required".into(),
        ));
    }

    // 验证平台
    let valid_platforms = ["anthropic", "openai", "gemini", "droid", "antigravity"];
    if !valid_platforms.contains(&req.platform.as_str()) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("Invalid platform. Must be one of: {:?}", valid_platforms),
        ));
    }

    // 验证调度策略
    if let Some(ref policy) = req.scheduling_policy {
        let valid_policies = ["sticky", "load_balance", "scoring"];
        if !valid_policies.contains(&policy.as_str()) {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                format!("Invalid scheduling_policy. Must be one of: {:?}", valid_policies),
            ));
        }
    }

    let group_service = GroupService::new(state.db.clone());

    let group = group_service
        .create_group(req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "id": group.id,
        "name": group.name,
        "platform": group.platform,
        "status": group.status,
        "created_at": group.created_at.to_rfc3339(),
    })))
}

/// 获取分组详情 - 需要 GroupRead 权限
pub async fn get_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    let group = group_service
        .get_group(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Group not found".into()))?;

    Ok(Json(json!({
        "id": group.id,
        "name": group.name,
        "description": group.description,
        "platform": group.platform,
        "status": group.status,
        "daily_limit_usd": group.daily_limit_usd,
        "weekly_limit_usd": group.weekly_limit_usd,
        "monthly_limit_usd": group.monthly_limit_usd,
        "rate_multiplier": group.rate_multiplier,
        "model_routing": group.model_routing,
        "model_routing_enabled": group.model_routing_enabled,
        "fallback_group_id": group.fallback_group_id,
        "claude_code_only": group.claude_code_only,
        "is_exclusive": group.is_exclusive,
        "sort_order": group.sort_order,
        "account_count": group.account_count,
        "created_at": group.created_at.to_rfc3339(),
        "updated_at": group.updated_at.to_rfc3339(),
    })))
}

/// 更新分组 - 需要 GroupWrite 权限
pub async fn update_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateGroupRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    let group = group_service
        .update_group(id, req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Group not found".into()))?;

    Ok(Json(json!({
        "id": group.id,
        "name": group.name,
        "status": group.status,
        "updated_at": group.updated_at.to_rfc3339(),
    })))
}

/// 删除分组 - 需要 GroupDelete 权限
pub async fn delete_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupDelete)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    group_service
        .delete_group(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 获取分组内的账号列表 - 需要 GroupRead 权限
pub async fn get_group_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    // 先检查分组是否存在
    let _group = group_service
        .get_group(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Group not found".into()))?;

    let accounts = group_service
        .get_group_accounts(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": accounts.iter().map(|a| json!({
            "id": a.id.to_string(),
            "name": a.name,
            "provider": a.provider,
            "status": a.status,
            "priority": a.priority,
        })).collect::<Vec<_>>()
    })))
}

/// 添加账号到分组 - 需要 GroupWrite 权限
pub async fn add_account_to_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id_str = body
        .get("account_id")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing account_id".into(),
        ))?;

    let account_id = Uuid::parse_str(account_id_str)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let group_service = GroupService::new(state.db.clone());

    group_service
        .add_account_to_group(account_id, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 从分组移除账号 - 需要 GroupWrite 权限
pub async fn remove_account_from_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(params): Path<(i64, String)>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let (group_id, account_id_str) = params;

    let account_id = Uuid::parse_str(&account_id_str)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let group_service = GroupService::new(state.db.clone());

    group_service
        .remove_account_from_group(account_id, group_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 获取分组配额状态 - 需要 GroupRead 权限
pub async fn get_group_quota(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let group_service = GroupService::new(state.db.clone());

    let quota = group_service
        .check_group_quota(id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "group_id": quota.group_id,
        "group_name": quota.group_name,
        "daily_limit": quota.daily_limit,
        "daily_used": quota.daily_used,
        "weekly_limit": quota.weekly_limit,
        "monthly_limit": quota.monthly_limit,
        "monthly_used": quota.monthly_used,
        "is_over_limit": quota.is_over_limit,
    })))
}

/// 更新分组模型路由配置 - 需要 GroupWrite 权限
pub async fn update_group_model_routing(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::GroupWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let model_routing = body
        .get("model_routing")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid model_routing format".into(),
        ))?;

    let enabled = body
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let group_service = GroupService::new(state.db.clone());

    let update_req = UpdateGroupRequest {
        name: None,
        description: None,
        status: None,
        daily_limit_usd: None,
        weekly_limit_usd: None,
        monthly_limit_usd: None,
        rate_multiplier: None,
        fallback_group_id: None,
        model_routing: Some(model_routing),
        model_routing_enabled: Some(enabled),
        claude_code_only: None,
        fallback_group_id_on_invalid_request: None,
        is_exclusive: None,
        sort_order: None,
        scheduling_policy: None,
    };

    let group = group_service
        .update_group(id, update_req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Group not found".into()))?;

    Ok(Json(json!({
        "id": group.id,
        "model_routing_enabled": group.model_routing_enabled,
        "model_routing": group.model_routing,
    })))
}
