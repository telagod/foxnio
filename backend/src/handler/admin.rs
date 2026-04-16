//! 管理后台处理器 - 完整实现
//!
//! 使用角色权限系统进行访问控制

#![allow(dead_code)]
use axum::{extract::Path, extract::Query, http::StatusCode, Extension, Json};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::dashboard_query_service::{DashboardDateRange, DashboardQueryService};
use crate::service::permission::{Permission, PermissionService};
use crate::service::user::Claims;
use crate::service::{LegacyAccountService as AccountService, LegacyApiKeyService, UserService};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

// ============ 用户管理 API ============

/// 用户创建请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub role: String,
}

/// 用户更新请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub balance: Option<i64>,
}

/// 余额更新请求
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateBalanceRequest {
    pub delta: i64,
}

/// 用户信息响应
#[derive(Debug, Serialize, ToSchema)]
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
///
/// 获取所有用户列表（管理员）
#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    responses(
        (status = 200, description = "用户列表"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-用户"
)]
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

    let users = user_service
        .list_all(1, 100)
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
///
/// 创建新用户（管理员）
#[utoipa::path(
    post,
    path = "/api/v1/admin/users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "用户创建成功"),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-用户"
)]
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
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid email format".into(),
        ));
    }

    // 验证密码长度
    if req.password.len() < 8 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters".into(),
        ));
    }

    let role = if req.role.is_empty() {
        "user"
    } else {
        &req.role
    };

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    // 注册用户
    let user = user_service
        .register(&req.email, &req.password)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 如果指定了角色，更新角色（需要额外权限）
    if role != "user" {
        // NOTE: 更新用户角色
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
///
/// 获取指定用户的详细信息（管理员）
#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}",
    responses(
        (status = 200, description = "用户详情"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-用户"
)]
pub async fn get_user(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::UserRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let user_id =
        Uuid::parse_str(&id).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let user = user_service
        .get_by_id(user_id)
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
///
/// 更新用户信息（管理员）
#[utoipa::path(
    put,
    path = "/api/v1/admin/users/{id}",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "更新成功"),
        (status = 400, description = "无效的请求参数"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-用户"
)]
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

    let user_id =
        Uuid::parse_str(&id).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let updated = user_service
        .update_user(
            user_id,
            req.email.as_deref(),
            req.role.as_deref(),
            req.status.as_deref(),
            req.balance,
        )
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "user": {
            "id": updated.id.to_string(),
            "email": updated.email,
            "role": updated.role,
            "status": updated.status,
            "balance": updated.balance,
            "balance_yuan": updated.balance as f64 / 100.0,
            "updated_at": updated.created_at.to_rfc3339(),
        }
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

    let user_id =
        Uuid::parse_str(&id).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 不能删除自己
    if claims.sub == id {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Cannot delete yourself".into(),
        ));
    }

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    user_service
        .delete_user(user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "User deleted successfully"
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

    let user_id =
        Uuid::parse_str(&id).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );

    let new_balance = user_service
        .update_balance(user_id, req.delta)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "new_balance": new_balance,
        "new_balance_yuan": new_balance as f64 / 100.0,
    })))
}

// ============ 账号管理 API ============

/// 账号列表查询参数
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListAccountsQuery {
    /// 页码（从 1 开始）
    #[serde(default = "default_page")]
    pub page: Option<u64>,
    /// 每页数量（最大 200）
    #[serde(default = "default_per_page")]
    pub per_page: Option<u64>,
    /// 状态过滤
    pub status: Option<String>,
    /// Provider 过滤
    pub provider: Option<String>,
    /// 名称搜索
    pub search: Option<String>,
    /// 分组过滤（group_id）
    pub group_id: Option<i64>,
}

fn default_page() -> Option<u64> {
    Some(1)
}
fn default_per_page() -> Option<u64> {
    Some(50)
}

/// 列出账号 - 需要 AccountRead 权限
///
/// 支持分页、过滤和搜索
#[utoipa::path(
    get,
    path = "/api/v1/admin/accounts",
    params(
        ("page" = Option<u64>, Query, description = "页码（从 1 开始）"),
        ("per_page" = Option<u64>, Query, description = "每页数量（最大 200）"),
        ("status" = Option<String>, Query, description = "状态过滤"),
        ("provider" = Option<String>, Query, description = "Provider 过滤"),
        ("search" = Option<String>, Query, description = "名称搜索"),
        ("group_id" = Option<i64>, Query, description = "分组ID过滤"),
    ),
    responses(
        (status = 200, description = "账号列表"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "管理员-账号"
)]
pub async fn list_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListAccountsQuery>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_service = AccountService::new(state.db.clone());

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);

    // 使用分页查询
    let (accounts, total) = account_service
        .list_paged(
            page,
            per_page,
            query.status.as_deref(),
            query.provider.as_deref(),
            query.search.as_deref(),
            query.group_id,
        )
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_pages = total.div_ceil(per_page);

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
            "group_id": a.group_id,
            "created_at": a.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total": total,
            "total_pages": total_pages,
            "has_more": page < total_pages,
        }
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

    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing name".into()))?;

    let provider = body
        .get("provider")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing provider".into()))?;

    let credential_type = body
        .get("credential_type")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing credential_type".into(),
        ))?;

    let credential = body
        .get("credential")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing credential".into(),
        ))?;

    let priority = body.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let account_service = AccountService::new(state.db.clone());

    let account = account_service
        .add(name, provider, credential_type, credential, priority)
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

    let account_id =
        Uuid::parse_str(&id).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let account_service = AccountService::new(state.db.clone());
    account_service
        .delete(account_id)
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

    let account_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing id".into()))?;

    let account_id = Uuid::parse_str(account_id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let account_service = AccountService::new(state.db.clone());
    account_service
        .delete(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

// ============ API Key 管理 API ============

/// 列出所有 API Keys - 需要 ApiKeyRead 权限
pub async fn list_apikeys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Value>, ApiError> {
    // 权限检查
    check_permission(&claims, Permission::ApiKeyRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let page = query.page.filter(|p| *p > 0).unwrap_or(1);
    let per_page = query.per_page.filter(|p| *p > 0).unwrap_or(50).min(200);

    let api_key_service = LegacyApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let (keys, total) = api_key_service
        .list_all(page, per_page)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "total": total,
        "page": page,
        "per_page": per_page,
        "data": keys.iter().map(|k| json!({
            "id": k.id.to_string(),
            "user_id": k.user_id.to_string(),
            "key": k.key_masked,
            "name": k.name,
            "status": k.status,
            "created_at": k.created_at.to_rfc3339(),
            "last_used_at": k.last_used_at.map(|t| t.to_rfc3339()),
        })).collect::<Vec<_>>()
    })))
}

// ============ 统计和监控 API ============

/// 获取全局统计 - 需要 BillingRead 权限
pub async fn get_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let stats = service
        .get_usage_totals(rolling_range(30)?)
        .await
        .map_err(internal_error)?;

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
    ensure_billing_permission(&claims).await?;

    let service = DashboardQueryService::new(state.db.clone());
    let stats = service
        .get_usage_totals(rolling_range(7)?)
        .await
        .map_err(internal_error)?;

    Ok(Json(json!({
        "week": {
            "total_requests": stats.total_requests,
            "total_input_tokens": stats.total_input_tokens,
            "total_output_tokens": stats.total_output_tokens,
            "total_cost": stats.total_cost,
        }
    })))
}

async fn ensure_billing_permission(claims: &Claims) -> Result<(), ApiError> {
    check_permission(claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))
}

fn rolling_range(days: i64) -> Result<DashboardDateRange, ApiError> {
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(days.max(1));
    DashboardDateRange::new(start_time, end_time)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))
}

fn internal_error(error: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
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
