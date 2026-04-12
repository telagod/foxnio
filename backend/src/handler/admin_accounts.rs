//! 管理端账号批量操作处理器
//!
//! 提供账号批量创建、批量更新、批量操作等端点

#![allow(dead_code)]

use axum::{extract::Path, http::StatusCode, Extension, Json};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::ApiError;
use crate::entity::accounts;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::providers::default_provider_registry;
use crate::gateway::SharedState;
use crate::metrics::BatchMetrics;
use crate::service::account::AccountService;
use crate::service::batch_import::{BatchImportConfig, BatchImportService, ImportAccountItem};
use crate::service::batch_operations::{
    BatchClearRateLimitResult, BatchCreateAccountsRequest, BatchOperationService, CreateAccountItem,
};
use crate::service::permission::{Permission, PermissionService};
use crate::service::user::Claims;
use crate::service::{AuditEntry, AuditService};

const MAX_FAST_IMPORT_BATCH_SIZE: usize = 5_000;
const MIN_FAST_IMPORT_BATCH_SIZE: usize = 1;
const MAX_VALIDATION_CONCURRENCY: usize = 256;
const MIN_VALIDATION_CONCURRENCY: usize = 1;

/// 批量更新凭证请求
#[derive(Debug, Deserialize)]
pub struct BatchUpdateCredentialsRequest {
    pub account_ids: Vec<String>,
    pub credential: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchStatusRequest {
    pub account_ids: Option<Vec<String>>,
    pub status: String,
    pub clear_error: Option<bool>,
    pub filter_status: Option<String>,
    pub filter_provider: Option<String>,
    pub filter_search: Option<String>,
    pub filter_group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BatchGroupRequest {
    pub account_ids: Option<Vec<String>>,
    pub group_id: Option<i64>,
    pub filter_status: Option<String>,
    pub filter_provider: Option<String>,
    pub filter_search: Option<String>,
    pub filter_group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BatchClearRateLimitRequest {
    pub account_ids: Option<Vec<String>>,
    pub filter_status: Option<String>,
    pub filter_provider: Option<String>,
    pub filter_search: Option<String>,
    pub filter_group_id: Option<i64>,
}

/// 高性能批量导入请求
#[derive(Debug, Deserialize)]
pub struct FastImportRequest {
    /// 账号列表
    pub accounts: Vec<ImportAccountItem>,
    /// 每批次大小（可选，默认 1000）
    #[serde(default)]
    pub batch_size: Option<usize>,
    /// 并发验证数（可选，默认 50）
    #[serde(default)]
    pub validation_concurrency: Option<usize>,
    /// 是否跳过重复（默认 true）
    #[serde(default = "default_skip_duplicates")]
    pub skip_duplicates: bool,
    /// 是否快速导入（跳过验证，默认 false）
    #[serde(default)]
    pub fast_mode: bool,
    /// 是否仅做预检，不落库
    #[serde(default)]
    pub dry_run: bool,
}

fn default_skip_duplicates() -> bool {
    true
}

fn summarize_batch_scope(
    account_ids: Option<&Vec<String>>,
    filter_status: Option<&str>,
    filter_provider: Option<&str>,
    filter_search: Option<&str>,
    filter_group_id: Option<i64>,
) -> Value {
    match account_ids {
        Some(ids) if !ids.is_empty() => json!({
            "mode": "explicit_ids",
            "account_id_count": ids.len(),
            "account_id_sample": ids.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        _ => json!({
            "mode": "filter_scope",
            "filter_status": filter_status,
            "filter_provider": filter_provider,
            "filter_search": filter_search,
            "filter_group_id": filter_group_id,
        }),
    }
}

fn record_batch_metrics(
    operation: &str,
    mode: &str,
    total: usize,
    failed: usize,
    started_at: std::time::Instant,
) {
    BatchMetrics::record(
        operation,
        mode,
        total,
        failed,
        started_at.elapsed().as_millis() as u64,
    );
}

async fn log_batch_audit(
    state: &SharedState,
    claims: &Claims,
    resource_id: &str,
    request_data: Value,
) {
    let Ok(admin_id) = Uuid::parse_str(&claims.sub) else {
        return;
    };

    let audit_svc = AuditService::new(state.db.clone());
    let entry = AuditEntry::admin_action(
        admin_id,
        "account_batch_operation",
        "account_batch",
        resource_id,
        None,
        Some(request_data),
    );
    let _ = audit_svc.log(entry).await;
}

/// GET /api/v1/admin/accounts/providers - 获取可用 provider 描述
pub async fn list_account_providers(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let providers = default_provider_registry()
        .descriptors()
        .into_iter()
        .filter(|provider| provider.key != "google")
        .map(|provider| {
            json!({
                "key": provider.key,
                "display_name": provider.display_name,
                "base_url": provider.base_url,
                "auth_header": provider.auth_header,
                "requires_version_header": provider.requires_version_header,
                "api_version": provider.api_version,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "success": true,
        "providers": providers,
    })))
}

/// POST /api/v1/admin/accounts/batch - 批量创建账号
pub async fn batch_create_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(items): Json<Vec<CreateAccountItem>>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if items.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No accounts provided".into(),
        ));
    }

    if items.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100 accounts per batch".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let total = items.len();
    let batch_service = BatchOperationService::new(state.db.clone());
    let req = BatchCreateAccountsRequest { accounts: items };
    let results = batch_service
        .batch_create_accounts(req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_create_accounts",
        "explicit_ids",
        total,
        results.failed.max(0) as usize,
        started_at,
    );

    Ok(Json(json!({
        "success": true,
        "total": results.total,
        "succeeded": results.succeeded,
        "failed": results.failed,
        "account_ids": results.account_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "errors": results.errors,
    })))
}

/// POST /api/v1/admin/accounts/:id/refresh - 刷新账号Token
pub async fn refresh_account_token(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    let result = account_service
        .refresh_token(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "account_id": id,
        "refreshed": result,
    })))
}

/// POST /api/v1/admin/accounts/:id/recover-state - 恢复账号状态
pub async fn recover_account_state(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    let result = account_service
        .recover_state(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "account_id": id,
        "recovered": result,
    })))
}

/// POST /api/v1/admin/accounts/:id/set-privacy - 设置账号隐私
pub async fn set_account_privacy(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let privacy_enabled = body
        .get("privacy_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 获取账号
    let account = accounts::Entity::find_by_id(account_id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Account not found".into()))?;

    // 更新 metadata 中的 privacy_enabled
    let mut metadata = account.metadata.clone().unwrap_or(serde_json::json!({}));
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "privacy_enabled".to_string(),
            serde_json::json!(privacy_enabled),
        );
    }

    let mut account: accounts::ActiveModel = account.into();
    account.metadata = Set(Some(metadata));
    account.updated_at = Set(Utc::now());
    let _updated = account
        .update(&state.db)
        .await
        .map_err(|e: sea_orm::DbErr| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        account_id = %account_id,
        privacy_enabled = privacy_enabled,
        "Account privacy setting updated"
    );

    Ok(Json(json!({
        "success": true,
        "account_id": id,
        "privacy_enabled": privacy_enabled,
    })))
}

/// POST /api/v1/admin/accounts/:id/refresh-tier - 刷新账号Tier
pub async fn refresh_account_tier(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    let tier = account_service
        .refresh_tier(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "account_id": id,
        "tier": tier,
    })))
}

/// POST /api/v1/admin/accounts/:id/clear-error - 清除账号错误
pub async fn clear_account_error(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    account_service
        .clear_error(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "account_id": id,
    })))
}

/// GET /api/v1/admin/accounts/:id/usage - 获取账号使用量
pub async fn get_account_usage(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    let usage = account_service
        .get_usage_stats(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "account_id": id,
        "usage": usage,
    })))
}

/// GET /api/v1/admin/accounts/:id/today-stats - 获取账号今日统计
pub async fn get_account_today_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    let stats = account_service
        .get_today_stats(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "account_id": id,
        "stats": stats,
    })))
}

/// POST /api/v1/admin/accounts/today-stats/batch - 批量获取今日统计
pub async fn batch_get_today_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_ids: Vec<String> = body
        .get("account_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing account_ids".into(),
        ))?;

    if account_ids.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No account_ids provided".into(),
        ));
    }

    let batch_service = BatchOperationService::new(state.db.clone());
    let stats = batch_service
        .batch_get_today_stats(&account_ids)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "stats": stats,
    })))
}

/// POST /api/v1/admin/accounts/:id/clear-rate-limit - 清除账号限流
pub async fn clear_account_rate_limit(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    // 清除 Redis 中的限流键
    // 常见的限流键模式：
    // - rate_limit:{account_id}
    // - ratelimit:{account_id}
    // - account_rate_limit:{account_id}
    let keys_to_delete = vec![
        format!("rate_limit:{}", account_id),
        format!("ratelimit:{}", account_id),
        format!("account_rate_limit:{}", account_id),
        format!("account:{account_id}:rate_limit"),
        format!("account:{account_id}:rpm"),
    ];

    let mut deleted_count = 0;
    for key in &keys_to_delete {
        match state.redis.del(key).await {
            Ok(_) => {
                deleted_count += 1;
                tracing::debug!("Deleted rate limit key: {}", key);
            }
            Err(e) => {
                tracing::debug!("Failed to delete key {}: {}", key, e);
            }
        }
    }

    tracing::info!(
        account_id = %account_id,
        deleted_keys = deleted_count,
        "Rate limits cleared for account"
    );

    Ok(Json(json!({
        "success": true,
        "account_id": id,
        "deleted_keys": deleted_count,
        "message": "Rate limits cleared",
    })))
}

/// POST /api/v1/admin/accounts/:id/reset-quota - 重置账号配额
pub async fn reset_account_quota(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Invalid account ID: {e}")))?;

    let account_service = AccountService::new(state.db.clone());
    account_service
        .reset_quota(account_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "account_id": id,
    })))
}

/// GET /api/v1/admin/accounts/data - 导出账号数据
#[allow(deprecated)]
pub async fn export_accounts_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_service = AccountService::new(state.db.clone());
    let accounts = account_service
        .list_all()
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

/// POST /api/v1/admin/accounts/data - 导入账号数据
pub async fn import_accounts_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let accounts: Vec<CreateAccountItem> = body
        .get("accounts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing accounts array".into(),
        ))?;

    if accounts.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No accounts provided".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let total = accounts.len();
    let batch_service = BatchOperationService::new(state.db.clone());
    let req = BatchCreateAccountsRequest { accounts };
    let results = batch_service
        .batch_create_accounts(req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "import_accounts_data",
        "explicit_ids",
        total,
        results.failed.max(0) as usize,
        started_at,
    );

    Ok(Json(json!({
        "success": true,
        "imported": results.succeeded,
        "total": results.total,
    })))
}

/// POST /api/v1/admin/accounts/batch-update-credentials - 批量更新凭证
pub async fn batch_update_credentials(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<BatchUpdateCredentialsRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if req.account_ids.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No account_ids provided".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let total = req.account_ids.len();
    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_update_credentials(&req.account_ids, &req.credential)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_update_credentials",
        "explicit_ids",
        total,
        results.iter().filter(|ok| !**ok).count(),
        started_at,
    );

    Ok(Json(json!({
        "success": true,
        "updated": results.iter().filter(|r| **r).count(),
        "total": results.len(),
        "results": results,
    })))
}

/// POST /api/v1/admin/accounts/batch-set-status - 批量设置账号状态
pub async fn batch_set_status(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<BatchStatusRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    if req.status.trim().is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "status is required".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let batch_service = BatchOperationService::new(state.db.clone());
    let clear_error = req.clear_error.unwrap_or(false);
    let has_filter = req.filter_status.is_some()
        || req.filter_provider.is_some()
        || req.filter_search.is_some()
        || req.filter_group_id.is_some();

    let scope = summarize_batch_scope(
        req.account_ids.as_ref(),
        req.filter_status.as_deref(),
        req.filter_provider.as_deref(),
        req.filter_search.as_deref(),
        req.filter_group_id,
    );

    let result = match req.account_ids {
        Some(account_ids) if !account_ids.is_empty() => {
            batch_service
                .batch_set_status(&account_ids, req.status.as_str(), clear_error)
                .await
        }
        Some(account_ids) if account_ids.is_empty() => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids provided".into(),
            ))
        }
        _ if has_filter => {
            batch_service
                .batch_set_status_by_filter(
                    req.status.as_str(),
                    clear_error,
                    req.filter_status.as_deref(),
                    req.filter_provider.as_deref(),
                    req.filter_search.as_deref(),
                    req.filter_group_id,
                )
                .await
        }
        _ => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids or filter conditions provided".into(),
            ))
        }
    }
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_set_status",
        if has_filter {
            "filter_scope"
        } else {
            "explicit_ids"
        },
        result.total.max(0) as usize,
        result.failed.max(0) as usize,
        started_at,
    );

    log_batch_audit(
        &state,
        &claims,
        "batch_set_status",
        json!({
            "operation": "batch_set_status",
            "scope": scope,
            "request": {
                "status": req.status,
                "clear_error": clear_error,
            },
            "result": {
                "total": result.total,
                "succeeded": result.succeeded,
                "failed": result.failed,
                "error_sample": result.errors.iter().take(5).cloned().collect::<Vec<_>>(),
            }
        }),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "total": result.total,
        "succeeded": result.succeeded,
        "failed": result.failed,
        "scope": scope,
        "errors": result.errors,
    })))
}

/// POST /api/v1/admin/accounts/batch-set-group - 批量切换账号分组
pub async fn batch_set_group(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<BatchGroupRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let started_at = std::time::Instant::now();
    let batch_service = BatchOperationService::new(state.db.clone());
    let has_filter = req.filter_status.is_some()
        || req.filter_provider.is_some()
        || req.filter_search.is_some()
        || req.filter_group_id.is_some();

    let scope = summarize_batch_scope(
        req.account_ids.as_ref(),
        req.filter_status.as_deref(),
        req.filter_provider.as_deref(),
        req.filter_search.as_deref(),
        req.filter_group_id,
    );

    let result = match req.account_ids {
        Some(account_ids) if !account_ids.is_empty() => {
            batch_service
                .batch_set_group(&account_ids, req.group_id)
                .await
        }
        Some(account_ids) if account_ids.is_empty() => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids provided".into(),
            ))
        }
        _ if has_filter => {
            batch_service
                .batch_set_group_by_filter(
                    req.group_id,
                    req.filter_status.as_deref(),
                    req.filter_provider.as_deref(),
                    req.filter_search.as_deref(),
                    req.filter_group_id,
                )
                .await
        }
        _ => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids or filter conditions provided".into(),
            ))
        }
    }
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_set_group",
        if has_filter {
            "filter_scope"
        } else {
            "explicit_ids"
        },
        result.total.max(0) as usize,
        result.failed.max(0) as usize,
        started_at,
    );

    log_batch_audit(
        &state,
        &claims,
        "batch_set_group",
        json!({
            "operation": "batch_set_group",
            "scope": scope,
            "request": {
                "group_id": req.group_id,
            },
            "result": {
                "total": result.total,
                "succeeded": result.succeeded,
                "failed": result.failed,
                "error_sample": result.errors.iter().take(5).cloned().collect::<Vec<_>>(),
            }
        }),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "total": result.total,
        "succeeded": result.succeeded,
        "failed": result.failed,
        "group_id": req.group_id,
        "scope": scope,
        "errors": result.errors,
    })))
}

/// POST /api/v1/admin/accounts/batch-clear-rate-limit - 批量清理限流 key
pub async fn batch_clear_rate_limit(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<BatchClearRateLimitRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let started_at = std::time::Instant::now();
    let batch_service = BatchOperationService::new(state.db.clone());
    let has_filter = req.filter_status.is_some()
        || req.filter_provider.is_some()
        || req.filter_search.is_some()
        || req.filter_group_id.is_some();

    let scope = summarize_batch_scope(
        req.account_ids.as_ref(),
        req.filter_status.as_deref(),
        req.filter_provider.as_deref(),
        req.filter_search.as_deref(),
        req.filter_group_id,
    );

    let result: BatchClearRateLimitResult = match req.account_ids {
        Some(account_ids) if !account_ids.is_empty() => {
            batch_service
                .batch_clear_rate_limit_keys(&state.redis, &account_ids)
                .await
        }
        Some(account_ids) if account_ids.is_empty() => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids provided".into(),
            ))
        }
        _ if has_filter => {
            batch_service
                .batch_clear_rate_limit_keys_by_filter(
                    &state.redis,
                    req.filter_status.as_deref(),
                    req.filter_provider.as_deref(),
                    req.filter_search.as_deref(),
                    req.filter_group_id,
                )
                .await
        }
        _ => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No account_ids or filter conditions provided".into(),
            ))
        }
    }
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_clear_rate_limit",
        if has_filter {
            "filter_scope"
        } else {
            "explicit_ids"
        },
        result.total.max(0) as usize,
        (result.missing + result.invalid).max(0) as usize,
        started_at,
    );

    log_batch_audit(
        &state,
        &claims,
        "batch_clear_rate_limit",
        json!({
            "operation": "batch_clear_rate_limit",
            "scope": scope,
            "result": {
                "total": result.total,
                "processed": result.processed,
                "missing": result.missing,
                "invalid": result.invalid,
                "deleted_keys": result.deleted_keys,
            }
        }),
    )
    .await;

    Ok(Json(json!({
        "success": true,
        "total": result.total,
        "processed": result.processed,
        "missing": result.missing,
        "invalid": result.invalid,
        "deleted_keys": result.deleted_keys,
        "scope": scope,
    })))
}

/// POST /api/v1/admin/accounts/batch-refresh-tier - 批量刷新Tier
pub async fn batch_refresh_tier(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let account_ids: Vec<String> = body
        .get("account_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "Missing account_ids".into(),
        ))?;

    if account_ids.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No account_ids provided".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_refresh_tier(&account_ids)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    record_batch_metrics(
        "batch_refresh_tier",
        "explicit_ids",
        account_ids.len(),
        results.failed.max(0) as usize,
        started_at,
    );

    Ok(Json(json!({
        "success": true,
        "results": results,
    })))
}

/// POST /api/v1/admin/accounts/fast-import - 高性能批量导入
///
/// 支持几千到几万账号的高性能导入：
/// - 真正的批量 SQL INSERT（每批 1000 条）
/// - 并行凭证验证（50 并发）
/// - 自动去重
/// - 事务保证
///
/// 请求体:
/// ```json
/// {
///   "accounts": [
///     {
///       "name": "account-1",
///       "provider": "anthropic",
///       "credential": "sk-ant-xxx",
///       "priority": 50
///     }
///   ],
///   "batch_size": 1000,        // 可选，每批大小
///   "validation_concurrency": 50,  // 可选，验证并发数
///   "skip_duplicates": true,    // 可选，跳过重复
///   "fast_mode": false          // 可选，跳过验证（仅信任数据源）
/// }
/// ```
pub async fn fast_import_accounts(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<FastImportRequest>,
) -> Result<Json<Value>, ApiError> {
    if !PermissionService::is_manager_or_higher(&claims) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            format!("Permission '{}' is required", Permission::AccountWrite),
        ));
    }

    if req.accounts.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "No accounts provided".into(),
        ));
    }

    // 限制最大批次
    if req.accounts.len() > 100_000 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Maximum 100,000 accounts per request".into(),
        ));
    }

    let started_at = std::time::Instant::now();
    let config = BatchImportConfig {
        batch_size: req
            .batch_size
            .unwrap_or(1000)
            .clamp(MIN_FAST_IMPORT_BATCH_SIZE, MAX_FAST_IMPORT_BATCH_SIZE),
        validation_concurrency: req
            .validation_concurrency
            .unwrap_or(50)
            .clamp(MIN_VALIDATION_CONCURRENCY, MAX_VALIDATION_CONCURRENCY),
        skip_duplicates: req.skip_duplicates,
        continue_on_error: true,
    };

    let db = state.db.clone();
    let fast_mode = req.fast_mode;
    let dry_run = req.dry_run;
    let accounts = req.accounts;
    let import_service = BatchImportService::with_config(db, config);
    if dry_run {
        let preview = import_service
            .preview_import(&accounts, fast_mode)
            .await
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        return Ok(Json(json!({
            "success": true,
            "dry_run": true,
            "preview": {
                "total": preview.total,
                "valid": preview.valid,
                "invalid": preview.invalid,
                "duplicate": preview.duplicate,
                "will_import": preview.will_import,
                "skip_duplicates": preview.skip_duplicates,
                "fast_mode": preview.fast_mode,
                "batch_size": preview.batch_size,
                "validation_concurrency": preview.validation_concurrency,
                "duration_ms": preview.duration_ms,
                "throughput_items_per_sec": preview.throughput_items_per_sec,
                "providers": preview.providers,
                "errors": preview.errors,
            }
        })));
    }

    let result = if fast_mode {
        import_service.fast_import(accounts).await
    } else {
        import_service.import_accounts(accounts).await
    }
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "total": result.total,
        "imported": result.imported,
        "skipped": result.skipped,
        "failed": result.failed,
        "duration_ms": result.duration_ms,
        "throughput_items_per_sec": result.throughput_items_per_sec,
        "wall_clock_duration_ms": started_at.elapsed().as_millis() as u64,
        "providers": result.providers,
        "errors": result.errors.iter().take(10).collect::<Vec<_>>(), // 只返回前 10 个错误
        "account_ids": result.account_ids.iter().take(100).collect::<Vec<_>>(), // 只返回前 100 个 ID
    })))
}
