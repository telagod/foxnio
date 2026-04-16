//! 用户端公告处理器
//!
//! 提供用户查看公告列表的端点

#![allow(dead_code)]

use axum::{extract::Query, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct UserAnnouncementsQuery {
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

/// GET /api/v1/announcements - 获取用户可见公告列表（用户端）
pub async fn list_user_announcements(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<UserAnnouncementsQuery>,
) -> Result<Json<Value>, ApiError> {
    // NOTE: 从数据库获取公告列表
    // 目前返回空列表

    Ok(Json(json!({
        "object": "list",
        "total": 0,
        "page": query.page,
        "page_size": query.page_size,
        "data": []
    })))
}
