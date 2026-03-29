use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Subscription expiry service
pub struct SubscriptionExpiryService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExpiryNotification {
    pub subscription_id: i64,
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExpiryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl SubscriptionExpiryService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get expiring subscriptions
    pub async fn get_expiring(
        &self,
        within_days: i32,
    ) -> Result<Vec<ExpiryNotification>, ExpiryError> {
        let cutoff = Utc::now() + chrono::Duration::days(within_days as i64);

        let notifications = query_as::<_, ExpiryNotification>(r#"
            SELECT s.id as subscription_id, s.user_id, s.current_period_end as expires_at, n.notified_at
            FROM subscriptions s
            LEFT JOIN subscription_expiry_notifications n ON s.id = n.subscription_id
            WHERE s.status = 'active'
            AND s.current_period_end <= $1
            AND s.current_period_end > NOW()
            AND n.notified_at IS NULL
            "#)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

        Ok(notifications)
    }

    /// Mark as notified
    pub async fn mark_notified(&self, subscription_id: i64) -> Result<(), ExpiryError> {
        query(
            r#"
            INSERT INTO subscription_expiry_notifications (subscription_id, notified_at)
            VALUES ($1, NOW())
            ON CONFLICT (subscription_id) DO UPDATE SET notified_at = NOW()
            "#,
        )
        .bind(subscription_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiry_service() {
        // Test would require database connection
    }
}
