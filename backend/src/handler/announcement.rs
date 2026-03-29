//! 公告管理 API Handler

#![allow(dead_code)]

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::announcement::{
    AnnouncementService, CreateAnnouncementRequest, UpdateAnnouncementRequest,
};
use crate::service::permission::Permission;
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct ListAnnouncementsQuery {
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

fn default_page() -> u64 {
    0
}
fn default_page_size() -> u64 {
    20
}

/// 列出所有公告
pub async fn list_announcements(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListAnnouncementsQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let announcements = AnnouncementService::list(db, query.status, query.page, query.page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": announcements
    })))
}

/// 创建公告
pub async fn create_announcement(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(mut body): Json<CreateAnnouncementRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    // Set created_by from claims
    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;
    body.created_by = Some(user_id);

    let db = &state.db;
    let announcement = AnnouncementService::create(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(announcement)))
}

/// 获取公告详情
pub async fn get_announcement(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let announcement = AnnouncementService::get_by_id(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Announcement not found".into()))?;

    Ok(Json(json!(announcement)))
}

/// 更新公告
pub async fn update_announcement(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(mut body): Json<UpdateAnnouncementRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    // Set updated_by from claims
    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;
    body.updated_by = Some(user_id);

    let db = &state.db;
    let announcement = AnnouncementService::update(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Announcement not found".into()))?;

    Ok(Json(json!(announcement)))
}

/// 删除公告
pub async fn delete_announcement(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = AnnouncementService::delete(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(
            json!({ "success": true, "message": "Announcement deleted" }),
        ))
    } else {
        Err(ApiError(
            StatusCode::NOT_FOUND,
            "Announcement not found".into(),
        ))
    }
}

/// 标记公告为已读
pub async fn mark_announcement_read(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let db = &state.db;

    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    AnnouncementService::mark_as_read(db, id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 获取用户未读公告
pub async fn get_unread_announcements(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let db = &state.db;

    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    let announcements = AnnouncementService::get_active_for_user(db, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter to only unread
    let unread: Vec<_> = announcements
        .into_iter()
        .filter(|a| a.is_read != Some(true))
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": unread
    })))
}
