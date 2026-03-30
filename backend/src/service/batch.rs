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

/// 批量创建 API Key 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub user_id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 批量更新账号请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub ids: Vec<Uuid>,
    pub updates: HashMap<String, serde_json::Value>,
}

/// CSV 导入用户记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserCsvRecord {
    pub email: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "user".to_string()
}

impl From<CreateUserCsvRecord> for CreateUserRequest {
    fn from(record: CreateUserCsvRecord) -> Self {
        Self {
            email: record.email,
            password: record.password,
            role: record.role,
        }
    }
}

/// 创建用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: String,
}

/// 批量操作服务
pub struct BatchOperationService {
    db: DatabaseConnection,
}

impl BatchOperationService {
    /// 创建新的批量操作服务
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 批量创建 API Keys
    ///
    /// # Arguments
    /// * `requests` - 创建请求列表
    /// * `stop_on_error` - 是否在遇到错误时停止
    pub async fn batch_create_api_keys(
        &self,
        requests: Vec<CreateApiKeyRequest>,
        stop_on_error: bool,
    ) -> Result<BatchResult<ApiKeyInfo>> {
        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        for (index, req) in requests.into_iter().enumerate() {
            match self.create_api_key(req).await {
                Ok(key) => {
                    success += 1;
                    results.push(BatchItemResult {
                        index,
                        data: Some(key),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(e.to_string()),
                    });
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 创建单个 API Key
    async fn create_api_key(&self, req: CreateApiKeyRequest) -> Result<ApiKeyInfo> {
        // 生成 API Key
        let key = format!("sk-{}", Uuid::new_v4().to_string().replace('-', ""));
        let key_prefix = key[..10].to_string();
        
        // 使用 SHA256 哈希存储
        let key_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        // 插入数据库
        let api_key = sqlx::query_as::<_, ApiKeyRecord>(
            r#"
            INSERT INTO api_keys (id, user_id, key, name, prefix, status, created_at)
            VALUES ($1, $2, $3, $4, $5, 'active', NOW())
            RETURNING *
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(req.user_id)
        .bind(&key_hash)
        .bind(&req.name)
        .bind(&key_prefix)
        .fetch_one(&self.db)
        .await?;

        Ok(ApiKeyInfo {
            id: api_key.id,
            user_id: api_key.user_id,
            name: api_key.name,
            prefix: api_key.prefix,
            status: api_key.status,
            key: Some(key), // 只在创建时返回完整 key
            created_at: api_key.created_at,
        })
    }

    /// 批量更新账号
    pub async fn batch_update_accounts(
        &self,
        req: BatchUpdateRequest,
        stop_on_error: bool,
    ) -> Result<BatchResult<()>> {
        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        for (index, id) in req.ids.into_iter().enumerate() {
            match self.update_account(id, &req.updates).await {
                Ok(()) => {
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
                        error: Some(e.to_string()),
                    });
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 更新单个账号
    async fn update_account(
        &self,
        id: Uuid,
        updates: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // 构建动态更新 SQL
        let mut set_clauses = Vec::new();
        let mut values = Vec::new();
        let mut param_index = 1;

        for (key, value) in updates {
            match key.as_str() {
                "status" | "priority" | "group_id" | "concurrent_limit" | "rate_limit_rpm" => {
                    set_clauses.push(format!("{} = ${}", key, param_index));
                    values.push(value.clone());
                    param_index += 1;
                }
                _ => continue,
            }
        }

        if set_clauses.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE accounts SET {} WHERE id = ${}",
            set_clauses.join(", "),
            param_index
        );

        let mut query = sqlx::query(&sql);
        for value in values {
            query = query.bind(value);
        }
        query = query.bind(id);

        query.execute(&self.db).await?;

        Ok(())
    }

    /// CSV 导入用户
    pub async fn batch_import_users_csv(&self, csv_content: &str) -> Result<BatchResult<UserInfo>> {
        let mut reader = ReaderBuilder::new().from_reader(csv_content.as_bytes());
        let mut requests = Vec::new();

        for result in reader.deserialize() {
            let record: CreateUserCsvRecord = result?;
            requests.push(record.into());
        }

        self.batch_create_users(requests).await
    }

    /// 批量创建用户
    pub async fn batch_create_users(
        &self,
        requests: Vec<CreateUserRequest>,
    ) -> Result<BatchResult<UserInfo>> {
        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        for (index, req) in requests.into_iter().enumerate() {
            match self.create_user(req).await {
                Ok(user) => {
                    success += 1;
                    results.push(BatchItemResult {
                        index,
                        data: Some(user),
                        error: None,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchItemResult {
                        index,
                        data: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 创建单个用户
    async fn create_user(&self, req: CreateUserRequest) -> Result<UserInfo> {
        // 密码哈希
        let password_hash = {
            use argon2::{
                password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
                Argon2,
            };
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            argon2
                .hash_password(req.password.as_bytes(), &salt)
                .map_err(|e| anyhow::anyhow!("Password hashing failed: {}", e))?
                .to_string()
        };

        let user_id = Uuid::new_v4();
        let now = Utc::now();

        // 插入数据库
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, role, status, balance, totp_enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'active', 0, false, $5, $5)
            "#,
        )
        .bind(user_id)
        .bind(&req.email)
        .bind(&password_hash)
        .bind(&req.role)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(UserInfo {
            id: user_id,
            email: req.email,
            role: req.role,
            status: "active".to_string(),
            created_at: now,
        })
    }

    /// 批量删除 API Keys
    pub async fn batch_delete_api_keys(&self, ids: Vec<Uuid>) -> Result<BatchResult<()>> {
        let mut results = Vec::new();
        let mut success = 0;
        let mut failed = 0;

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_api_key(id).await {
                Ok(()) => {
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
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(BatchResult {
            total: results.len(),
            success,
            failed,
            results,
        })
    }

    /// 删除单个 API Key
    async fn delete_api_key(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow::anyhow!("API Key not found"));
        }

        Ok(())
    }
}

/// API Key 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub status: String,
    /// 只在创建时返回完整的 key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// API Key 数据库记录
#[derive(Debug, Clone, sqlx::FromRow)]
struct ApiKeyRecord {
    id: Uuid,
    user_id: Uuid,
    key: String,
    name: String,
    prefix: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_result_serialization() {
        let result = BatchResult {
            total: 10,
            success: 8,
            failed: 2,
            results: vec![
                BatchItemResult {
                    index: 0,
                    data: Some("test".to_string()),
                    error: None,
                },
                BatchItemResult {
                    index: 1,
                    data: None,
                    error: Some("error".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"success\":8"));
        assert!(json.contains("\"failed\":2"));
    }

    #[test]
    fn test_csv_record_deserialization() {
        let csv = "email,password,role\ntest@example.com,password123,user";
        let mut reader = ReaderBuilder::new().from_reader(csv.as_bytes());
        
        for result in reader.deserialize() {
            let record: CreateUserCsvRecord = result.unwrap();
            assert_eq!(record.email, "test@example.com");
            assert_eq!(record.password, "password123");
            assert_eq!(record.role, "user");
        }
    }
}
