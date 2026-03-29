use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};

/// Usage service for tracking API usage
pub struct UsageService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageRecord {
    pub id: i64,
    pub user_id: i64,
    pub api_key_id: Option<i64>,
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageStats {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub unique_models: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl UsageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record usage
    pub async fn record(
        &self,
        user_id: i64,
        api_key_id: Option<i64>,
        provider: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<UsageRecord, UsageError> {
        let record = query_as::<_, UsageRecord>(r#"
            INSERT INTO usage_records (user_id, api_key_id, provider, model, request_count, input_tokens, output_tokens, created_at)
            VALUES ($1, $2, $3, $4, 1, $5, $6, NOW())
            RETURNING *
            "#)
            .bind(user_id)
            .bind(api_key_id)
            .bind(&provider)
            .bind(&model)
            .bind(input_tokens as i64)
            .bind(output_tokens as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    /// Get usage stats for user
    pub async fn get_stats(
        &self,
        user_id: i64,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<UsageStats, UsageError> {
        let stats = query_as::<_, UsageStats>(
            r#"
            SELECT
                COALESCE(SUM(request_count), 0) as total_requests,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                COUNT(DISTINCT model) as unique_models
            FROM usage_records
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            "#,
        )
        .bind(user_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_service() {
        // Test would require database connection
    }
}
