use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool, Row};

/// Billing service
pub struct BillingService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BillingAccount {
    pub id: i64,
    pub user_id: i64,
    pub balance: f64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub id: i64,
    pub account_id: i64,
    pub amount: f64,
    pub transaction_type: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Credit,
    Debit,
    Refund,
}

#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Account not found")]
    AccountNotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl BillingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get account balance
    pub async fn get_balance(&self, user_id: i64) -> Result<f64, BillingError> {
        let account = query("SELECT balance FROM billing_accounts WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(BillingError::AccountNotFound)?;

        let balance: f64 = account.try_get("balance")?;
        Ok(balance)
    }

    /// Add credit
    pub async fn add_credit(
        &self,
        user_id: i64,
        amount: f64,
        description: String,
    ) -> Result<Transaction, BillingError> {
        // Update balance
        query(
            r#"
            INSERT INTO billing_accounts (user_id, balance, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (user_id) DO UPDATE SET
                balance = billing_accounts.balance + $2,
                updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(amount)
        .execute(&self.pool)
        .await?;

        // Record transaction
        let transaction = query_as::<_, Transaction>(r#"
            INSERT INTO billing_transactions (account_id, amount, transaction_type, description, created_at)
            SELECT id, $2, 'credit', $3, NOW() FROM billing_accounts WHERE user_id = $1
            RETURNING *
            "#)
            .bind(user_id)
            .bind(amount)
            .bind(&description)
        .fetch_one(&self.pool)
        .await?;

        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_billing_service() {
        // Test would require database connection
    }
}
