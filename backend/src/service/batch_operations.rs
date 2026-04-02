//! 账号批量操作服务
//!
//! 提供批量创建、更新、刷新 Token、刷新 Tier 等操作

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
                    errors.push(format!("Failed to encrypt credential for {}: {}", item.name, e));
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
        let mut succeeded = 0;
        let mut errors = Vec::new();

        // 批量查询现有账号
        let existing = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(req.account_ids.clone()))
            .all(&txn)
            .await?;

        let existing_ids: std::collections::HashSet<Uuid> =
            existing.iter().map(|a| a.id).collect();

        // 更新账号
        for id in &req.account_ids {
            if !existing_ids.contains(id) {
                errors.push(format!("Account {} not found", id));
                continue;
            }

            let account = existing.iter().find(|a| &a.id == id).unwrap();
            let mut active: accounts::ActiveModel = account.clone().into();

            if let Some(ref status) = req.updates.status {
                active.status = Set(status.clone());
            }
            if let Some(priority) = req.updates.priority {
                active.priority = Set(priority);
            }
            if let Some(group_id) = req.updates.group_id {
                active.group_id = Set(Some(group_id));
            }
            if let Some(concurrent_limit) = req.updates.concurrent_limit {
                active.concurrent_limit = Set(Some(concurrent_limit));
            }
            if let Some(rate_limit_rpm) = req.updates.rate_limit_rpm {
                active.rate_limit_rpm = Set(Some(rate_limit_rpm));
            }
            active.updated_at = Set(Utc::now());

            match active.update(&txn).await {
                Ok(_) => succeeded += 1,
                Err(e) => errors.push(format!("Failed to update {}: {}", id, e)),
            }
        }

        txn.commit().await?;

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

        // 解析 UUID
        let uuids: Vec<Uuid> = account_ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        if uuids.is_empty() {
            return Ok(account_ids.iter().map(|_| false).collect());
        }

        let txn = self.db.begin().await?;

        // 批量查询现有账号
        let existing = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(uuids.clone()))
            .all(&txn)
            .await?;

        let existing_ids: std::collections::HashSet<Uuid> =
            existing.iter().map(|a| a.id).collect();

        // 更新凭证
        let mut results = Vec::new();
        for id_str in account_ids {
            let id = match Uuid::parse_str(id_str) {
                Ok(id) => id,
                Err(_) => {
                    results.push(false);
                    continue;
                }
            };

            if !existing_ids.contains(&id) {
                results.push(false);
                continue;
            }

            let account = existing.iter().find(|a| a.id == id).unwrap();
            let mut active: accounts::ActiveModel = account.clone().into();
            active.credential = Set(encrypted_credential.clone());
            active.updated_at = Set(Utc::now());

            match active.update(&txn).await {
                Ok(_) => results.push(true),
                Err(_) => results.push(false),
            }
        }

        txn.commit().await?;

        Ok(results)
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

        let txn = self.db.begin().await?;

        // 批量查询 OAuth 账号
        let accounts_list = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .filter(accounts::Column::CredentialType.eq("oauth"))
            .all(&txn)
            .await?;

        let mut succeeded = 0;
        let mut refreshed = Vec::new();
        let errors = Vec::new();

        for account in accounts_list {
            // TODO: 调用实际的 OAuth 服务刷新 Token
            // 目前先返回模拟数据
            refreshed.push(RefreshTokenInfo {
                account_id: account.id,
                account_name: account.name,
                status: account.status,
                refreshed_at: Utc::now(),
            });
            succeeded += 1;
        }

        txn.commit().await?;

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

        let txn = self.db.begin().await?;

        // 批量查询账号
        let accounts_list = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.clone()))
            .all(&txn)
            .await?;

        let mut succeeded = 0;
        let mut tier_info = Vec::new();
        let mut errors = Vec::new();

        for account in &accounts_list {
            // TODO: 调用实际的 API 查询 Tier
            // 目前返回模拟数据
            tier_info.push(TierInfo {
                account_id: account.id,
                account_name: account.name.clone(),
                tier: "pro".to_string(),
                refreshed_at: Utc::now(),
            });
            succeeded += 1;
        }

        // 检查未找到的账号
        let found_ids: std::collections::HashSet<Uuid> =
            accounts_list.iter().map(|a| a.id).collect();
        for id in &account_ids {
            if !found_ids.contains(id) {
                errors.push(format!("Account {} not found", id));
            }
        }

        txn.commit().await?;

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

        // TODO: 从 usage_logs 表查询今日统计
        // 目前返回模拟数据
        let stats: Vec<serde_json::Value> = uuids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "account_id": id.to_string(),
                    "requests": 0,
                    "tokens": 0,
                    "cost": 0.0,
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

        let account_map: std::collections::HashMap<Uuid, accounts::Model> = accounts_list
            .into_iter()
            .map(|a| (a.id, a))
            .collect();

        let results: Vec<(Uuid, bool, Option<String>)> = account_ids
            .iter()
            .map(|id| {
                match account_map.get(id) {
                    Some(account) => {
                        // TODO: 实际测试账号连接
                        // 目前只检查状态
                        let valid = account.status == "active";
                        let error = if valid {
                            None
                        } else {
                            Some(format!("Account status: {}", account.status))
                        };
                        (*id, valid, error)
                    }
                    None => (*id, false, Some("Account not found".to_string())),
                }
            })
            .collect();

        Ok(results)
    }

    /// 获取批量操作进度
    pub async fn get_batch_progress(&self, _operation_id: Uuid) -> Result<Option<BatchProgress>> {
        // TODO: 从缓存或数据库获取进度
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
