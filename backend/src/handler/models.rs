//! Model Management Handlers
//!
//! 模型配置管理 API 端点

#![allow(dead_code)]
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use utoipa::OpenApi;

use crate::entity::model_configs::{CreateModelRequest, ModelInfoResponse, UpdateModelRequest};
use crate::gateway::middleware::permission::check_permission;
use crate::service::permission::Permission;
use crate::service::ModelRegistry;
use crate::state::AppState;

use super::ApiError;

/// 列出所有模型（公开 API）
#[utoipa::path(
    get,
    path = "/v1/models",
    responses(
        (status = 200, description = "模型列表")
    ),
    tag = "模型"
)]
pub async fn list_models_public(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let registry = ModelRegistry::new(state.db.clone());

    // 尝试从数据库加载，如果失败则返回空列表
    let _ = registry.load_from_db().await;

    let models = registry.list_models_info().await;

    let data: Vec<serde_json::Value> = models
        .into_iter()
        .filter(|m| m.enabled)
        .map(|m| {
            json!({
                "id": m.id,
                "object": "model",
                "created": 1700000000,
                "owned_by": m.provider.to_lowercase(),
                "permission": [{
                    "id": format!("modelperm-{}", m.id),
                    "object": "model_permission",
                    "created": 1700000000,
                    "allow_create_engine": false,
                    "allow_sampling": true,
                    "allow_logprobs": true,
                    "allow_search_indices": false,
                    "allow_view": true,
                    "allow_fine_tuning": false,
                    "organization": "*",
                    "group": null,
                    "is_blocking": false
                }],
                "root": m.id,
                "parent": null,
            })
        })
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": data
    })))
}

/// 列出所有模型（管理 API）
///
/// 获取所有模型配置（管理员）
#[utoipa::path(
    get,
    path = "/api/v1/admin/models",
    responses(
        (status = 200, description = "模型列表"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-模型"
)]
pub async fn list_models_admin(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let models = registry.list_models_info().await;

    Ok(Json(json!({
        "object": "list",
        "data": models
    })))
}

/// 获取模型详情
///
/// 获取指定模型的详细配置信息（管理员）
#[utoipa::path(
    get,
    path = "/api/v1/admin/models/{id}",
    responses(
        (status = 200, description = "模型详情"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "模型不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-模型"
)]
pub async fn get_model(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let model = registry
        .resolve(&id)
        .await
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("Model not found: {}", id)))?;

    Ok(Json(json!(ModelInfoResponse {
        id: model.name.clone(),
        name: model.display_name.clone(),
        provider: model.provider.clone(),
        context_window: model.context_window as u32,
        max_tokens: model.max_tokens as u32,
        input_price: model.input_price,
        output_price: model.output_price,
        capabilities: model.get_capabilities(),
        enabled: model.enabled,
    })))
}

/// 创建模型
pub async fn create_model(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Json(req): Json<CreateModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let model = registry
        .create(req)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(ModelInfoResponse {
        id: model.name.clone(),
        name: model.display_name.clone(),
        provider: model.provider.clone(),
        context_window: model.context_window as u32,
        max_tokens: model.max_tokens as u32,
        input_price: model.input_price,
        output_price: model.output_price,
        capabilities: model.get_capabilities(),
        enabled: model.enabled,
    })))
}

/// 更新模型
pub async fn update_model(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let model = registry
        .update(id, req)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(ModelInfoResponse {
        id: model.name.clone(),
        name: model.display_name.clone(),
        provider: model.provider.clone(),
        context_window: model.context_window as u32,
        max_tokens: model.max_tokens as u32,
        input_price: model.input_price,
        output_price: model.output_price,
        capabilities: model.get_capabilities(),
        enabled: model.enabled,
    })))
}

/// 删除模型
pub async fn delete_model(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    registry
        .delete(id)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 热加载模型配置
pub async fn reload_models(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());

    registry
        .reload()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let models = registry.list_models_info().await;

    Ok(Json(json!({
        "success": true,
        "message": format!("Loaded {} models", models.len())
    })))
}

/// 导入默认模型
pub async fn import_default_models(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let count = registry
        .import_defaults()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "imported": count
    })))
}

/// 获取模型路由信息
pub async fn get_model_route(
    Extension(state): Extension<Arc<AppState>>,
    Extension(claims): Extension<crate::service::user::Claims>,
    Path(model_name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_permission(&claims, Permission::ModelRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let registry = ModelRegistry::new(state.db.clone());
    let _ = registry.load_from_db().await;

    let route = registry
        .route(&model_name)
        .await
        .map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(Json(json!({
        "model": route.model.name,
        "provider": route.model.provider,
        "api_name": route.model.api_name,
        "is_fallback": route.is_fallback,
        "original_model": route.original_model,
        "provider_config": {
            "base_url": route.provider_config.base_url,
            "auth_header": route.provider_config.auth_header,
        }
    })))
}
