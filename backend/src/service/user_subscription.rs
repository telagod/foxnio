use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query_as, FromRow, PgPool};

/// User subscription management
pub struct UserSubscription {
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
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("Subscription not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl UserSubscription {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get active subscription for user
    pub async fn get_active(
        &self,
        user_id: i64,
    ) -> Result<Option<Subscription>, SubscriptionError> {
        let subscription = query_as::<_, Subscription>(
            r#"
            SELECT * FROM subscriptions
            WHERE user_id = $1 AND status = 'active' AND current_period_end > NOW()
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(subscription)
    }

    /// Check if user has active subscription
    pub async fn is_active(&self, user_id: i64) -> Result<bool, SubscriptionError> {
        let sub = self.get_active(user_id).await?;
        Ok(sub.is_some())
    }

    /// Get subscription plan
    pub async fn get_plan(&self, user_id: i64) -> Result<String, SubscriptionError> {
        let sub = self
            .get_active(user_id)
            .await?
            .ok_or(SubscriptionError::NotFound)?;
        Ok(sub.plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_subscription() {
        // Test would require database connection
    }
}
