//! 优惠码管理 API Handler

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
use crate::service::promo_code::{
    CreatePromoCodeRequest, PromoCodeService, UpdatePromoCodeRequest, VerifyPromoCodeRequest,
};
use crate::service::user::Claims;

#[derive(Debug, Deserialize)]
pub struct ListPromoCodesQuery {
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

/// 列出所有优惠码
pub async fn list_promo_codes(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListPromoCodesQuery>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let codes = PromoCodeService::list(db, query.status, query.page, query.page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": codes
    })))
}

/// 创建优惠码
pub async fn create_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreatePromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let code = PromoCodeService::create(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(code)))
}

/// 获取优惠码详情
pub async fn get_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let code = PromoCodeService::get_by_id(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Promo code not found".into()))?;

    Ok(Json(json!(code)))
}

/// 更新优惠码
pub async fn update_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let code = PromoCodeService::update(db, id, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Promo code not found".into()))?;

    Ok(Json(json!(code)))
}

/// 删除优惠码
pub async fn delete_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let db = &state.db;
    let deleted = PromoCodeService::delete(db, id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(
            json!({ "success": true, "message": "Promo code deleted" }),
        ))
    } else {
        Err(ApiError(
            StatusCode::NOT_FOUND,
            "Promo code not found".into(),
        ))
    }
}

/// 验证优惠码
pub async fn verify_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(_claims): Extension<Claims>,
    Json(body): Json<VerifyPromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let db = &state.db;
    let result = PromoCodeService::verify(db, body)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!(result)))
}

/// 使用优惠码
pub async fn use_promo_code(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<VerifyPromoCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let db = &state.db;

    // Get user_id from claims
    let user_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid user ID".into()))?;

    let bonus = PromoCodeService::use_code(db, &body.code, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "bonus_amount": bonus
    })))
}
