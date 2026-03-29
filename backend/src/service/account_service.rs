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

    /// List active accounts
    pub async fn list_active(&self) -> Result<Vec<Account>, AccountError> {
        let accounts = query_as::<_, Account>(
            "SELECT * FROM accounts WHERE status = 'active' ORDER BY created_at DESC",
        )
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

    /// Delete account
    pub async fn delete(&self, id: i64) -> Result<(), AccountError> {
        self.update_status(id, AccountStatus::Deleted).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_service() {
        // Test would require database connection
    }
}
