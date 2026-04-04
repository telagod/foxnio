//! 管理仪表盘 API Handler

use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::dashboard_query_service::{DashboardDateRange, DashboardQueryService};
use crate::service::permission::Permission;
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// 获取仪表盘综合统计
pub async fn get_dashboard_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let stats = service
        .get_dashboard_stats()
        .await
        .map_err(internal_error)?;

    Ok(Json(json!(stats)))
}

/// 获取趋势数据
pub async fn get_trend_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let range = parse_range(&query)?;
    let service = DashboardQueryService::new(state.db.clone());
    let data = service
        .get_trend_data(range)
        .await
        .map_err(internal_error)?;

    Ok(Json(json!(data)))
}

/// 获取模型使用分布
pub async fn get_model_distribution(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let data = service
        .get_model_distribution()
        .await
        .map_err(internal_error)?;

    Ok(Json(json!(data)))
}

/// 获取平台使用分布
pub async fn get_platform_distribution(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let data = service
        .get_platform_distribution()
        .await
        .map_err(internal_error)?;

    Ok(Json(json!(data)))
}

/// 获取折线图数据
pub async fn get_line_chart_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let range = parse_range(&query)?;
    let service = DashboardQueryService::new(state.db.clone());
    let data = service
        .get_line_chart_data(range)
        .await
        .map_err(internal_error)?;

    Ok(Json(json!(data)))
}

/// 获取饼图数据
pub async fn get_pie_chart_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let data = service.get_pie_chart_data().await.map_err(internal_error)?;

    Ok(Json(json!(data)))
}

async fn ensure_billing_permission(claims: &Claims) -> Result<(), ApiError> {
    check_permission(claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))
}

fn parse_range(query: &StatsQuery) -> Result<DashboardDateRange, ApiError> {
    DashboardDateRange::parse(query.start_date.as_deref(), query.end_date.as_deref())
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))
}

fn internal_error(error: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
