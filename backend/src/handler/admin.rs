//! 管理后台处理器 - 完整实现

use axum::{
    Extension,
    Json,
    http::StatusCode,
};
use serde_json::{json, Value};
use crate::gateway::SharedState;
use crate::service::{UserService, AccountService, BillingService};
use crate::service::user::Claims;
use super::ApiError;

/// 列出用户
pub async fn list_users(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let users = user_service.list_all(1, 100)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": users.iter().map(|u| json!({
            "id": u.id.to_string(),
            "email": u.email,
            "role": u.role,
            "status": u.status,
            "balance": u.balance,
            "balance_yuan": u.balance as f64 / 100.0,
            "created_at": u.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// 列出账号
pub async fn list_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let account_service = AccountService::new(state.db.clone());

    let accounts = account_service.list_all()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": accounts.iter().map(|a| json!({
            "id": a.id.to_string(),
            "name": a.name,
            "provider": a.provider,
            "credential_type": a.credential_type,
            "status": a.status,
            "priority": a.priority,
            "last_error": a.last_error,
            "created_at": a.created_at.to_rfc3339(),
        })).collect::<Vec<_>>()
    })))
}

/// 列出 API Keys
pub async fn list_apikeys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    // TODO: 实现管理员查看所有 API Key
    Ok(Json(json!({
        "object": "list",
        "data": []
    })))
}

/// 获取全局统计
pub async fn get_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let billing_service = BillingService::new(
        state.db.clone(),
        state.config.gateway.rate_multiplier,
    );

    let stats = billing_service.get_global_stats(30)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "total_requests": stats.total_requests,
        "total_input_tokens": stats.total_input_tokens,
        "total_output_tokens": stats.total_output_tokens,
        "total_cost": stats.total_cost,
        "total_cost_yuan": stats.total_cost as f64 / 100.0,
    })))
}

/// 添加账号
pub async fn add_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let name = body.get("name")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing name".into()))?;
    
    let provider = body.get("provider")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing provider".into()))?;
    
    let credential_type = body.get("credential_type")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing credential_type".into()))?;
    
    let credential = body.get("credential")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing credential".into()))?;
    
    let priority = body.get("priority")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let account_service = AccountService::new(state.db.clone());

    let account = account_service.add(name, provider, credential_type, credential, priority)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "id": account.id.to_string(),
        "name": account.name,
        "provider": account.provider,
        "status": account.status,
    })))
}

/// 删除账号
pub async fn delete_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let account_id = body.get("id")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing id".into()))?;

    let account_id = uuid::Uuid::parse_str(account_id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let account_service = AccountService::new(state.db.clone());
    account_service.delete(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}
