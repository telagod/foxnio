//! Webhook HTTP Handler
//!
//! 提供 Webhook 端点的 CRUD 操作和测试功能

#![allow(dead_code)]

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ApiError;
use crate::gateway::SharedState;
use crate::service::user::Claims;
use crate::service::webhook::WebhookService;

/// 创建 Webhook 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWebhookRequest {
    /// 回调 URL（必须是 HTTPS）
    pub url: String,
    /// 订阅的事件列表
    pub events: Vec<String>,
    /// 用于签名验证的密钥（可选）
    pub secret: Option<String>,
}

/// 更新 Webhook 请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateWebhookRequest {
    /// 回调 URL
    pub url: Option<String>,
    /// 订阅的事件列表
    pub events: Option<Vec<String>>,
    /// 用于签名验证的密钥
    pub secret: Option<String>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// Webhook 响应
#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookResponse {
    pub id: i64,
    pub url: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: String,
}

/// Webhook 投递记录响应
#[derive(Debug, Serialize, ToSchema)]
pub struct DeliveryResponse {
    pub id: i64,
    pub event_type: String,
    pub status: String,
    pub response_code: Option<i32>,
    pub attempts: i32,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

/// POST /api/v1/webhooks - 创建 webhook
///
/// 创建一个新的 Webhook 端点
#[utoipa::path(
    post,
    path = "/api/v1/webhooks",
    request_body = CreateWebhookRequest,
    responses(
        (status = 201, description = "创建成功", body = WebhookResponse),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn create_webhook(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    // 验证 URL 格式
    if !req.url.starts_with("https://") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Webhook URL must use HTTPS".into(),
        ));
    }

    // 验证事件列表不为空
    if req.events.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "At least one event type must be specified".into(),
        ));
    }

    let webhook_service = WebhookService::new(state.db.clone());

    let secret_for_create = req.secret.clone().unwrap_or_default();
    let endpoint = webhook_service
        .create_endpoint(user_id, req.url, req.events, secret_for_create)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let events: Vec<String> = endpoint
        .events
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(WebhookResponse {
        id: endpoint.id,
        url: endpoint.url,
        events,
        is_active: endpoint.enabled,
        created_at: endpoint.created_at.to_rfc3339(),
    }))
}

/// GET /api/v1/webhooks - 列出 webhooks
///
/// 列出当前用户的所有 Webhook 端点
#[utoipa::path(
    get,
    path = "/api/v1/webhooks",
    responses(
        (status = 200, description = "成功", body = Vec<WebhookResponse>),
        (status = 401, description = "未授权")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn list_webhooks(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<WebhookResponse>>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let webhook_service = WebhookService::new(state.db.clone());

    let endpoints = webhook_service
        .list_endpoints(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<WebhookResponse> = endpoints
        .into_iter()
        .map(|e| {
            let events: Vec<String> = e
                .events
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            WebhookResponse {
                id: e.id,
                url: e.url,
                events,
                is_active: e.enabled,
                created_at: e.created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(responses))
}

/// GET /api/v1/webhooks/:id - 获取详情
///
/// 获取指定 Webhook 端点的详细信息
#[utoipa::path(
    get,
    path = "/api/v1/webhooks/{id}",
    params(
        ("id" = i64, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "成功", body = WebhookResponse),
        (status = 401, description = "未授权"),
        (status = 404, description = "Webhook 不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn get_webhook(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let webhook_service = WebhookService::new(state.db.clone());

    let endpoint = webhook_service
        .get_endpoint(id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Webhook not found".into()))?;

    let events: Vec<String> = endpoint
        .events
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(WebhookResponse {
        id: endpoint.id,
        url: endpoint.url,
        events,
        is_active: endpoint.enabled,
        created_at: endpoint.created_at.to_rfc3339(),
    }))
}

/// PUT /api/v1/webhooks/:id - 更新 webhook
///
/// 更新指定 Webhook 端点的配置
#[utoipa::path(
    put,
    path = "/api/v1/webhooks/{id}",
    params(
        ("id" = i64, Path, description = "Webhook ID")
    ),
    request_body = UpdateWebhookRequest,
    responses(
        (status = 200, description = "更新成功", body = WebhookResponse),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权"),
        (status = 404, description = "Webhook 不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn update_webhook(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    // 验证 URL 格式（如果提供）
    if let Some(ref url) = req.url {
        if !url.starts_with("https://") {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Webhook URL must use HTTPS".into(),
            ));
        }
    }

    // 验证事件列表不为空（如果提供）
    if let Some(ref events) = req.events {
        if events.is_empty() {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "At least one event type must be specified".into(),
            ));
        }
    }

    let webhook_service = WebhookService::new(state.db.clone());

    let endpoint = webhook_service
        .update_endpoint(id, user_id, req.url, req.events, req.secret, req.is_active)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "Webhook not found".into()))?;

    let events: Vec<String> = endpoint
        .events
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(WebhookResponse {
        id: endpoint.id,
        url: endpoint.url,
        events,
        is_active: endpoint.enabled,
        created_at: endpoint.created_at.to_rfc3339(),
    }))
}

/// DELETE /api/v1/webhooks/:id - 删除 webhook
///
/// 删除指定的 Webhook 端点
#[utoipa::path(
    delete,
    path = "/api/v1/webhooks/{id}",
    params(
        ("id" = i64, Path, description = "Webhook ID")
    ),
    responses(
        (status = 204, description = "删除成功"),
        (status = 401, description = "未授权"),
        (status = 404, description = "Webhook 不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn delete_webhook(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let webhook_service = WebhookService::new(state.db.clone());

    let deleted = webhook_service
        .delete_endpoint(id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError(StatusCode::NOT_FOUND, "Webhook not found".into()))
    }
}

/// POST /api/v1/webhooks/:id/test - 测试 webhook
///
/// 向指定 Webhook 端点发送测试请求
#[utoipa::path(
    post,
    path = "/api/v1/webhooks/{id}/test",
    params(
        ("id" = i64, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "测试结果"),
        (status = 401, description = "未授权"),
        (status = 404, description = "Webhook 不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn test_webhook(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let webhook_service = WebhookService::new(state.db.clone());

    let success = webhook_service
        .test_endpoint(id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if success {
        Ok(Json(json!({
            "success": true,
            "message": "Test webhook sent successfully"
        })))
    } else {
        Err(ApiError(StatusCode::NOT_FOUND, "Webhook not found".into()))
    }
}

/// GET /api/v1/webhooks/:id/deliveries - 投递日志
///
/// 获取指定 Webhook 端点的投递记录
#[utoipa::path(
    get,
    path = "/api/v1/webhooks/{id}/deliveries",
    params(
        ("id" = i64, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "成功", body = Vec<DeliveryResponse>),
        (status = 401, description = "未授权"),
        (status = 404, description = "Webhook 不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Webhook"
)]
pub async fn list_deliveries(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<DeliveryResponse>>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid user ID: {e}")))?;

    let webhook_service = WebhookService::new(state.db.clone());

    let deliveries = webhook_service
        .list_deliveries(id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let responses: Vec<DeliveryResponse> = deliveries
        .into_iter()
        .map(|d| DeliveryResponse {
            id: d.id,
            event_type: d.event_type,
            status: d.status,
            response_code: d.response_code,
            attempts: d.attempts,
            created_at: d.created_at.to_rfc3339(),
            delivered_at: d.delivered_at.map(|t| t.to_rfc3339()),
        })
        .collect();

    Ok(Json(responses))
}
