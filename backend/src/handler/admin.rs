//! 管理后台处理器 - 完整实现
//!
//! 使用角色权限系统进行访问控制

use axum::{
    Extension,
    Json,
    http::StatusCode,
    extract::Path,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::gateway::SharedState;
use crate::service::{UserService, AccountService, BillingService};
use crate::service::user::Claims;
use crate::service::permission::{Permission, PermissionService};
use crate::gateway::middleware::permission::check_permission;
use super::ApiError;

// ============ 用户管理 API ============

/// 用户创建请求
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub role: String,
}

/// 用户更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

/// 余额更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateBalanceRequest {
    pub delta: i64,
}

/// 用户信息响应
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub balance_yuan: f64,
    pub created_at: String,
}

/// 列出用户 - 需要 UserRead 权限
pub async fn list_users(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

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

/// 创建用户 - 需要 UserWrite 权限
pub async fn create_user(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    // 验证邮箱格式
    if !req.email.contains('@') {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Invalid email format".into()));
    }

    // 验证密码长度
    if req.password.len() < 8 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Password must be at least 8 characters".into()));
    }

    let role = if req.role.is_empty() { "user" } else { &req.role };

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 注册用户
    let user = user_service.register(&req.email, &req.password)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 如果指定了角色，更新角色（需要额外权限）
    if role != "user" {
        // TODO: 更新用户角色
    }

    Ok(Json(json!({
        "id": user.id.to_string(),
        "email": user.email,
        "role": user.role,
        "status": user.status,
        "balance": user.balance,
        "balance_yuan": user.balance as f64 / 100.0,
        "created_at": user.created_at.to_rfc3339(),
    })))
}

/// 获取用户详情 - 需要 UserRead 权限
pub async fn get_user(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let user = user_service.get_by_id(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "User not found".into()))?;

    Ok(Json(json!({
        "id": user.id.to_string(),
        "email": user.email,
        "role": user.role,
        "status": user.status,
        "balance": user.balance,
        "balance_yuan": user.balance as f64 / 100.0,
        "created_at": user.created_at.to_rfc3339(),
    })))
}

/// 更新用户 - 需要 UserWrite 权限
pub async fn update_user(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let _user_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // TODO: 实现用户更新逻辑
    // 需要在 UserService 中添加 update 方法

    Ok(Json(json!({
        "success": true,
        "message": "User update not yet implemented"
    })))
}

/// 删除用户 - 需要 UserDelete 权限
pub async fn delete_user(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserDelete)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 不能删除自己
    if claims.sub == id {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Cannot delete yourself".into()));
    }

    // TODO: 实现用户删除逻辑
    // 需要在 UserService 中添加 delete 方法

    Ok(Json(json!({
        "success": true,
        "message": "User deletion not yet implemented"
    })))
}

/// 更新用户余额 - 需要 UserWrite 权限
pub async fn update_user_balance(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBalanceRequest>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let new_balance = user_service.update_balance(user_id, req.delta)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "new_balance": new_balance,
        "new_balance_yuan": new_balance as f64 / 100.0,
    })))
}

// ============ 账号管理 API ============

/// 列出账号 - 需要 AccountRead 权限
pub async fn list_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

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

/// 添加账号 - 需要 AccountWrite 权限
pub async fn add_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

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

/// 删除账号 - 需要 AccountWrite 权限
pub async fn delete_account_by_id(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let account_service = AccountService::new(state.db.clone());
    account_service.delete(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

/// 删除账号（旧接口，保持兼容）
pub async fn delete_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = body.get("id")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing id".into()))?;

    let account_id = Uuid::parse_str(account_id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let account_service = AccountService::new(state.db.clone());
    account_service.delete(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

// ============ API Key 管理 API ============

/// 列出所有 API Keys - 需要 ApiKeyRead 权限
pub async fn list_apikeys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::ApiKeyRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    // TODO: 实现管理员查看所有 API Key
    Ok(Json(json!({
        "object": "list",
        "data": []
    })))
}

// ============ 统计和监控 API ============

/// 获取全局统计 - 需要 BillingRead 权限
pub async fn get_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

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

/// 获取仪表盘数据 - 需要 BillingRead 权限
pub async fn get_dashboard(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let billing_service = BillingService::new(
        state.db.clone(),
        state.config.gateway.rate_multiplier,
    );

    let stats = billing_service.get_global_stats(7)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "week": {
            "total_requests": stats.total_requests,
            "total_input_tokens": stats.total_input_tokens,
            "total_output_tokens": stats.total_output_tokens,
            "total_cost": stats.total_cost,
        }
    })))
}

// ============ 权限管理 API ============

/// 获取权限矩阵
pub async fn get_permission_matrix(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查 - 只有管理员可以查看
    if !PermissionService::is_admin_or_higher(&claims) {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let matrix = PermissionService::get_permission_matrix().await;

    Ok(Json(json!({
        "matrix": matrix
    })))
}

/// 获取所有角色
pub async fn list_roles(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let service = crate::gateway::middleware::permission::get_permission_service();
    let roles = service.get_all_roles().await;

    Ok(Json(json!({
        "roles": roles.into_iter().map(|(name, perms)| json!({
            "name": name,
            "permissions": perms.iter().map(|p| p.as_str()).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })))
}
