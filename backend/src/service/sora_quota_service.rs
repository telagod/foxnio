use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Quota service for Sora API
pub struct SoraQuotaService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SoraQuota {
    pub user_id: i64,
    pub generations_used: i32,
    pub generations_limit: i32,
    pub seconds_generated: i32,
    pub seconds_limit: i32,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    #[error("Quota exceeded")]
    QuotaExceeded,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl SoraQuotaService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if user has quota
    pub async fn check(&self, user_id: i64) -> Result<SoraQuota, QuotaError> {
        let quota = query_as::<_, SoraQuota>(r#"
            SELECT user_id, generations_used, generations_limit, seconds_generated, seconds_limit, period_start, period_end
            FROM sora_quotas
            WHERE user_id = $1 AND period_end > NOW()
            "#)
            .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(SoraQuota {
            user_id,
            generations_used: 0,
            generations_limit: 100,
            seconds_generated: 0,
            seconds_limit: 300,
            period_start: Utc::now(),
            period_end: Utc::now() + chrono::Duration::days(30),
        });

        if quota.generations_used >= quota.generations_limit
            || quota.seconds_generated >= quota.seconds_limit
        {
            return Err(QuotaError::QuotaExceeded);
        }

        Ok(quota)
    }

    /// Consume quota
    pub async fn consume(&self, user_id: i64, duration_seconds: u32) -> Result<(), QuotaError> {
        query(
            r#"
            UPDATE sora_quotas
            SET generations_used = generations_used + 1, seconds_generated = seconds_generated + $1
            WHERE user_id = $2
            "#,
        )
        .bind(duration_seconds as i32)
        .bind(user_id)
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
