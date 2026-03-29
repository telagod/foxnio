use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Credits and overages management for Antigravity
pub struct AntigravityCreditsOverages {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CreditBalance {
    pub user_id: i64,
    pub credits_remaining: f64,
    pub credits_used: f64,
    pub overage_amount: f64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OverageRecord {
    pub id: i64,
    pub user_id: i64,
    pub amount: f64,
    pub rate: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreditsError {
    #[error("Insufficient credits")]
    InsufficientCredits,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl AntigravityCreditsOverages {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get credit balance for user
    pub async fn get_balance(&self, user_id: i64) -> Result<CreditBalance, CreditsError> {
        let balance = query_as::<_, CreditBalance>(
            r#"
            SELECT user_id, credits_remaining, credits_used, overage_amount, last_updated
            FROM credit_balances
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(CreditBalance {
            user_id,
            credits_remaining: 0.0,
            credits_used: 0.0,
            overage_amount: 0.0,
            last_updated: Utc::now(),
        });

        Ok(balance)
    }

    /// Consume credits
    pub async fn consume(&self, user_id: i64, amount: f64) -> Result<(), CreditsError> {
        query(
            r#"
            UPDATE credit_balances
            SET
                credits_remaining = GREATEST(credits_remaining - $1, 0),
                credits_used = credits_used + $1,
                overage_amount = overage_amount + GREATEST($1 - credits_remaining, 0),
                last_updated = NOW()
            WHERE user_id = $2
            "#,
        )
        .bind(amount)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Add credits
    pub async fn add(&self, user_id: i64, amount: f64) -> Result<(), CreditsError> {
        query(
            r#"
            INSERT INTO credit_balances (user_id, credits_remaining, last_updated)
            VALUES ($1, $2, NOW())
            ON CONFLICT (user_id) DO UPDATE SET
                credits_remaining = credit_balances.credits_remaining + $2,
                last_updated = NOW()
            "#,
        )
        .bind(user_id)
        .bind(amount)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credits_operations() {
        // Test would require database connection
    }
}
