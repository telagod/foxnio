use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Subscription service for Antigravity API
pub struct AntigravitySubscriptionService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Subscription {
    pub id: i64,
    pub user_id: i64,
    pub plan: String,
    pub status: String,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at_period_end: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubscriptionPlan {
    Free,
    Pro,
    Enterprise,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubscriptionStatus {
    Active,
    PastDue,
    Cancelled,
    Incomplete,
    Expired,
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("Subscription not found")]
    NotFound,
    #[error("Subscription inactive")]
    Inactive,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl AntigravitySubscriptionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get subscription for user
    pub async fn get_by_user(
        &self,
        user_id: i64,
    ) -> Result<Option<Subscription>, SubscriptionError> {
        let subscription = query_as::<_, Subscription>(
            r#"
            SELECT * FROM subscriptions
            WHERE user_id = $1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(subscription)
    }

    /// Create subscription
    pub async fn create(
        &self,
        user_id: i64,
        plan: SubscriptionPlan,
        period_months: i32,
    ) -> Result<Subscription, SubscriptionError> {
        let now = Utc::now();
        let period_end = now + chrono::Duration::days(period_months as i64 * 30);

        let subscription = query_as::<_, Subscription>(r#"
            INSERT INTO subscriptions (user_id, plan, status, current_period_start, current_period_end, created_at, updated_at)
            VALUES ($1, $2, 'active', $3, $4, $5, $6)
            RETURNING *
            "#)
            .bind(user_id)
            .bind(serde_json::to_string(&plan).unwrap())
            .bind(now)
            .bind(period_end)
            .bind(now)
            .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(subscription)
    }

    /// Cancel subscription
    pub async fn cancel(
        &self,
        subscription_id: i64,
        immediately: bool,
    ) -> Result<(), SubscriptionError> {
        if immediately {
            query(
                "UPDATE subscriptions SET status = 'cancelled', updated_at = NOW() WHERE id = $1",
            )
            .bind(subscription_id)
            .execute(&self.pool)
            .await?;
        } else {
            query("UPDATE subscriptions SET cancel_at_period_end = true, updated_at = NOW() WHERE id = $1")
                .bind(subscription_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Check if subscription is active
    pub async fn is_active(&self, user_id: i64) -> Result<bool, SubscriptionError> {
        let subscription = self.get_by_user(user_id).await?;
        Ok(subscription.is_some())
    }

    /// Get plan features
    pub fn get_plan_features(plan: &SubscriptionPlan) -> PlanFeatures {
        match plan {
            SubscriptionPlan::Free => PlanFeatures {
                max_requests_per_day: 100,
                max_tokens_per_month: 10_000,
                models: vec!["gpt-3.5-turbo".to_string()],
                priority_support: false,
            },
            SubscriptionPlan::Pro => PlanFeatures {
                max_requests_per_day: 10_000,
                max_tokens_per_month: 1_000_000,
                models: vec!["gpt-4".to_string(), "claude-2".to_string()],
                priority_support: true,
            },
            SubscriptionPlan::Enterprise => PlanFeatures {
                max_requests_per_day: 100_000,
                max_tokens_per_month: 100_000_000,
                models: vec!["*".to_string()],
                priority_support: true,
            },
            SubscriptionPlan::Custom(_) => PlanFeatures {
                max_requests_per_day: 1_000_000,
                max_tokens_per_month: 1_000_000_000,
                models: vec!["*".to_string()],
                priority_support: true,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFeatures {
    pub max_requests_per_day: u64,
    pub max_tokens_per_month: u64,
    pub models: Vec<String>,
    pub priority_support: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_features() {
        let features = AntigravitySubscriptionService::get_plan_features(&SubscriptionPlan::Pro);
        assert_eq!(features.max_requests_per_day, 10_000);
    }
}
