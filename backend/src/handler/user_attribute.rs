//! 用户属性管理 API Handler

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
use crate::service::user::Claims;
use crate::service::user_attribute::{
    CreateAttributeDefinitionRequest, SetAttributeValueRequest, UpdateAttributeDefinitionRequest,
    UserAttributeService,
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub enabled_only: bool,
}

// ==================== 属性定义管理 ====================

/// 列出所有属性定义
pub async fn list_definitions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let definitions = UserAttributeService::list_definitions(db, query.enabled_only)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": definitions
    })))
}

/// 创建属性定义
pub async fn create_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateAttributeDefinitionRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let definition = UserAttributeService::create_definition(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(definition)))
}

/// 更新属性定义
pub async fn update_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAttributeDefinitionRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let definition = UserAttributeService::update_definition(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                "Attribute definition not found".into(),
            )
        })?;

    Ok(Json(json!(definition)))
}

/// 删除属性定义
pub async fn delete_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = UserAttributeService::delete_definition(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(
            json!({ "success": true, "message": "Attribute definition deleted" }),
        ))
    } else {
        Err(ApiError(
            StatusCode::NOT_FOUND,
            "Attribute definition not found".into(),
        ))
    }
}

// ==================== 属性值管理 ====================

/// 设置用户属性值
pub async fn set_value(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path((user_id, attribute_id)): Path<(i64, i64)>,
    Json(body): Json<SetAttributeValueRequest>,
) -> Result<Json<Value>, ApiError> {
    // 只能设置自己的属性，或者管理员可以设置所有人的属性
    let current_user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    if user_id != current_user_id {
        check_permission(&claims, Permission::BillingWrite)
            .await
            .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;
    }

    let db = &state.db;
    let value = UserAttributeService::set_value(db, user_id, attribute_id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(value)))
}

/// 获取用户所有属性值
pub async fn get_user_values(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // 只能查看自己的属性，或者管理员可以查看所有人的属性
    let current_user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    if user_id != current_user_id {
        check_permission(&claims, Permission::BillingRead)
            .await
            .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;
    }

    let db = &state.db;
    let values = UserAttributeService::get_user_values(db, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": values
    })))
}

// 路由别名函数
pub async fn create_attribute_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateAttributeDefinitionRequest>,
) -> Result<Json<Value>, ApiError> {
    create_definition(Extension(state), Extension(claims), Json(body)).await
}

pub async fn list_attribute_definitions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    list_definitions(Extension(state), Extension(claims), Query(query)).await
}

pub async fn update_attribute_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAttributeDefinitionRequest>,
) -> Result<Json<Value>, ApiError> {
    update_definition(Extension(state), Extension(claims), Path(id), Json(body)).await
}

pub async fn delete_attribute_definition(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    delete_definition(Extension(state), Extension(claims), Path(id)).await
}

pub async fn set_user_attribute(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<SetUserAttributeRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    set_value(
        Extension(state),
        Extension(claims),
        Path((user_id, body.attribute_id)),
        Json(SetAttributeValueRequest { value: body.value }),
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct SetUserAttributeRequest {
    pub attribute_id: i64,
    pub value: String,
}

pub async fn get_user_attributes(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    get_user_values(Extension(state), Extension(claims), Path(user_id)).await
}
