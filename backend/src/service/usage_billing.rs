use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};

/// Billing service for usage tracking
pub struct UsageBilling {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BillingRecord {
    pub id: i64,
    pub user_id: i64,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BillingSummary {
    pub user_id: i64,
    pub total_cost: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl UsageBilling {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record billing
    pub async fn record(
        &self,
        user_id: i64,
        provider: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
    ) -> Result<BillingRecord, BillingError> {
        let record = query_as::<_, BillingRecord>(r#"
            INSERT INTO billing_records (user_id, provider, model, input_tokens, output_tokens, cost, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING *
            "#)
            .bind(user_id)
            .bind(&provider)
            .bind(&model)
            .bind(input_tokens as i64)
            .bind(output_tokens as i64)
            .bind(cost)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    /// Get billing summary
    pub async fn get_summary(
        &self,
        user_id: i64,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<BillingSummary, BillingError> {
        let summary = query_as::<_, BillingSummary>(
            r#"
            SELECT
                user_id,
                COALESCE(SUM(cost), 0) as total_cost,
                COALESCE(SUM(input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(output_tokens), 0) as total_output_tokens,
                $2 as period_start,
                $3 as period_end
            FROM billing_records
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            GROUP BY user_id
            "#,
        )
        .bind(user_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(BillingSummary {
            user_id,
            total_cost: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            period_start,
            period_end,
        });

        Ok(summary)
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
