//! 错误透传规则管理 API Handler

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
use crate::service::error_passthrough_rule::{
    CreateErrorRuleRequest, ErrorPassthroughRuleService, UpdateErrorRuleRequest,
};
use crate::service::permission::Permission;
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub enabled_only: bool,
}

/// 列出所有错误透传规则
pub async fn list_rules(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let rules = ErrorPassthroughRuleService::list(db, query.enabled_only)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": rules
    })))
}

/// 创建错误透传规则
pub async fn create_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateErrorRuleRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let rule = ErrorPassthroughRuleService::create(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(rule)))
}

/// 获取规则详情
pub async fn get_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    // TODO: implement get_by_id in service
    let rules = ErrorPassthroughRuleService::list(db, false)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rule = rules
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Rule not found".into()))?;

    Ok(Json(json!(rule)))
}

/// 更新错误透传规则
pub async fn update_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateErrorRuleRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let rule = ErrorPassthroughRuleService::update(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Rule not found".into()))?;

    Ok(Json(json!(rule)))
}

/// 删除错误透传规则
pub async fn delete_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = ErrorPassthroughRuleService::delete(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(json!({ "success": true, "message": "Rule deleted" })))
    } else {
        Err(ApiError(StatusCode::NOT_FOUND, "Rule not found".into()))
    }
}

// 路由别名函数
pub async fn create_error_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateErrorRuleRequest>,
) -> Result<Json<Value>, ApiError> {
    create_rule(Extension(state), Extension(claims), Json(body)).await
}

pub async fn list_error_rules(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    list_rules(Extension(state), Extension(claims), Query(query)).await
}

pub async fn update_error_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateErrorRuleRequest>,
) -> Result<Json<Value>, ApiError> {
    update_rule(Extension(state), Extension(claims), Path(id), Json(body)).await
}

pub async fn delete_error_rule(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    delete_rule(Extension(state), Extension(claims), Path(id)).await
}

/// 应用错误规则
pub async fn apply_error_rules(
    Extension(state): Extension<SharedState>,
    Json(body): Json<ApplyRulesRequest>,
) -> Result<Json<Value>, ApiError> {
    let db = &state.db;
    let result = ErrorPassthroughRuleService::apply_rules(
        db,
        body.error_code,
        body.error_message.as_deref(),
        body.platform.as_deref(),
        body.response_code,
        body.response_body.as_deref(),
    )
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!(result)))
}

#[derive(Debug, Deserialize)]
pub struct ApplyRulesRequest {
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub platform: Option<String>,
    pub response_code: Option<i32>,
    pub response_body: Option<String>,
}
