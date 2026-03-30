use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Account service for managing AI service accounts
pub struct AccountService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub credentials: String,
    pub models: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 账号扩展信息（包含调度相关字段）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountWithScheduling {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub credentials: String,
    pub models: Vec<String>,
    pub priority: i32,
    pub concurrency: i32,
    pub load_factor: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 账号并发信息
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccountConcurrency {
    pub id: i64,
    pub max_concurrency: i32,
}

/// 账号负载信息（用于调度）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountLoadInfo {
    pub account_id: i64,
    pub load_rate: f64,
    pub current_concurrency: u32,
    pub max_concurrency: u32,
    pub waiting_count: u32,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountStatus {
    Active,
    Inactive,
    Suspended,
    Deleted,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("Account not found")]
    NotFound,
    #[error("Account inactive")]
    Inactive,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl AccountService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get account by ID
    pub async fn get(&self, id: i64) -> Result<Account, AccountError> {
        let account = query_as::<_, Account>("SELECT * FROM accounts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AccountError::NotFound)?;

        Ok(account)
    }

    /// Get account with scheduling info
    pub async fn get_with_scheduling(&self, id: i64) -> Result<AccountWithScheduling, AccountError> {
        let account = query_as::<_, AccountWithScheduling>(
            "SELECT id, name, provider, status, credentials, models, \
             COALESCE(priority, 0) as priority, \
             COALESCE(concurrency, 10) as concurrency, \
             COALESCE(load_factor, 1.0) as load_factor, \
             created_at, updated_at \
             FROM accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AccountError::NotFound)?;

        Ok(account)
    }

    /// List active accounts
    pub async fn list_active(&self) -> Result<Vec<Account>, AccountError> {
        let accounts = query_as::<_, Account>(
            "SELECT * FROM accounts WHERE status = 'active' ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(accounts)
    }

    /// List active accounts with scheduling info
    pub async fn list_active_with_scheduling(&self) -> Result<Vec<AccountWithScheduling>, AccountError> {
        let accounts = query_as::<_, AccountWithScheduling>(
            "SELECT id, name, provider, status, credentials, models, \
             COALESCE(priority, 0) as priority, \
             COALESCE(concurrency, 10) as concurrency, \
             COALESCE(load_factor, 1.0) as load_factor, \
             created_at, updated_at \
             FROM accounts WHERE status = 'active' \
             ORDER BY priority ASC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(accounts)
    }

    /// List schedulable accounts (active and with proper configuration)
    pub async fn list_schedulable(&self, group_id: Option<i64>) -> Result<Vec<AccountWithScheduling>, AccountError> {
        let accounts = if let Some(gid) = group_id {
            query_as::<_, AccountWithScheduling>(
                "SELECT a.id, a.name, a.provider, a.status, a.credentials, a.models, \
                 COALESCE(a.priority, 0) as priority, \
                 COALESCE(a.concurrency, 10) as concurrency, \
                 COALESCE(a.load_factor, 1.0) as load_factor, \
                 a.created_at, a.updated_at \
                 FROM accounts a \
                 JOIN account_groups ag ON a.id = ag.account_id \
                 WHERE a.status = 'active' AND ag.group_id = $1 \
                 ORDER BY a.priority ASC, a.created_at DESC",
            )
            .bind(gid)
            .fetch_all(&self.pool)
            .await?
        } else {
            self.list_active_with_scheduling().await?
        };

        Ok(accounts)
    }

    /// Get account concurrency limits
    pub async fn get_concurrency_batch(&self, account_ids: &[i64]) -> Result<Vec<AccountConcurrency>, AccountError> {
        let accounts = query_as::<_, AccountConcurrency>(
            "SELECT id, COALESCE(concurrency, 10) as max_concurrency \
             FROM accounts WHERE id = ANY($1)",
        )
        .bind(account_ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(accounts)
    }

    /// Create account
    pub async fn create(
        &self,
        name: String,
        provider: String,
        credentials: String,
        models: Vec<String>,
    ) -> Result<Account, AccountError> {
        let account = query_as::<_, Account>(r#"
            INSERT INTO accounts (name, provider, status, credentials, models, created_at, updated_at)
            VALUES ($1, $2, 'active', $3, $4, NOW(), NOW())
            RETURNING *
            "#)
            .bind(name)
            .bind(provider)
            .bind(credentials)
            .bind(&models)
            .fetch_one(&self.pool)
            .await?;

        Ok(account)
    }

    /// Create account with scheduling config
    pub async fn create_with_scheduling(
        &self,
        name: String,
        provider: String,
        credentials: String,
        models: Vec<String>,
        priority: i32,
        concurrency: u32,
        load_factor: f64,
    ) -> Result<AccountWithScheduling, AccountError> {
        let account = query_as::<_, AccountWithScheduling>(r#"
            INSERT INTO accounts (name, provider, status, credentials, models, priority, concurrency, load_factor, created_at, updated_at)
            VALUES ($1, $2, 'active', $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING id, name, provider, status, credentials, models, priority, concurrency, load_factor, created_at, updated_at
            "#)
            .bind(name)
            .bind(provider)
            .bind(credentials)
            .bind(&models)
            .bind(priority)
            .bind(concurrency as i32)
            .bind(load_factor)
            .fetch_one(&self.pool)
            .await?;

        Ok(account)
    }

    /// Update account status
    pub async fn update_status(&self, id: i64, status: AccountStatus) -> Result<(), AccountError> {
        let status_str = serde_json::to_string(&status).unwrap();

        query("UPDATE accounts SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status_str)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Update account scheduling config
    pub async fn update_scheduling_config(
        &self,
        id: i64,
        priority: i32,
        concurrency: u32,
        load_factor: f64,
    ) -> Result<(), AccountError> {
        query(
            "UPDATE accounts SET priority = $1, concurrency = $2, load_factor = $3, updated_at = NOW() WHERE id = $4",
        )
        .bind(priority)
        .bind(concurrency as i32)
        .bind(load_factor)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete account
    pub async fn delete(&self, id: i64) -> Result<(), AccountError> {
        self.update_status(id, AccountStatus::Deleted).await
    }

    /// Check if account supports a model
    pub async fn supports_model(&self, id: i64, model: &str) -> Result<bool, AccountError> {
        let account = self.get(id).await?;
        Ok(account.models.iter().any(|m| m == model || m == "*"))
    }

    /// Filter accounts by model support
    pub fn filter_by_model(accounts: &[AccountWithScheduling], model: &str) -> Vec<AccountWithScheduling> {
        accounts
            .iter()
            .filter(|a| a.models.iter().any(|m| m == model || m == "*"))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_account_service() {
        // Test would require database connection
    }
}
