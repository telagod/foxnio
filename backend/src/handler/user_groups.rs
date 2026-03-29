//! 用户端分组信息处理器
//!
//! 提供用户查看可用分组和费率的端点

#![allow(dead_code)]

use axum::{extract::Query, http::StatusCode, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::group::GroupService;
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct AvailableGroupsQuery {
    pub platform: Option<String>,
}

/// GET /api/v1/groups/available - 获取可用分组列表（用户端）
pub async fn list_available_groups(
    Extension(state): Extension<SharedState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<AvailableGroupsQuery>,
) -> Result<Json<Value>, ApiError> {
    let group_service = GroupService::new(state.db.clone());

    let groups = group_service
        .list_available_groups(query.platform.as_deref())
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
            "features": {
                "model_routing_enabled": g.model_routing_enabled,
                "claude_code_only": g.claude_code_only,
            },
            "created_at": g.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// GET /api/v1/groups/rates - 获取分组费率信息（用户端）
pub async fn list_group_rates(
    Extension(state): Extension<SharedState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<AvailableGroupsQuery>,
) -> Result<Json<Value>, ApiError> {
    let group_service = GroupService::new(state.db.clone());

    let rates = group_service
        .get_group_rates(query.platform.as_deref())
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": rates.iter().map(|r| json!({
            "group_id": r.group_id,
            "group_name": r.group_name,
            "platform": r.platform,
            "rate_multiplier": r.rate_multiplier,
            "models": r.models,
        })).collect::<Vec<_>>()
    })))
}
