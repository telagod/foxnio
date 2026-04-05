//! 运维清理服务 - Ops Cleanup Service
//!
//! 定期清理过期的运维数据和日志

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 清理任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupTask {
    pub id: i64,
    pub task_type: String,
    pub status: String, // "pending", "running", "completed", "failed"
    pub retention_days: i32,
    pub deleted_count: i64,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 清理统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupStats {
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub failed_tasks: i64,
    pub total_deleted: i64,
    pub last_cleanup_at: Option<DateTime<Utc>>,
}

/// 清理服务配置
#[derive(Debug, Clone)]
pub struct CleanupServiceConfig {
    pub enabled: bool,
    pub cleanup_interval_hours: u64,
    pub default_retention_days: i32,
    pub batch_size: u64,
    pub max_runtime_secs: u64,
}

impl Default for CleanupServiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cleanup_interval_hours: 6,
            default_retention_days: 30,
            batch_size: 1000,
            max_runtime_secs: 3600,
        }
    }
}

/// 运维清理服务
pub struct OpsCleanupService {
    db: sea_orm::DatabaseConnection,
    config: CleanupServiceConfig,
    stop_signal: Arc<RwLock<bool>>,
    stats: Arc<RwLock<CleanupStats>>,
}

impl OpsCleanupService {
    /// 创建新的清理服务
    pub fn new(db: sea_orm::DatabaseConnection, config: CleanupServiceConfig) -> Self {
        Self {
            db,
            config,
            stop_signal: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(CleanupStats {
                total_tasks: 0,
                completed_tasks: 0,
                failed_tasks: 0,
                total_deleted: 0,
                last_cleanup_at: None,
            })),
        }
    }

    /// 启动清理服务
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("运维清理服务已禁用");
            return Ok(());
        }

        tracing::info!("启动运维清理服务");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.cleanup_interval_hours * 3600,
        ));

        loop {
            if *self.stop_signal.read().await {
                break;
            }

            interval.tick().await;

            // 执行清理
            if let Err(e) = self.run_cleanup().await {
                tracing::error!("清理任务失败: {}", e);
            }
        }

        Ok(())
    }

    /// 停止清理服务
    pub async fn stop(&self) -> Result<()> {
        let mut stop = self.stop_signal.write().await;
        *stop = true;
        Ok(())
    }

    /// 执行清理
    pub async fn run_cleanup(&self) -> Result<CleanupStats> {
        tracing::info!("开始执行清理任务");

        let start_time = Utc::now();
        let default_cutoff = start_time - Duration::days(self.config.default_retention_days as i64);
        let usages_cutoff = start_time - Duration::days(90);

        let mut usages_deleted = 0i64;
        let mut audit_logs_deleted = 0i64;
        let mut tokens_deleted = 0i64;
        let mut other_deleted = 0i64;

        // 1. 清理 usages（90 天）
        let deleted = self.cleanup_usages(usages_cutoff).await?;
        usages_deleted += deleted;
        tracing::info!("清理了 {} 条 usages", deleted);

        // 2. 清理 audit_logs（180 天）
        let deleted = self.cleanup_audit_logs().await?;
        audit_logs_deleted += deleted;
        tracing::info!("清理了 {} 条 audit_logs", deleted);

        // 3. 清理过期/已撤销的 refresh_tokens
        let deleted = self.cleanup_expired_tokens().await?;
        tokens_deleted += deleted;
        tracing::info!("清理了 {} 条 refresh_tokens", deleted);

        // 4. 清理过期的 password_reset_tokens
        let deleted = self.cleanup_password_reset_tokens().await?;
        tokens_deleted += deleted;
        tracing::info!("清理了 {} 条 password_reset_tokens", deleted);

        // 5. 清理告警历史
        let deleted = self.cleanup_alert_history(default_cutoff).await?;
        other_deleted += deleted;
        tracing::info!("清理了 {} 条告警历史", deleted);

        let total_deleted = usages_deleted + audit_logs_deleted + tokens_deleted + other_deleted;

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.total_tasks += 1;
        stats.completed_tasks += 1;
        stats.total_deleted += total_deleted;
        stats.last_cleanup_at = Some(Utc::now());

        tracing::info!(
            "清理任务完成: usages={}, audit_logs={}, tokens={}, other={}, 耗时 {:?}",
            usages_deleted,
            audit_logs_deleted,
            tokens_deleted,
            other_deleted,
            Utc::now() - start_time
        );

        Ok(stats.clone())
    }

    /// 批量删除指定表中早于 cutoff 的记录（按 created_at）
    async fn batch_delete_by_cutoff(
        &self,
        table: &str,
        cutoff: DateTime<Utc>,
    ) -> Result<i64> {
        let batch_size = self.config.batch_size;
        let start = Utc::now();
        let max_runtime = Duration::seconds(self.config.max_runtime_secs as i64);
        let mut total_deleted = 0i64;

        loop {
            // 安全超时检查
            if Utc::now() - start > max_runtime {
                tracing::warn!(
                    "清理 {} 超时，已删除 {} 条",
                    table,
                    total_deleted
                );
                break;
            }

            // 子查询 + LIMIT 避免长时间行锁
            let sql = format!(
                "DELETE FROM {table} WHERE id IN (\
                   SELECT id FROM {table} WHERE created_at < $1 LIMIT $2\
                 )"
            );
            let result = self
                .db
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    &sql,
                    [cutoff.into(), (batch_size as i64).into()],
                ))
                .await?;

            let deleted = result.rows_affected() as i64;
            total_deleted += deleted;

            if deleted < batch_size as i64 {
                break;
            }
        }

        Ok(total_deleted)
    }

    /// 清理过期的 usages（默认 90 天）
    async fn cleanup_usages(&self, cutoff: DateTime<Utc>) -> Result<i64> {
        self.batch_delete_by_cutoff("usages", cutoff).await
    }

    /// 清理过期的 audit_logs（默认 180 天）
    async fn cleanup_audit_logs(&self) -> Result<i64> {
        let cutoff = Utc::now() - Duration::days(180);
        self.batch_delete_by_cutoff("audit_logs", cutoff).await
    }

    /// 清理过期/已撤销的 refresh_tokens
    async fn cleanup_expired_tokens(&self) -> Result<i64> {
        let now = Utc::now();
        let batch_size = self.config.batch_size;
        let start = Utc::now();
        let max_runtime = Duration::seconds(self.config.max_runtime_secs as i64);
        let mut total_deleted = 0i64;

        loop {
            if Utc::now() - start > max_runtime {
                tracing::warn!("清理 tokens 超时，已删除 {} 条", total_deleted);
                break;
            }

            let result = self
                .db
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM refresh_tokens WHERE id IN (\
                       SELECT id FROM refresh_tokens \
                       WHERE expires_at < $1 OR revoked = true \
                       LIMIT $2\
                     )",
                    [now.into(), (batch_size as i64).into()],
                ))
                .await?;

            let deleted = result.rows_affected() as i64;
            total_deleted += deleted;

            if deleted < batch_size as i64 {
                break;
            }
        }

        Ok(total_deleted)
    }

    /// 清理过期的 password_reset_tokens
    async fn cleanup_password_reset_tokens(&self) -> Result<i64> {
        let now = Utc::now();
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM password_reset_tokens WHERE expires_at < $1 OR used_at IS NOT NULL",
                [now.into()],
            ))
            .await?;
        Ok(result.rows_affected() as i64)
    }

    /// 清理错误日志（复用通用批量删除）
    async fn cleanup_error_logs(&self, cutoff: DateTime<Utc>) -> Result<i64> {
        self.batch_delete_by_cutoff("alert_history", cutoff).await
    }

    /// 清理请求日志（usages 的别名入口，cutoff 由调用方控制）
    async fn cleanup_request_logs(&self, cutoff: DateTime<Utc>) -> Result<i64> {
        self.cleanup_usages(cutoff).await
    }

    /// 清理聚合数据（暂无独立聚合表，预留）
    async fn cleanup_aggregation_data(&self, _cutoff: DateTime<Utc>) -> Result<i64> {
        Ok(0)
    }

    /// 清理指标数据（暂无独立指标表，预留）
    async fn cleanup_metrics_data(&self, _cutoff: DateTime<Utc>) -> Result<i64> {
        Ok(0)
    }

    /// 清理告警历史
    async fn cleanup_alert_history(&self, cutoff: DateTime<Utc>) -> Result<i64> {
        self.batch_delete_by_cutoff("alert_history", cutoff).await
    }

    /// 手动触发清理
    pub async fn trigger_cleanup(
        &self,
        retention_days: i32,
        data_types: Vec<String>,
    ) -> Result<CleanupTask> {
        let task_id = chrono::Utc::now().timestamp_millis();

        let task = CleanupTask {
            id: task_id,
            task_type: "manual".to_string(),
            status: "running".to_string(),
            retention_days,
            deleted_count: 0,
            error_message: None,
            started_at: Some(Utc::now()),
            completed_at: None,
            created_at: Utc::now(),
        };

        // 执行清理
        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        let mut total_deleted = 0i64;

        for data_type in data_types {
            let deleted = match data_type.as_str() {
                "error_logs" => self.cleanup_error_logs(cutoff).await?,
                "request_logs" | "usages" => self.cleanup_usages(cutoff).await?,
                "audit_logs" => {
                    let audit_cutoff = Utc::now() - Duration::days(retention_days as i64);
                    self.batch_delete_by_cutoff("audit_logs", audit_cutoff).await?
                }
                "tokens" | "refresh_tokens" => self.cleanup_expired_tokens().await?,
                "password_reset_tokens" => self.cleanup_password_reset_tokens().await?,
                "aggregation" => self.cleanup_aggregation_data(cutoff).await?,
                "metrics" => self.cleanup_metrics_data(cutoff).await?,
                "alerts" => self.cleanup_alert_history(cutoff).await?,
                _ => 0,
            };

            total_deleted += deleted;
        }

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.total_tasks += 1;
        stats.completed_tasks += 1;
        stats.total_deleted += total_deleted;

        Ok(CleanupTask {
            id: task_id,
            task_type: "manual".to_string(),
            status: "completed".to_string(),
            retention_days,
            deleted_count: total_deleted,
            error_message: None,
            started_at: task.started_at,
            completed_at: Some(Utc::now()),
            created_at: task.created_at,
        })
    }

    /// 获取清理统计
    pub async fn get_stats(&self) -> CleanupStats {
        self.stats.read().await.clone()
    }

    /// 获取预估存储大小（各表行数）
    pub async fn estimate_storage_size(&self) -> Result<HashMap<String, i64>> {
        let mut sizes = HashMap::new();

        let tables = [
            ("usages", "usages"),
            ("audit_logs", "audit_logs"),
            ("refresh_tokens", "refresh_tokens"),
            ("password_reset_tokens", "password_reset_tokens"),
            ("alert_history", "alert_history"),
        ];

        for (key, table) in tables {
            let sql = format!(
                "SELECT COUNT(*) AS cnt FROM {table}"
            );
            let row = self
                .db
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    &sql,
                    [],
                ))
                .await?;

            let count: i64 = row
                .as_ref()
                .and_then(|r| r.try_get_by_index(0).ok())
                .unwrap_or(0);
            sizes.insert(key.to_string(), count);
        }

        Ok(sizes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_cleanup_service() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = CleanupServiceConfig::default();
        let service = OpsCleanupService::new(db, config);

        let stats = service.get_stats().await;
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.total_deleted, 0);
    }
}
