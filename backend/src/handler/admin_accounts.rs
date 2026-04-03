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
use crate::gateway::SharedState;
use crate::service::account::AccountService;
use crate::service::batch_import::{BatchImportConfig, BatchImportService, ImportAccountItem};
use crate::service::batch_operations::{
    BatchCreateAccountsRequest, BatchOperationService, CreateAccountItem,
};
use crate::service::permission::Permission;
use crate::service::user::Claims;

/// 批量更新凭证请求
#[derive(Debug, Deserialize)]
pub struct BatchUpdateCredentialsRequest {
    pub account_ids: Vec<String>,
    pub credential: String,
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
}

fn default_skip_duplicates() -> bool {
    true
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

    let batch_service = BatchOperationService::new(state.db.clone());
    let req = BatchCreateAccountsRequest { accounts: items };
    let results = batch_service
        .batch_create_accounts(req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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

    let batch_service = BatchOperationService::new(state.db.clone());
    let req = BatchCreateAccountsRequest { accounts };
    let results = batch_service
        .batch_create_accounts(req)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_update_credentials(&req.account_ids, &req.credential)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "updated": results.iter().filter(|r| **r).count(),
        "total": results.len(),
        "results": results,
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

    let batch_service = BatchOperationService::new(state.db.clone());
    let results = batch_service
        .batch_refresh_tier(&account_ids)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

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

    let config = BatchImportConfig {
        batch_size: req.batch_size.unwrap_or(1000),
        validation_concurrency: req.validation_concurrency.unwrap_or(50),
        skip_duplicates: req.skip_duplicates,
        continue_on_error: true,
    };

    let db = state.db.clone();
    let result = if req.fast_mode {
        // 快速模式：跳过验证，直接导入
        let import_service = BatchImportService::with_config(db, config);
        import_service
            .fast_import(req.accounts)
            .await
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        // 正常模式：验证 + 导入
        let import_service = BatchImportService::with_config(db, config);
        import_service
            .import_accounts(req.accounts)
            .await
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(json!({
        "success": true,
        "total": result.total,
        "imported": result.imported,
        "skipped": result.skipped,
        "failed": result.failed,
        "duration_ms": result.duration_ms,
        "errors": result.errors.iter().take(10).collect::<Vec<_>>(), // 只返回前 10 个错误
        "account_ids": result.account_ids.iter().take(100).collect::<Vec<_>>(), // 只返回前 100 个 ID
    })))
}
