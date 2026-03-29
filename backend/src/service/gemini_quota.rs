use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Quota management for Gemini API
pub struct GeminiQuota {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QuotaInfo {
    pub account_id: i64,
    pub requests_used: i64,
    pub requests_limit: i64,
    pub tokens_used: i64,
    pub tokens_limit: i64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub is_over_quota: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    #[error("Quota exceeded")]
    QuotaExceeded,
    #[error("Account not found")]
    AccountNotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl GeminiQuota {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if account has quota available
    pub async fn check_quota(&self, account_id: i64) -> Result<QuotaInfo, QuotaError> {
        let info = query_as::<_, QuotaInfo>(
            r#"
            SELECT 
                account_id,
                requests_used,
                requests_limit,
                tokens_used,
                tokens_limit,
                period_start,
                period_end,
                (requests_used >= requests_limit OR tokens_used >= tokens_limit) as is_over_quota
            FROM gemini_quotas
            WHERE account_id = $1 AND period_end > NOW()
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(QuotaError::AccountNotFound)?;

        if info.is_over_quota {
            return Err(QuotaError::QuotaExceeded);
        }

        Ok(info)
    }

    /// Consume quota
    pub async fn consume(&self, account_id: i64, tokens: u64) -> Result<(), QuotaError> {
        query(
            r#"
            UPDATE gemini_quotas
            SET 
                requests_used = requests_used + 1,
                tokens_used = tokens_used + $1
            WHERE account_id = $2
            "#,
        )
        .bind(tokens as i64)
        .bind(account_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Reset quota for new period
    pub async fn reset(&self, account_id: i64) -> Result<(), QuotaError> {
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(1);

        query(r#"
            INSERT INTO gemini_quotas (account_id, requests_used, tokens_used, period_start, period_end)
            VALUES ($1, 0, 0, $2, $3)
            ON CONFLICT (account_id) DO UPDATE SET
                requests_used = 0,
                tokens_used = 0,
                period_start = $2,
                period_end = $3
            "#)
            .bind(account_id)
            .bind(now)
            .bind(period_end)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_check() {
        // Test would require database connection
    }
}
