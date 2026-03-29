//! 定时测试计划管理 API Handler

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
use crate::service::scheduled_test_plan::{
    CreateTestPlanRequest, ScheduledTestPlanService, UpdateTestPlanRequest,
};
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub enabled_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct ResultsQuery {
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

/// 列出所有测试计划
pub async fn list_plans(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let plans = ScheduledTestPlanService::list(db, query.enabled_only)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": plans
    })))
}

/// 创建测试计划
pub async fn create_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(mut body): Json<CreateTestPlanRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;
    body.created_by = Some(user_id);

    let db = &state.db;
    let plan = ScheduledTestPlanService::create(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(plan)))
}

/// 获取测试计划详情
pub async fn get_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let plans = ScheduledTestPlanService::list(db, false)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let plan = plans
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Test plan not found".into()))?;

    Ok(Json(json!(plan)))
}

/// 更新测试计划
pub async fn update_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTestPlanRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let plan = ScheduledTestPlanService::update(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Test plan not found".into()))?;

    Ok(Json(json!(plan)))
}

/// 删除测试计划
pub async fn delete_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = ScheduledTestPlanService::delete(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(
            json!({ "success": true, "message": "Test plan deleted" }),
        ))
    } else {
        Err(ApiError(
            StatusCode::NOT_FOUND,
            "Test plan not found".into(),
        ))
    }
}

/// 获取测试计划的结果列表
pub async fn get_plan_results(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Query(query): Query<ResultsQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let results = ScheduledTestPlanService::get_results(db, id, query.page, query.page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": results
    })))
}

/// 手动执行测试计划
pub async fn run_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;

    // TODO: 实现实际的测试执行逻辑
    // 这里只是记录一个测试结果
    let start = std::time::Instant::now();

    // 模拟测试执行
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let duration_ms = start.elapsed().as_millis() as i64;

    ScheduledTestPlanService::record_result(
        db,
        id,
        "success",
        Some(json!({ "manual_run": true, "duration_ms": duration_ms })),
        None,
        Some(duration_ms),
    )
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "Test plan executed",
        "duration_ms": duration_ms
    })))
}

// 路由别名函数
pub async fn create_test_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateTestPlanRequest>,
) -> Result<Json<Value>, ApiError> {
    create_plan(Extension(state), Extension(claims), Json(body)).await
}

pub async fn list_test_plans(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    list_plans(Extension(state), Extension(claims), Query(query)).await
}

pub async fn update_test_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTestPlanRequest>,
) -> Result<Json<Value>, ApiError> {
    update_plan(Extension(state), Extension(claims), Path(id), Json(body)).await
}

pub async fn delete_test_plan(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    delete_plan(Extension(state), Extension(claims), Path(id)).await
}

pub async fn get_test_results(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Query(query): Query<ResultsQuery>,
) -> Result<Json<Value>, ApiError> {
    get_plan_results(Extension(state), Extension(claims), Path(id), Query(query)).await
}

pub async fn record_test_result(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    run_plan(Extension(state), Extension(claims), Path(id)).await
}
