//! 代理管理 API Handler

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
use crate::service::permission::Permission;
use crate::service::proxy::{CreateProxyRequest, ProxyService, UpdateProxyRequest};
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub enabled_only: Option<bool>,
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

fn default_true() -> bool {
    true
}

/// 列出所有代理
pub async fn list_proxies(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let proxies = ProxyService::list(
        db,
        query.enabled_only.unwrap_or(false),
        query.page,
        query.page_size,
    )
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": proxies
    })))
}

/// 创建代理
pub async fn create_proxy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateProxyRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let proxy = ProxyService::create(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(proxy)))
}

/// 获取代理详情
pub async fn get_proxy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let proxy = ProxyService::get_by_id(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Proxy not found".into()))?;

    Ok(Json(json!(proxy)))
}

/// 更新代理
pub async fn update_proxy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateProxyRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let proxy = ProxyService::update(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Proxy not found".into()))?;

    Ok(Json(json!(proxy)))
}

/// 删除代理
pub async fn delete_proxy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = ProxyService::delete(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(json!({ "success": true, "message": "Proxy deleted" })))
    } else {
        Err(ApiError(StatusCode::NOT_FOUND, "Proxy not found".into()))
    }
}

/// 检查代理健康状态
pub async fn check_proxy_health(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let result = ProxyService::check_health(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "proxy_id": id,
        "healthy": result.healthy,
        "latency_ms": result.latency_ms,
        "error": result.error
    })))
}

/// 批量检查代理健康状态
pub async fn check_all_proxies_health(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let results = ProxyService::check_all_health(db)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": results
    })))
}

/// 测试代理 (路由别名)
pub async fn test_proxy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_proxy_health(Extension(state), Extension(claims), Path(id)).await
}

/// 批量测试代理 (路由别名)
pub async fn test_all_proxies(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_all_proxies_health(Extension(state), Extension(claims)).await
}

/// 获取代理质量
pub async fn get_proxy_quality(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let proxy = ProxyService::get_by_id(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Proxy not found".into()))?;

    // TODO: 实现质量评分
    Ok(Json(json!({
        "proxy_id": id,
        "name": proxy.name,
        "quality_score": 100,
        "latency_ms": 0,
        "success_rate": 1.0,
        "last_check": proxy.last_check_at
    })))
}
