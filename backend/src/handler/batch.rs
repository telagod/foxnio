//! 批量操作处理器
//!
//! 提供批量创建、更新、删除等 API 端点

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::batch::{
    BatchOperationService, BatchResult, CreateApiKeyRequest, CreateUserCsvRecord,
    CreateUserRequest,
};
use crate::service::permission::Permission;
use crate::service::user::Claims;

/// 批量更新请求
#[derive(Debug, Deserialize)]
pub struct BatchUpdateRequest {
    pub ids: Vec<Uuid>,
    pub updates: serde_json::Value,
}

/// POST /api/v1/admin/api-keys/batch-create
///
/// 批量创建 API Keys
pub async fn batch_create_api_keys(
    State(state): State<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(requests): Json<Vec<CreateApiKeyRequest>>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::ApiKeyWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if requests.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No API key requests provided".into(),
        ));
    }

    if requests.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100 API keys per batch".into(),
        ));
    }

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_create_api_keys(requests, false)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.success,
        "failed": results.failed,
        "results": results.results,
    })))
}

/// POST /api/v1/admin/accounts/batch-update
///
/// 批量更新账号
pub async fn batch_update_accounts(
    State(state): State<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<BatchUpdateRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if req.ids.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No account IDs provided".into(),
        ));
    }

    if req.ids.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100 accounts per batch".into(),
        ));
    }

    let updates: std::collections::HashMap<String, serde_json::Value> = serde_json::from_value(req.updates.clone())
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid updates: {}", e)))?;

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_update_accounts(
            crate::service::batch::BatchUpdateRequest {
                ids: req.ids,
                updates,
            },
            false,
        )
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.success,
        "failed": results.failed,
        "results": results.results,
    })))
}

/// POST /api/v1/admin/users/batch-import
///
/// 批量导入用户（CSV 文件）
pub async fn batch_import_users(
    State(state): State<SharedState>,
    Extension(claims): Extension<Claims>,
    Multipart(mut form): Multipart,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let mut csv_content = String::new();

    // 解析 multipart 表单
    while let Some(field) = form.next_field().await.map_err(|e| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("Failed to parse multipart: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            csv_content = field.text().await.map_err(|e| {
                ApiError(
                    StatusCode::BAD_REQUEST,
                    format!("Failed to read file content: {}", e),
                )
            })?;
        }
    }

    if csv_content.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No CSV file provided".into(),
        ));
    }

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_import_users_csv(&csv_content)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.success,
        "failed": results.failed,
        "results": results.results,
    })))
}

/// POST /api/v1/admin/api-keys/batch-delete
///
/// 批量删除 API Keys
pub async fn batch_delete_api_keys(
    State(state): State<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(ids): Json<Vec<Uuid>>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::ApiKeyWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if ids.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No API key IDs provided".into(),
        ));
    }

    if ids.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100 API keys per batch".into(),
        ));
    }

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_delete_api_keys(ids)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.success,
        "failed": results.failed,
        "results": results.results,
    })))
}

/// POST /api/v1/admin/users/batch-create
///
/// 批量创建用户
pub async fn batch_create_users(
    State(state): State<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(requests): Json<Vec<CreateUserRequest>>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if requests.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No user requests provided".into(),
        ));
    }

    if requests.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100 users per batch".into(),
        ));
    }

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_create_users(requests)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.success,
        "failed": results.failed,
        "results": results.results,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_update_request() {
        let json = r#"{
            "ids": ["550e8400-e29b-41d4-a716-446655440000"],
            "updates": {
                "status": "active",
                "priority": 10
            }
        }"#;

        let req: BatchUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.ids.len(), 1);
    }
}
