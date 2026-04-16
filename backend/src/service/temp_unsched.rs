//! 临时不可调度 - Temp Unschedulable
//!
//! 管理账号的临时不可调度状态

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 不可调度原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UnschedulableReason {
    RateLimited,
    QuotaExceeded,
    Maintenance,
    ErrorThresholdExceeded,
    Manual,
    AuthenticationFailed,
    ServiceDegradation,
}

/// 不可调度记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnschedulableRecord {
    pub id: i64,
    pub account_id: i64,
    pub reason: UnschedulableReason,
    pub message: Option<String>,
    pub scheduled_resume_at: Option<DateTime<Utc>>,
    pub actual_resume_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<i64>,
    pub resolved: bool,
}

/// 临时不可调度管理器
pub struct TempUnschedulable {
    db: sea_orm::DatabaseConnection,
    cache: Arc<RwLock<HashMap<i64, UnschedulableRecord>>>,
}

impl TempUnschedulable {
    /// 创建新的管理器
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 标记账号为不可调度
    pub async fn mark_unschedulable(
        &self,
        account_id: i64,
        reason: UnschedulableReason,
        message: Option<&str>,
        duration: Option<Duration>,
        created_by: Option<i64>,
    ) -> Result<UnschedulableRecord> {
        let scheduled_resume_at = duration.map(|d| Utc::now() + d);
        let reason_clone = reason.clone();

        let record = UnschedulableRecord {
            id: chrono::Utc::now().timestamp_millis(),
            account_id,
            reason,
            message: message.map(|s| s.to_string()),
            scheduled_resume_at,
            actual_resume_at: None,
            created_at: Utc::now(),
            created_by,
            resolved: false,
        };

        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(account_id, record.clone());
        }

        // 临时调度状态保留在内存，重启后恢复

        tracing::info!(
            "账号 {} 标记为不可调度，原因: {:?}",
            account_id,
            reason_clone
        );

        Ok(record)
    }

    /// 恢复账号调度
    pub async fn resume_scheduling(&self, account_id: i64) -> Result<bool> {
        // 更新缓存
        let removed = {
            let mut cache = self.cache.write().await;
            if let Some(mut record) = cache.remove(&account_id) {
                record.resolved = true;
                record.actual_resume_at = Some(Utc::now());

                // 临时调度状态保留在内存

                tracing::info!("账号 {} 已恢复调度", account_id);
                true
            } else {
                false
            }
        };

        Ok(removed)
    }

    /// 检查账号是否可调度
    pub async fn is_schedulable(&self, account_id: i64) -> Result<bool> {
        let cache = self.cache.read().await;

        if let Some(record) = cache.get(&account_id) {
            // 检查是否已到恢复时间
            if let Some(scheduled_resume_at) = record.scheduled_resume_at {
                if scheduled_resume_at <= Utc::now() {
                    // 自动恢复
                    drop(cache);
                    self.resume_scheduling(account_id).await?;
                    return Ok(true);
                }
            }

            return Ok(false);
        }

        Ok(true)
    }

    /// 获取不可调度记录
    pub async fn get_record(&self, account_id: i64) -> Result<Option<UnschedulableRecord>> {
        let cache = self.cache.read().await;
        Ok(cache.get(&account_id).cloned())
    }

    /// 获取所有不可调度的账号
    pub async fn get_all_unschedulable(&self) -> Result<Vec<UnschedulableRecord>> {
        let cache = self.cache.read().await;
        Ok(cache.values().cloned().collect())
    }

    /// 按原因获取不可调度账号
    pub async fn get_by_reason(
        &self,
        reason: &UnschedulableReason,
    ) -> Result<Vec<UnschedulableRecord>> {
        let cache = self.cache.read().await;
        Ok(cache
            .values()
            .filter(|r| &r.reason == reason)
            .cloned()
            .collect())
    }

    /// 清理已过期的不可调度记录
    pub async fn cleanup_expired(&self) -> Result<i64> {
        let mut count = 0i64;
        let mut to_remove = Vec::new();

        {
            let cache = self.cache.read().await;
            for (account_id, record) in cache.iter() {
                if let Some(scheduled_resume_at) = record.scheduled_resume_at {
                    if scheduled_resume_at <= Utc::now() {
                        to_remove.push(*account_id);
                    }
                }
            }
        }

        for account_id in to_remove {
            if self.resume_scheduling(account_id).await? {
                count += 1;
            }
        }

        Ok(count)
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> UnschedulableStats {
        let cache = self.cache.read().await;

        let mut stats = UnschedulableStats::default();

        for record in cache.values() {
            stats.total_count += 1;

            match &record.reason {
                UnschedulableReason::RateLimited => stats.rate_limited_count += 1,
                UnschedulableReason::QuotaExceeded => stats.quota_exceeded_count += 1,
                UnschedulableReason::Maintenance => stats.maintenance_count += 1,
                UnschedulableReason::ErrorThresholdExceeded => stats.error_threshold_count += 1,
                UnschedulableReason::Manual => stats.manual_count += 1,
                UnschedulableReason::AuthenticationFailed => stats.auth_failed_count += 1,
                UnschedulableReason::ServiceDegradation => stats.service_degradation_count += 1,
            }
        }

        stats
    }
}

/// 不可调度统计
#[derive(Debug, Clone, Default)]
pub struct UnschedulableStats {
    pub total_count: i64,
    pub rate_limited_count: i64,
    pub quota_exceeded_count: i64,
    pub maintenance_count: i64,
    pub error_threshold_count: i64,
    pub manual_count: i64,
    pub auth_failed_count: i64,
    pub service_degradation_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_temp_unschedulable() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let manager = TempUnschedulable::new(db);

        // 标记为不可调度
        let record = manager
            .mark_unschedulable(
                1,
                UnschedulableReason::RateLimited,
                Some("Rate limit exceeded"),
                Some(Duration::minutes(5)),
                None,
            )
            .await
            .unwrap();

        assert!(!record.resolved);

        // 检查是否可调度
        let schedulable = manager.is_schedulable(1).await.unwrap();
        assert!(!schedulable);

        // 恢复调度
        let resumed = manager.resume_scheduling(1).await.unwrap();
        assert!(resumed);

        // 再次检查
        let schedulable = manager.is_schedulable(1).await.unwrap();
        assert!(schedulable);
    }
}
