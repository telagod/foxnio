//! 批量操作服务
//!
//! 提供通用的批量操作功能，支持批量创建、更新、删除等操作

use anyhow::Result;
use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::metrics::{BATCH_ERRORS, BATCH_ITEMS_PROCESSED, BATCH_OPERATIONS_TOTAL};

/// 批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// 总数
    pub total: usize,
    /// 成功数
    pub success: usize,
    /// 失败数
    pub failed: usize,
    /// 详细结果
    pub results: Vec<BatchItemResult<T>>,
}

/// 单个批量操作项的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult<T> {
    /// 索引
    pub index: usize,
    /// 操作成功时的数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// 操作失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============ API Key 批量操作 ============

/// 创建 API Key 请求
#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub user_id: Uuid,
    pub name: String,
}

/// API Key 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============ 用户批量操作 ============

/// 创建用户请求
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: String,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// CSV 用户导入记录
#[derive(Debug, Deserialize)]
pub struct CreateUserCsvRecord {
    pub email: String,
    pub password: String,
    pub role: String,
}

// ============ 账号批量操作 ============

/// 更新账号请求
#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub account_id: i64,
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub concurrency: Option<i32>,
}

/// 批量更新账号请求
#[derive(Debug, Deserialize)]
pub struct BatchUpdateRequest {
    pub ids: Vec<Uuid>,
    pub updates: HashMap<String, serde_json::Value>,
}

/// 账号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub priority: i32,
    pub concurrency: i32,
}

// ============ 批量操作服务 ============

/// 批量操作服务
pub struct BatchOperationService {
    db: DatabaseConnection,
}

impl BatchOperationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 批量创建 API Keys
    pub async fn batch_create_api_keys(
        &self,
        requests: Vec<CreateApiKeyRequest>,
        stop_on_error: bool,
    ) -> Result<BatchResult<ApiKeyInfo>> {
        use crate::entity::api_keys;
        use crate::utils::crypto::random_hex;
        use sea_orm::{ActiveModelTrait, Set, TransactionTrait};

        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        let txn = self.db.begin().await?;

        for (index, req) in requests.into_iter().enumerate() {
            let id = Uuid::new_v4();
            let prefix = "sk-fox";
            let hex_part = random_hex(32);
            let key_string = format!("{prefix}_{hex_part}");
            let now = chrono::Utc::now();

            let model = api_keys::ActiveModel {
                id: Set(id),
                user_id: Set(req.user_id),
                key: Set(key_string.clone()),
                name: Set(if req.name.is_empty() {
                    None
                } else {
                    Some(req.name.clone())
                }),
                prefix: Set(prefix.to_string()),
                status: Set("active".to_string()),
                concurrent_limit: Set(None),
                rate_limit_rpm: Set(None),
                allowed_models: Set(None),
                ip_whitelist: Set(None),
                expires_at: Set(None),
                daily_quota: Set(None),
                daily_used_quota: Set(None),
                quota_reset_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now),
            };

            match model.insert(&txn).await {
                Ok(_) => {
                    success += 1;
                    results.push(BatchItemResult {
                        index,
                        data: Some(ApiKeyInfo {
                            id,
                            user_id: req.user_id,
                            name: req.name,
                            prefix: prefix.to_string(),
                            status: "active".to_string(),
                            key: Some(key_string),
                            created_at: now,
                        }),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Insert failed: {e}")),
                    });
                    BATCH_ERRORS.inc();
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        txn.commit().await?;

        BATCH_ITEMS_PROCESSED.inc_by(results.len() as u64);
        BATCH_OPERATIONS_TOTAL.inc();

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 批量创建用户
    pub async fn batch_create_users(
        &self,
        requests: Vec<CreateUserRequest>,
        stop_on_error: bool,
    ) -> Result<BatchResult<UserInfo>> {
        use crate::entity::users;
        use crate::utils::crypto::hash_password;
        use sea_orm::{ActiveModelTrait, Set, TransactionTrait};

        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        let txn = self.db.begin().await?;

        for (index, req) in requests.into_iter().enumerate() {
            let password_hash = match hash_password(&req.password) {
                Ok(h) => h,
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Password hash failed: {e}")),
                    });
                    BATCH_ERRORS.inc();
                    if stop_on_error {
                        break;
                    }
                    continue;
                }
            };

            let id = Uuid::new_v4();
            let now = chrono::Utc::now();

            let model = users::ActiveModel {
                id: Set(id),
                email: Set(req.email.clone()),
                password_hash: Set(password_hash),
                balance: Set(0),
                role: Set("user".to_string()),
                status: Set("active".to_string()),
                totp_secret: Set(None),
                totp_enabled: Set(false),
                created_at: Set(now),
                updated_at: Set(now),
            };

            match model.insert(&txn).await {
                Ok(_) => {
                    success += 1;
                    results.push(BatchItemResult {
                        index,
                        data: Some(UserInfo {
                            id,
                            email: req.email,
                            role: "user".to_string(),
                            created_at: now,
                        }),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Insert failed: {e}")),
                    });
                    BATCH_ERRORS.inc();
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        txn.commit().await?;

        BATCH_ITEMS_PROCESSED.inc_by(results.len() as u64);
        BATCH_OPERATIONS_TOTAL.inc();

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 从 CSV 文件批量导入用户
    pub async fn batch_import_users_csv(&self, csv_content: &str) -> Result<BatchResult<UserInfo>> {
        use crate::entity::users;
        use crate::utils::crypto::hash_password;
        use sea_orm::{ActiveModelTrait, Set, TransactionTrait};

        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(csv_content.as_bytes());

        let txn = self.db.begin().await?;

        for (index, record) in reader.deserialize().enumerate() {
            match record {
                Ok(CreateUserCsvRecord {
                    email,
                    password,
                    role: _,
                }) => {
                    let password_hash = match hash_password(&password) {
                        Ok(h) => h,
                        Err(e) => {
                            failed += 1;
                            results.push(BatchItemResult {
                                index,
                                data: None,
                                error: Some(format!("Password hash failed: {e}")),
                            });
                            BATCH_ERRORS.inc();
                            continue;
                        }
                    };

                    let id = Uuid::new_v4();
                    let now = chrono::Utc::now();

                    let model = users::ActiveModel {
                        id: Set(id),
                        email: Set(email.clone()),
                        password_hash: Set(password_hash),
                        balance: Set(0),
                        role: Set("user".to_string()),
                        status: Set("active".to_string()),
                        totp_secret: Set(None),
                        totp_enabled: Set(false),
                        created_at: Set(now),
                        updated_at: Set(now),
                    };

                    match model.insert(&txn).await {
                        Ok(_) => {
                            success += 1;
                            results.push(BatchItemResult {
                                index,
                                data: Some(UserInfo {
                                    id,
                                    email,
                                    role: "user".to_string(),
                                    created_at: now,
                                }),
                                error: None,
                            });
                        }
                        Err(e) => {
                            failed += 1;
                            results.push(BatchItemResult {
                                index,
                                data: None,
                                error: Some(format!("Insert failed: {e}")),
                            });
                            BATCH_ERRORS.inc();
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("CSV parse error: {e}")),
                    });
                    BATCH_ERRORS.inc();
                }
            }
        }

        txn.commit().await?;

        BATCH_ITEMS_PROCESSED.inc_by(results.len() as u64);
        BATCH_OPERATIONS_TOTAL.inc();

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 批量更新账号
    pub async fn batch_update_accounts(
        &self,
        request: BatchUpdateRequest,
    ) -> Result<BatchResult<AccountInfo>> {
        use crate::entity::accounts;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        let txn = self.db.begin().await?;

        for (index, id) in request.ids.iter().enumerate() {
            let account = match accounts::Entity::find_by_id(*id).one(&txn).await {
                Ok(Some(a)) => a,
                Ok(None) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Account {id} not found")),
                    });
                    BATCH_ERRORS.inc();
                    continue;
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Query failed for {id}: {e}")),
                    });
                    BATCH_ERRORS.inc();
                    continue;
                }
            };

            let mut active: accounts::ActiveModel = account.clone().into();
            let updates = &request.updates;

            if let Some(v) = updates.get("status").and_then(|v| v.as_str()) {
                active.status = Set(v.to_string());
            }
            if let Some(v) = updates.get("priority").and_then(|v| v.as_i64()) {
                active.priority = Set(v as i32);
            }
            if let Some(v) = updates.get("concurrency").and_then(|v| v.as_i64()) {
                active.concurrent_limit = Set(Some(v as i32));
            }
            if let Some(v) = updates.get("rate_limit_rpm").and_then(|v| v.as_i64()) {
                active.rate_limit_rpm = Set(Some(v as i32));
            }
            if let Some(v) = updates.get("group_id").and_then(|v| v.as_i64()) {
                active.group_id = Set(Some(v));
            }
            active.updated_at = Set(chrono::Utc::now());

            match active.update(&txn).await {
                Ok(updated) => {
                    success += 1;
                    results.push(BatchItemResult {
                        index,
                        data: Some(AccountInfo {
                            id: 0, // UUID-based, use 0 as placeholder for i64 field
                            name: updated.name,
                            provider: updated.provider,
                            status: updated.status,
                            priority: updated.priority,
                            concurrency: updated.concurrent_limit.unwrap_or(0),
                        }),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Update failed for {id}: {e}")),
                    });
                    BATCH_ERRORS.inc();
                }
            }
        }

        txn.commit().await?;

        BATCH_ITEMS_PROCESSED.inc_by(results.len() as u64);
        BATCH_OPERATIONS_TOTAL.inc();

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 批量删除 API Keys
    pub async fn batch_delete_api_keys(
        &self,
        ids: Vec<Uuid>,
        stop_on_error: bool,
    ) -> Result<BatchResult<()>> {
        use crate::entity::api_keys;
        use sea_orm::{EntityTrait, ModelTrait, TransactionTrait};

        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        let txn = self.db.begin().await?;

        for (index, id) in ids.into_iter().enumerate() {
            match api_keys::Entity::find_by_id(id).one(&txn).await {
                Ok(Some(key)) => match key.delete(&txn).await {
                    Ok(_) => {
                        success += 1;
                        results.push(BatchItemResult {
                            index,
                            data: Some(()),
                            error: None,
                        });
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(BatchItemResult {
                            index,
                            data: None,
                            error: Some(format!("Delete failed for {id}: {e}")),
                        });
                        BATCH_ERRORS.inc();
                        if stop_on_error {
                            break;
                        }
                    }
                },
                Ok(None) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("API Key {id} not found")),
                    });
                    BATCH_ERRORS.inc();
                    if stop_on_error {
                        break;
                    }
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(format!("Query failed for {id}: {e}")),
                    });
                    BATCH_ERRORS.inc();
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        txn.commit().await?;

        BATCH_ITEMS_PROCESSED.inc_by(results.len() as u64);
        BATCH_OPERATIONS_TOTAL.inc();

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_csv_record() {
        let csv = "email,password,role\ntest@example.com,password123,user\n";
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(csv.as_bytes());

        for result in reader.deserialize() {
            let record: CreateUserCsvRecord = result.unwrap();
            assert_eq!(record.email, "test@example.com");
            assert_eq!(record.password, "password123");
            assert_eq!(record.role, "user");
        }
    }
}
