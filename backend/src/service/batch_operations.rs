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
        let mut succeeded = 0;
        let mut errors = Vec::new();

        // 批量查询现有账号
        let existing = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(req.account_ids.clone()))
            .all(&txn)
            .await?;

        let existing_ids: std::collections::HashSet<Uuid> = existing.iter().map(|a| a.id).collect();

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

        let existing_ids: std::collections::HashSet<Uuid> = existing.iter().map(|a| a.id).collect();

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
        let today_start_utc =
            chrono::DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

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
                                Some(format!("Unknown provider '{}', status: {}", other, account.status))
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
