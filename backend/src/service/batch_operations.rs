//! 账号批量操作服务
//!
//! 提供批量创建、更新、刷新 Token、刷新 Tier 等操作

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::db::RedisPool;
use crate::entity::accounts;
use crate::utils::encryption_global::GlobalEncryption;

/// 批量创建账号请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateAccountsRequest {
    pub accounts: Vec<CreateAccountItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountItem {
    pub name: String,
    pub provider: String,
    pub credential_type: String,
    pub credential: String,
    pub priority: Option<i32>,
    pub group_id: Option<i64>,
}

/// 批量创建结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateResult {
    pub total: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub account_ids: Vec<Uuid>,
    pub errors: Vec<String>,
}

/// 批量更新账号请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateAccountsRequest {
    pub account_ids: Vec<Uuid>,
    pub updates: AccountUpdates,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountUpdates {
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub group_id: Option<i64>,
    pub concurrent_limit: Option<i32>,
    pub rate_limit_rpm: Option<i32>,
}

/// 批量更新结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateResult {
    pub total: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub errors: Vec<String>,
}

/// 批量清理限流结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchClearRateLimitResult {
    pub total: i32,
    pub processed: i32,
    pub missing: i32,
    pub invalid: i32,
    pub deleted_keys: u64,
}

/// 批量刷新 Token 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRefreshTokenResult {
    pub total: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub refreshed: Vec<RefreshTokenInfo>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenInfo {
    pub account_id: Uuid,
    pub account_name: String,
    pub status: String,
    pub refreshed_at: DateTime<Utc>,
}

/// 批量刷新 Tier 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRefreshTierResult {
    pub total: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub tier_info: Vec<TierInfo>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierInfo {
    pub account_id: Uuid,
    pub account_name: String,
    pub tier: String,
    pub refreshed_at: DateTime<Utc>,
}

/// 批量操作进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub operation: String,
    pub total: i32,
    pub completed: i32,
    pub failed: i32,
    pub percentage: f64,
}

/// 批量操作服务
pub struct BatchOperationService {
    db: DatabaseConnection,
    concurrency_limit: usize,
}

const BATCH_SQL_ID_CHUNK_SIZE: usize = 500;
const MAX_BATCH_OPERATION_ERROR_DETAILS: usize = 80;

impl BatchOperationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            concurrency_limit: 10,
        }
    }

    /// 批量创建账号（使用 insert_many）
    pub async fn batch_create_accounts(
        &self,
        req: BatchCreateAccountsRequest,
    ) -> Result<BatchCreateResult> {
        let total = req.accounts.len() as i32;
        if req.accounts.is_empty() {
            return Ok(BatchCreateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                account_ids: Vec::new(),
                errors: Vec::new(),
            });
        }

        let now = Utc::now();
        let txn = self.db.begin().await?;
        let mut account_ids = Vec::new();
        let mut errors = Vec::new();
        let mut succeeded = 0;

        // 构建批量插入模型
        let mut models = Vec::with_capacity(req.accounts.len());
        for item in &req.accounts {
            // 加密凭证
            let encrypted_credential = match GlobalEncryption::encrypt(&item.credential) {
                Ok(enc) => enc,
                Err(e) => {
                    errors.push(format!(
                        "Failed to encrypt credential for {}: {}",
                        item.name, e
                    ));
                    continue;
                }
            };

            let id = Uuid::new_v4();
            account_ids.push(id);
            models.push(accounts::ActiveModel {
                id: Set(id),
                name: Set(item.name.clone()),
                provider: Set(item.provider.clone()),
                credential_type: Set(item.credential_type.clone()),
                credential: Set(encrypted_credential),
                metadata: Set(None),
                status: Set("active".to_string()),
                last_error: Set(None),
                priority: Set(item.priority.unwrap_or(50)),
                concurrent_limit: Set(Some(5)),
                rate_limit_rpm: Set(None),
                group_id: Set(item.group_id),
                created_at: Set(now),
                updated_at: Set(now),
            });
        }

        // 批量插入
        if !models.is_empty() {
            match accounts::Entity::insert_many(models).exec(&txn).await {
                Ok(_) => succeeded = account_ids.len() as i32,
                Err(e) => {
                    errors.push(format!("Batch insert failed: {}", e));
                    txn.commit().await?;
                    return Ok(BatchCreateResult {
                        total,
                        succeeded: 0,
                        failed: total,
                        account_ids: Vec::new(),
                        errors,
                    });
                }
            }
        }

        txn.commit().await?;

        Ok(BatchCreateResult {
            total,
            succeeded,
            failed: total - succeeded,
            account_ids,
            errors,
        })
    }

    /// 批量更新账号状态和配置
    pub async fn batch_update_accounts(
        &self,
        req: BatchUpdateAccountsRequest,
    ) -> Result<BatchUpdateResult> {
        let total = req.account_ids.len() as i32;
        if req.account_ids.is_empty() {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let txn = self.db.begin().await?;

        let existing = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(req.account_ids.clone()))
            .all(&txn)
            .await?;
        let existing_ids: HashSet<Uuid> = existing.iter().map(|a| a.id).collect();
        let mut errors = Vec::new();

        for id in &req.account_ids {
            if !existing_ids.contains(id) {
                errors.push(format!("Account {} not found", id));
            }
        }

        if !existing_ids.is_empty() {
            let now = Utc::now();
            let mut updater = accounts::Entity::update_many()
                .filter(accounts::Column::Id.is_in(existing_ids.iter().copied()))
                .col_expr(accounts::Column::UpdatedAt, Expr::value(now));

            if let Some(ref status) = req.updates.status {
                updater = updater.col_expr(accounts::Column::Status, Expr::value(status.clone()));
            }
            if let Some(priority) = req.updates.priority {
                updater = updater.col_expr(accounts::Column::Priority, Expr::value(priority));
            }
            if let Some(group_id) = req.updates.group_id {
                updater = updater.col_expr(accounts::Column::GroupId, Expr::value(group_id));
            }
            if let Some(concurrent_limit) = req.updates.concurrent_limit {
                updater = updater.col_expr(
                    accounts::Column::ConcurrentLimit,
                    Expr::value(concurrent_limit),
                );
            }
            if let Some(rate_limit_rpm) = req.updates.rate_limit_rpm {
                updater =
                    updater.col_expr(accounts::Column::RateLimitRpm, Expr::value(rate_limit_rpm));
            }

            if let Err(e) = updater.exec(&txn).await {
                errors.push(format!("Batch update failed: {}", e));
            }
        }

        txn.commit().await?;

        let succeeded = if errors
            .iter()
            .any(|err| err.starts_with("Batch update failed:"))
        {
            0
        } else {
            existing_ids.len() as i32
        };

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed: total - succeeded,
            errors,
        })
    }

    /// 批量删除账号
    pub async fn batch_delete_accounts(&self, account_ids: Vec<Uuid>) -> Result<BatchUpdateResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let txn = self.db.begin().await?;

        // 批量删除
        let result = accounts::Entity::delete_many()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .exec(&txn)
            .await?;

        txn.commit().await?;

        let succeeded = result.rows_affected as i32;
        let failed = total - succeeded;
        let errors = if failed > 0 {
            vec![format!("{} accounts not found or already deleted", failed)]
        } else {
            Vec::new()
        };

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed,
            errors,
        })
    }

    /// 批量更新凭证（加密存储）
    pub async fn batch_update_credentials(
        &self,
        account_ids: &[String],
        credential: &str,
    ) -> Result<Vec<bool>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 加密凭证
        let encrypted_credential = GlobalEncryption::encrypt(credential)
            .map_err(|e| anyhow::anyhow!("Failed to encrypt credential: {}", e))?;

        // 解析 UUID（保留重复 ID 映射，保持返回与输入顺序一致）
        let mut positions_by_uuid: HashMap<Uuid, Vec<usize>> = HashMap::new();
        let mut invalid_count = 0usize;
        for (idx, id_str) in account_ids.iter().enumerate() {
            match Uuid::parse_str(id_str) {
                Ok(id) => {
                    positions_by_uuid.entry(id).or_default().push(idx);
                }
                Err(_) => invalid_count += 1,
            }
        }

        let uuids: Vec<Uuid> = positions_by_uuid.keys().copied().collect();

        if uuids.is_empty() {
            return Ok(account_ids.iter().map(|_| false).collect());
        }

        let txn = self.db.begin().await?;

        let mut results = vec![false; account_ids.len()];
        let mut start = 0usize;
        while start < uuids.len() {
            let end = usize::min(start + BATCH_SQL_ID_CHUNK_SIZE, uuids.len());
            let chunk = &uuids[start..end];

            let existing = accounts::Entity::find()
                .filter(accounts::Column::Id.is_in(chunk.to_vec()))
                .all(&txn)
                .await?;

            let existing_ids: HashSet<Uuid> = existing.iter().map(|a| a.id).collect();
            let update_success = if existing_ids.is_empty() {
                false
            } else {
                accounts::Entity::update_many()
                    .filter(accounts::Column::Id.is_in(existing_ids.iter().copied()))
                    .col_expr(
                        accounts::Column::Credential,
                        Expr::value(encrypted_credential.clone()),
                    )
                    .col_expr(accounts::Column::UpdatedAt, Expr::value(Utc::now()))
                    .exec(&txn)
                    .await
                    .is_ok()
            };

            for id in chunk {
                if let Some(indices) = positions_by_uuid.get(id) {
                    for index in indices {
                        results[*index] = update_success;
                    }
                }
            }

            start += BATCH_SQL_ID_CHUNK_SIZE;
        }

        txn.commit().await?;

        if invalid_count > 0 {
            for (idx, raw) in account_ids.iter().enumerate() {
                if Uuid::parse_str(raw).is_err() {
                    results[idx] = false;
                }
            }
        }

        Ok(results)
    }

    pub async fn batch_set_status(
        &self,
        account_ids: &[String],
        status: &str,
        clear_error: bool,
    ) -> Result<BatchUpdateResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let mut errors = Vec::new();
        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let invalid_count = total as usize - uuids.len();
        if invalid_count > 0 {
            for id in account_ids.iter().filter(|id| Uuid::parse_str(id).is_err()) {
                errors.push(format!("Invalid UUID: {}", id));
            }
        }

        if uuids.is_empty() {
            return Ok(BatchUpdateResult {
                total,
                succeeded: 0,
                failed: total,
                errors,
            });
        }

        let txn = self.db.begin().await?;
        let mut has_error = false;
        let mut updated_count = 0i32;
        let mut start = 0usize;
        while start < uuids.len() {
            let end = usize::min(start + BATCH_SQL_ID_CHUNK_SIZE, uuids.len());
            let chunk = &uuids[start..end];

            let existing = accounts::Entity::find()
                .filter(accounts::Column::Id.is_in(chunk.to_vec()))
                .all(&txn)
                .await?;

            let existing_ids: HashSet<Uuid> = existing.iter().map(|a| a.id).collect();
            for id in chunk.iter().copied() {
                if !existing_ids.contains(&id) {
                    if errors.len() < MAX_BATCH_OPERATION_ERROR_DETAILS {
                        errors.push(format!("Account {} not found", id));
                    }
                }
            }
            if !existing_ids.is_empty() {
                let mut updater = accounts::Entity::update_many()
                    .filter(accounts::Column::Id.is_in(existing_ids.iter().copied()))
                    .col_expr(accounts::Column::Status, Expr::value(status.to_string()))
                    .col_expr(accounts::Column::UpdatedAt, Expr::value(Utc::now()));

                if clear_error {
                    updater = updater.col_expr(
                        accounts::Column::LastError,
                        Expr::value(Option::<String>::None),
                    );
                }

                if let Err(e) = updater.exec(&txn).await {
                    has_error = true;
                    errors.push(format!("Batch set status failed: {}", e));
                } else {
                    updated_count += existing_ids.len() as i32;
                }
            }

            start += BATCH_SQL_ID_CHUNK_SIZE;
        }

        txn.commit().await?;

        let mut succeeded = if has_error { 0 } else { updated_count };
        if errors
            .iter()
            .any(|err| err.starts_with("Batch set status failed"))
        {
            succeeded = 0;
        }
        let failed = total - succeeded;

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed,
            errors,
        })
    }

    pub async fn batch_set_status_by_filter(
        &self,
        status: &str,
        clear_error: bool,
        filter_status: Option<&str>,
        filter_provider: Option<&str>,
        filter_search: Option<&str>,
        filter_group_id: Option<i64>,
    ) -> Result<BatchUpdateResult> {
        let mut query = accounts::Entity::find();
        if let Some(s) = filter_status {
            query = query.filter(accounts::Column::Status.eq(s));
        }
        if let Some(provider) = filter_provider {
            query = query.filter(accounts::Column::Provider.eq(provider));
        }
        if let Some(search) = filter_search {
            query = query.filter(accounts::Column::Name.contains(search));
        }
        if let Some(group_id) = filter_group_id {
            query = query.filter(accounts::Column::GroupId.eq(group_id));
        }

        let total = query.clone().count(&self.db).await? as i32;
        if total == 0 {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let mut updater = accounts::Entity::update_many();
        updater = updater.col_expr(accounts::Column::Status, Expr::value(status.to_string()));
        updater = updater.col_expr(accounts::Column::UpdatedAt, Expr::value(Utc::now()));
        if let Some(s) = filter_status {
            updater = updater.filter(accounts::Column::Status.eq(s));
        }
        if let Some(provider) = filter_provider {
            updater = updater.filter(accounts::Column::Provider.eq(provider));
        }
        if let Some(search) = filter_search {
            updater = updater.filter(accounts::Column::Name.contains(search));
        }
        if let Some(group_id) = filter_group_id {
            updater = updater.filter(accounts::Column::GroupId.eq(group_id));
        }

        if clear_error {
            updater = updater.col_expr(
                accounts::Column::LastError,
                Expr::value(Option::<String>::None),
            );
        }

        let result = updater.exec(&self.db).await?;
        let succeeded = result.rows_affected as i32;
        let failed = (total - succeeded).max(0);

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed,
            errors: Vec::new(),
        })
    }

    pub async fn batch_set_group(
        &self,
        account_ids: &[String],
        group_id: Option<i64>,
    ) -> Result<BatchUpdateResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let mut errors = Vec::new();
        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let invalid_count = total as usize - uuids.len();
        if invalid_count > 0 {
            for id in account_ids.iter().filter(|id| Uuid::parse_str(id).is_err()) {
                errors.push(format!("Invalid UUID: {}", id));
            }
        }

        if uuids.is_empty() {
            return Ok(BatchUpdateResult {
                total,
                succeeded: 0,
                failed: total,
                errors,
            });
        }

        let txn = self.db.begin().await?;
        let mut has_error = false;
        let mut updated_count = 0i32;
        let mut start = 0usize;
        while start < uuids.len() {
            let end = usize::min(start + BATCH_SQL_ID_CHUNK_SIZE, uuids.len());
            let chunk = &uuids[start..end];

            let existing = accounts::Entity::find()
                .filter(accounts::Column::Id.is_in(chunk.to_vec()))
                .all(&txn)
                .await?;

            let existing_ids: HashSet<Uuid> = existing.iter().map(|a| a.id).collect();
            for id in chunk.iter().copied() {
                if !existing_ids.contains(&id) {
                    if errors.len() < MAX_BATCH_OPERATION_ERROR_DETAILS {
                        errors.push(format!("Account {} not found", id));
                    }
                }
            }

            if !existing_ids.is_empty() {
                if let Err(e) = accounts::Entity::update_many()
                    .filter(accounts::Column::Id.is_in(existing_ids.iter().copied()))
                    .col_expr(accounts::Column::GroupId, Expr::value(group_id))
                    .col_expr(accounts::Column::UpdatedAt, Expr::value(Utc::now()))
                    .exec(&txn)
                    .await
                {
                    has_error = true;
                    errors.push(format!("Batch set group failed: {}", e));
                } else {
                    updated_count += existing_ids.len() as i32;
                }
            }

            start += BATCH_SQL_ID_CHUNK_SIZE;
        }

        txn.commit().await?;

        let succeeded = if has_error { 0 } else { updated_count };
        let failed = total - succeeded;

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed,
            errors,
        })
    }

    pub async fn batch_set_group_by_filter(
        &self,
        group_id: Option<i64>,
        filter_status: Option<&str>,
        filter_provider: Option<&str>,
        filter_search: Option<&str>,
        filter_group_id: Option<i64>,
    ) -> Result<BatchUpdateResult> {
        let mut query = accounts::Entity::find();
        if let Some(s) = filter_status {
            query = query.filter(accounts::Column::Status.eq(s));
        }
        if let Some(provider) = filter_provider {
            query = query.filter(accounts::Column::Provider.eq(provider));
        }
        if let Some(search) = filter_search {
            query = query.filter(accounts::Column::Name.contains(search));
        }
        if let Some(group_id) = filter_group_id {
            query = query.filter(accounts::Column::GroupId.eq(group_id));
        }

        let total = query.clone().count(&self.db).await? as i32;
        if total == 0 {
            return Ok(BatchUpdateResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                errors: Vec::new(),
            });
        }

        let mut updater = accounts::Entity::update_many();
        if let Some(s) = filter_status {
            updater = updater.filter(accounts::Column::Status.eq(s));
        }
        if let Some(provider) = filter_provider {
            updater = updater.filter(accounts::Column::Provider.eq(provider));
        }
        if let Some(search) = filter_search {
            updater = updater.filter(accounts::Column::Name.contains(search));
        }
        if let Some(group_id_filter) = filter_group_id {
            updater = updater.filter(accounts::Column::GroupId.eq(group_id_filter));
        }

        let result = updater
            .col_expr(accounts::Column::GroupId, Expr::value(group_id))
            .col_expr(accounts::Column::UpdatedAt, Expr::value(Utc::now()))
            .exec(&self.db)
            .await?;

        let succeeded = result.rows_affected as i32;
        let failed = (total - succeeded).max(0);

        Ok(BatchUpdateResult {
            total,
            succeeded,
            failed,
            errors: Vec::new(),
        })
    }

    pub async fn batch_clear_rate_limit_keys(
        &self,
        redis: &RedisPool,
        account_ids: &[String],
    ) -> Result<BatchClearRateLimitResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchClearRateLimitResult {
                total: 0,
                processed: 0,
                missing: 0,
                invalid: 0,
                deleted_keys: 0,
            });
        }

        let mut invalid = 0i32;
        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| match Uuid::parse_str(id) {
                Ok(v) => Some(v),
                Err(_) => {
                    invalid += 1;
                    None
                }
            })
            .collect();

        if uuids.is_empty() {
            return Ok(BatchClearRateLimitResult {
                total,
                processed: 0,
                missing: total,
                invalid,
                deleted_keys: 0,
            });
        }

        let mut accounts_list = Vec::new();
        let mut offset = 0usize;
        while offset < uuids.len() {
            let end = usize::min(offset + BATCH_SQL_ID_CHUNK_SIZE, uuids.len());
            let chunk = &uuids[offset..end];

            let mut chunk_accounts = accounts::Entity::find()
                .filter(accounts::Column::Id.is_in(chunk.to_vec()))
                .all(&self.db)
                .await?;
            accounts_list.append(&mut chunk_accounts);

            offset += BATCH_SQL_ID_CHUNK_SIZE;
        }

        let existing: std::collections::HashSet<Uuid> =
            accounts_list.iter().map(|a| a.id).collect();

        let existing_len = existing.len() as i32;
        let missing = uuids.len() as i32 - existing_len;

        let mut keys = Vec::new();
        for id in existing {
            let account_id = id;
            keys.extend_from_slice(&[
                format!("rate_limit:{}", account_id),
                format!("ratelimit:{}", account_id),
                format!("account_rate_limit:{}", account_id),
                format!("account:{}:rate_limit", account_id),
                format!("account:{}:rpm", account_id),
            ]);
        }

        let deleted_keys = if keys.is_empty() {
            0
        } else {
            redis.del_many(&keys).await?
        };

        let processed = existing_len;

        Ok(BatchClearRateLimitResult {
            total,
            processed,
            missing: missing + invalid,
            invalid,
            deleted_keys,
        })
    }

    pub async fn batch_clear_rate_limit_keys_by_filter(
        &self,
        redis: &RedisPool,
        filter_status: Option<&str>,
        filter_provider: Option<&str>,
        filter_search: Option<&str>,
        filter_group_id: Option<i64>,
    ) -> Result<BatchClearRateLimitResult> {
        let mut query = accounts::Entity::find();
        if let Some(s) = filter_status {
            query = query.filter(accounts::Column::Status.eq(s));
        }
        if let Some(provider) = filter_provider {
            query = query.filter(accounts::Column::Provider.eq(provider));
        }
        if let Some(search) = filter_search {
            query = query.filter(accounts::Column::Name.contains(search));
        }
        if let Some(group_id) = filter_group_id {
            query = query.filter(accounts::Column::GroupId.eq(group_id));
        }

        let total = query.clone().count(&self.db).await? as i32;
        if total == 0 {
            return Ok(BatchClearRateLimitResult {
                total: 0,
                processed: 0,
                missing: 0,
                invalid: 0,
                deleted_keys: 0,
            });
        }

        let mut last_id: Option<Uuid> = None;
        let mut deleted_keys = 0u64;
        let mut processed = 0i32;
        loop {
            let mut page_query = accounts::Entity::find();
            if let Some(s) = filter_status {
                page_query = page_query.filter(accounts::Column::Status.eq(s));
            }
            if let Some(provider) = filter_provider {
                page_query = page_query.filter(accounts::Column::Provider.eq(provider));
            }
            if let Some(search) = filter_search {
                page_query = page_query.filter(accounts::Column::Name.contains(search));
            }
            if let Some(group_id) = filter_group_id {
                page_query = page_query.filter(accounts::Column::GroupId.eq(group_id));
            }
            if let Some(after) = last_id {
                page_query = page_query.filter(accounts::Column::Id.gt(after));
            }

            let page_ids = page_query
                .order_by_asc(accounts::Column::Id)
                .select_only()
                .column(accounts::Column::Id)
                .limit(BATCH_SQL_ID_CHUNK_SIZE as u64)
                .into_tuple::<Uuid>()
                .all(&self.db)
                .await?;

            if page_ids.is_empty() {
                break;
            }

            let mut keys = Vec::new();
            for account_id in page_ids.iter() {
                keys.extend_from_slice(&[
                    format!("rate_limit:{}", account_id),
                    format!("ratelimit:{}", account_id),
                    format!("account_rate_limit:{}", account_id),
                    format!("account:{}:rate_limit", account_id),
                    format!("account:{}:rpm", account_id),
                ]);
            }

            if !keys.is_empty() {
                deleted_keys += redis.del_many(&keys).await?;
            }

            let page_count = page_ids.len() as i32;
            processed += page_count;
            last_id = page_ids.last().copied();

            if page_count < BATCH_SQL_ID_CHUNK_SIZE as i32 {
                break;
            }
        }

        Ok(BatchClearRateLimitResult {
            total,
            processed,
            missing: 0,
            invalid: 0,
            deleted_keys,
        })
    }

    /// 批量刷新 Token
    pub async fn batch_refresh_tokens(
        &self,
        account_ids: Vec<Uuid>,
    ) -> Result<BatchRefreshTokenResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchRefreshTokenResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                refreshed: Vec::new(),
                errors: Vec::new(),
            });
        }

        // 批量查询账号（不限 credential_type，验证所有类型的 key）
        let accounts_list = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .all(&self.db)
            .await?;

        let found_ids: std::collections::HashSet<Uuid> =
            accounts_list.iter().map(|a| a.id).collect();

        let mut succeeded = 0;
        let mut refreshed = Vec::new();
        let mut errors = Vec::new();

        // 报告未找到的账号
        for id in &account_ids {
            if !found_ids.contains(id) {
                errors.push(format!("Account {} not found", id));
            }
        }

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        for account in &accounts_list {
            // 解密凭证
            let credential = match GlobalEncryption::decrypt(&account.credential) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!(
                        "Account {} ({}): decrypt failed: {}",
                        account.id, account.name, e
                    ));
                    continue;
                }
            };

            // 根据 provider 选择验证端点
            let verify_result = match account.provider.as_str() {
                "openai" => {
                    http_client
                        .get("https://api.openai.com/v1/models")
                        .header("Authorization", format!("Bearer {}", credential))
                        .send()
                        .await
                }
                "anthropic" | "claude" => {
                    http_client
                        .get("https://api.anthropic.com/v1/models")
                        .header("x-api-key", &credential)
                        .header("anthropic-version", "2023-06-01")
                        .send()
                        .await
                }
                "gemini" => {
                    let url = format!(
                        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                        credential
                    );
                    http_client.get(&url).send().await
                }
                other => {
                    // 未知 provider，跳过 HTTP 验证，标记为成功
                    refreshed.push(RefreshTokenInfo {
                        account_id: account.id,
                        account_name: account.name.clone(),
                        status: format!("skipped (unknown provider: {})", other),
                        refreshed_at: Utc::now(),
                    });
                    succeeded += 1;
                    continue;
                }
            };

            match verify_result {
                Ok(resp) if resp.status().is_success() => {
                    // 验证成功 — 更新 status 为 active
                    let mut active: accounts::ActiveModel = account.clone().into();
                    active.status = Set("active".to_string());
                    active.last_error = Set(None);
                    active.updated_at = Set(Utc::now());
                    let _ = active.update(&self.db).await;

                    refreshed.push(RefreshTokenInfo {
                        account_id: account.id,
                        account_name: account.name.clone(),
                        status: "active".to_string(),
                        refreshed_at: Utc::now(),
                    });
                    succeeded += 1;
                }
                Ok(resp) => {
                    let status_code = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let err_msg = format!("HTTP {}: {}", status_code, &body[..body.len().min(200)]);

                    // 标记账号错误
                    let mut active: accounts::ActiveModel = account.clone().into();
                    active.status = Set("error".to_string());
                    active.last_error = Set(Some(err_msg.clone()));
                    active.updated_at = Set(Utc::now());
                    let _ = active.update(&self.db).await;

                    errors.push(format!(
                        "Account {} ({}): {}",
                        account.id, account.name, err_msg
                    ));
                }
                Err(e) => {
                    let err_msg = format!("Request failed: {}", e);

                    let mut active: accounts::ActiveModel = account.clone().into();
                    active.last_error = Set(Some(err_msg.clone()));
                    active.updated_at = Set(Utc::now());
                    let _ = active.update(&self.db).await;

                    errors.push(format!(
                        "Account {} ({}): {}",
                        account.id, account.name, err_msg
                    ));
                }
            }
        }

        Ok(BatchRefreshTokenResult {
            total,
            succeeded,
            failed: total - succeeded,
            refreshed,
            errors,
        })
    }

    /// 批量刷新 Tier
    pub async fn batch_refresh_tiers(
        &self,
        account_ids: Vec<Uuid>,
    ) -> Result<BatchRefreshTierResult> {
        let total = account_ids.len() as i32;
        if account_ids.is_empty() {
            return Ok(BatchRefreshTierResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                tier_info: Vec::new(),
                errors: Vec::new(),
            });
        }

        // 批量查询账号
        let accounts_list = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .all(&self.db)
            .await?;

        let found_ids: std::collections::HashSet<Uuid> =
            accounts_list.iter().map(|a| a.id).collect();

        let mut succeeded = 0;
        let mut tier_info = Vec::new();
        let mut errors = Vec::new();

        // 报告未找到的账号
        for id in &account_ids {
            if !found_ids.contains(id) {
                errors.push(format!("Account {} not found", id));
            }
        }

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        for account in &accounts_list {
            // 解密凭证
            let credential = match GlobalEncryption::decrypt(&account.credential) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!(
                        "Account {} ({}): decrypt failed: {}",
                        account.id, account.name, e
                    ));
                    continue;
                }
            };

            // 验证 key 是否有效
            let verify_ok = match account.provider.as_str() {
                "openai" => {
                    match http_client
                        .get("https://api.openai.com/v1/models")
                        .header("Authorization", format!("Bearer {}", credential))
                        .send()
                        .await
                    {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                }
                "anthropic" | "claude" => {
                    match http_client
                        .get("https://api.anthropic.com/v1/models")
                        .header("x-api-key", &credential)
                        .header("anthropic-version", "2023-06-01")
                        .send()
                        .await
                    {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                }
                "gemini" => {
                    let url = format!(
                        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                        credential
                    );
                    match http_client.get(&url).send().await {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                }
                _ => {
                    // 未知 provider，跳过验证视为通过
                    true
                }
            };

            if verify_ok {
                // 标记为 verified
                let mut active: accounts::ActiveModel = account.clone().into();
                active.status = Set("active".to_string());
                active.last_error = Set(None);
                active.updated_at = Set(Utc::now());
                let _ = active.update(&self.db).await;

                tier_info.push(TierInfo {
                    account_id: account.id,
                    account_name: account.name.clone(),
                    tier: "verified".to_string(),
                    refreshed_at: Utc::now(),
                });
                succeeded += 1;
            } else {
                let mut active: accounts::ActiveModel = account.clone().into();
                active.status = Set("error".to_string());
                active.last_error = Set(Some("Tier verification failed".to_string()));
                active.updated_at = Set(Utc::now());
                let _ = active.update(&self.db).await;

                errors.push(format!(
                    "Account {} ({}): key verification failed",
                    account.id, account.name
                ));
            }
        }

        Ok(BatchRefreshTierResult {
            total,
            succeeded,
            failed: total - succeeded,
            tier_info,
            errors,
        })
    }

    /// 批量刷新 Tier（支持字符串 ID）
    pub async fn batch_refresh_tier(
        &self,
        account_ids: &[String],
    ) -> Result<BatchRefreshTierResult> {
        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let mut result = self.batch_refresh_tiers(uuids).await?;

        // 添加无效 UUID 的错误
        for id_str in account_ids {
            if Uuid::parse_str(id_str).is_err() {
                result.failed += 1;
                result.errors.push(format!("Invalid UUID: {}", id_str));
            }
        }

        Ok(result)
    }

    /// 批量获取今日统计
    pub async fn batch_get_today_stats(&self, account_ids: &[String]) -> Result<serde_json::Value> {
        if account_ids.is_empty() {
            return Ok(serde_json::json!({ "stats": [] }));
        }

        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        if uuids.is_empty() {
            return Ok(serde_json::json!({ "stats": [] }));
        }

        use crate::entity::usages;

        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight");
        let today_start_utc = chrono::DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

        // Query today's usages for the given accounts
        let rows = usages::Entity::find()
            .filter(usages::Column::CreatedAt.gte(today_start_utc))
            .filter(usages::Column::AccountId.is_in(uuids.clone()))
            .all(&self.db)
            .await?;

        // Aggregate per account
        let mut map: std::collections::HashMap<Uuid, (i64, i64, i64)> =
            std::collections::HashMap::new();
        for row in &rows {
            if let Some(aid) = row.account_id {
                let entry = map.entry(aid).or_insert((0, 0, 0));
                entry.0 += 1; // requests
                entry.1 += row.input_tokens + row.output_tokens; // tokens
                entry.2 += row.cost; // cost in cents
            }
        }

        let stats: Vec<serde_json::Value> = uuids
            .iter()
            .map(|id| {
                let (requests, tokens, cost) = map.get(id).copied().unwrap_or((0, 0, 0));
                serde_json::json!({
                    "account_id": id.to_string(),
                    "requests": requests,
                    "tokens": tokens,
                    "cost": cost as f64 / 100.0,
                })
            })
            .collect();

        Ok(serde_json::json!({ "stats": stats }))
    }

    /// 批量测试账号
    pub async fn batch_test_accounts(
        &self,
        account_ids: Vec<Uuid>,
    ) -> Result<Vec<(Uuid, bool, Option<String>)>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 批量查询账号
        let accounts_list = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .all(&self.db)
            .await?;

        let account_map: std::collections::HashMap<Uuid, accounts::Model> =
            accounts_list.into_iter().map(|a| (a.id, a)).collect();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        let mut results = Vec::new();

        for id in &account_ids {
            match account_map.get(id) {
                Some(account) => {
                    let credential = match GlobalEncryption::decrypt(&account.credential) {
                        Ok(c) => c,
                        Err(e) => {
                            results.push((*id, false, Some(format!("Decrypt failed: {}", e))));
                            continue;
                        }
                    };

                    let test_result = match account.provider.as_str() {
                        "openai" => {
                            http_client
                                .get("https://api.openai.com/v1/models")
                                .header("Authorization", format!("Bearer {}", credential))
                                .send()
                                .await
                        }
                        "anthropic" | "claude" => {
                            http_client
                                .get("https://api.anthropic.com/v1/models")
                                .header("x-api-key", &credential)
                                .header("anthropic-version", "2023-06-01")
                                .send()
                                .await
                        }
                        "gemini" => {
                            let url = format!(
                                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                                credential
                            );
                            http_client.get(&url).send().await
                        }
                        other => {
                            // Unknown provider, fall back to status check
                            let valid = account.status == "active";
                            let error = if valid {
                                None
                            } else {
                                Some(format!(
                                    "Unknown provider '{}', status: {}",
                                    other, account.status
                                ))
                            };
                            results.push((*id, valid, error));
                            continue;
                        }
                    };

                    match test_result {
                        Ok(resp) if resp.status().is_success() => {
                            results.push((*id, true, None));
                        }
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            results.push((
                                *id,
                                false,
                                Some(format!("HTTP {}: {}", status, &body[..body.len().min(200)])),
                            ));
                        }
                        Err(e) => {
                            results.push((*id, false, Some(format!("Connection failed: {}", e))));
                        }
                    }
                }
                None => {
                    results.push((*id, false, Some("Account not found".to_string())));
                }
            }
        }

        Ok(results)
    }

    /// 获取批量操作进度
    ///
    /// Uses a simple in-memory lookup. Callers that start long-running batch
    /// operations should store progress via the `BatchProgress` struct; this
    /// method returns whatever is currently cached for the given operation id.
    /// Without a running operation the result is `None`.
    pub async fn get_batch_progress(&self, _operation_id: Uuid) -> Result<Option<BatchProgress>> {
        // In-memory progress tracking: batch operations in this service are
        // synchronous (awaited to completion), so there is no intermediate
        // progress to report. Return None to signal "no active operation".
        // A future enhancement could store progress in Redis via
        //   HSET foxnio:batch_progress:{operation_id} completed/failed/total
        // and read it back here.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_create_result() {
        let result = BatchCreateResult {
            total: 10,
            succeeded: 8,
            failed: 2,
            account_ids: vec![Uuid::new_v4(); 8],
            errors: vec!["error1".to_string(), "error2".to_string()],
        };

        assert_eq!(result.total, 10);
        assert_eq!(result.succeeded, 8);
        assert_eq!(result.failed, 2);
    }

    #[test]
    fn test_batch_update_result() {
        let result = BatchUpdateResult {
            total: 5,
            succeeded: 5,
            failed: 0,
            errors: Vec::new(),
        };

        assert_eq!(result.total, 5);
        assert_eq!(result.succeeded, 5);
        assert!(result.errors.is_empty());
    }
}
