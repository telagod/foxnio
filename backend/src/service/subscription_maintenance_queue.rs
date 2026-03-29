use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Maintenance queue for subscriptions
pub struct SubscriptionMaintenanceQueue {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MaintenanceTask {
    pub id: i64,
    pub subscription_id: i64,
    pub task_type: String,
    pub status: String,
    pub scheduled_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskType {
    RenewalCheck,
    PaymentRetry,
    GracePeriodStart,
    SuspensionCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Task not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl SubscriptionMaintenanceQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Enqueue task
    pub async fn enqueue(
        &self,
        subscription_id: i64,
        task_type: TaskType,
        scheduled_at: DateTime<Utc>,
    ) -> Result<MaintenanceTask, QueueError> {
        let task = query_as::<_, MaintenanceTask>(r#"
            INSERT INTO subscription_maintenance_tasks (subscription_id, task_type, status, scheduled_at)
            VALUES ($1, $2, 'pending', $3)
            RETURNING *
            "#)
            .bind(subscription_id)
            .bind(serde_json::to_string(&task_type).unwrap())
            .bind(scheduled_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(task)
    }

    /// Get pending tasks
    pub async fn get_pending(&self, limit: i64) -> Result<Vec<MaintenanceTask>, QueueError> {
        let tasks = query_as::<_, MaintenanceTask>(
            r#"
            SELECT * FROM subscription_maintenance_tasks
            WHERE status = 'pending' AND scheduled_at <= NOW()
            ORDER BY scheduled_at
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    /// Mark task as processing
    pub async fn mark_processing(&self, task_id: i64) -> Result<(), QueueError> {
        query("UPDATE subscription_maintenance_tasks SET status = 'processing' WHERE id = $1")
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Mark task as completed
    pub async fn mark_completed(&self, task_id: i64) -> Result<(), QueueError> {
        query("UPDATE subscription_maintenance_tasks SET status = 'completed', processed_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Mark task as failed
    pub async fn mark_failed(&self, task_id: i64, error: String) -> Result<(), QueueError> {
        query("UPDATE subscription_maintenance_tasks SET status = 'failed', processed_at = NOW(), error = $1 WHERE id = $2")
            .bind(&error)
            .bind(task_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_operations() {
        // Test would require database connection
    }
}
