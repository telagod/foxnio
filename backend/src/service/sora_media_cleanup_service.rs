use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{query, PgPool};

/// Media cleanup service for Sora generated videos
pub struct SoraMediaCleanupService {
    pool: PgPool,
    config: CleanupConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    pub retention_days: u32,
    pub cleanup_interval_hours: u32,
    pub max_storage_gb: u32,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            cleanup_interval_hours: 24,
            max_storage_gb: 1000,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CleanupError {
    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl SoraMediaCleanupService {
    pub fn new(pool: PgPool, config: CleanupConfig) -> Self {
        Self { pool, config }
    }

    /// Cleanup expired media
    pub async fn cleanup_expired(&self) -> Result<u64, CleanupError> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);

        let result = query(
            r#"
            DELETE FROM sora_generations
            WHERE created_at < $1 AND status IN ('completed', 'failed')
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get storage stats
    pub async fn get_storage_stats(&self) -> Result<StorageStats, CleanupError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sora_generations WHERE status = 'completed'")
                .fetch_one(&self.pool)
                .await?;

        Ok(StorageStats {
            total_files: count as u64,
            total_size_gb: 0, // Would calculate from actual file sizes
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_files: u64,
    pub total_size_gb: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_config_default() {
        let config = CleanupConfig::default();
        assert_eq!(config.retention_days, 30);
    }
}
