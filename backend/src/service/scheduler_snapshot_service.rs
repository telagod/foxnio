//! 调度器快照服务
//!
//! 定期保存调度器状态快照，用于快速恢复

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 调度器快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerSnapshot {
    pub id: i64,
    pub version: u64,
    pub timestamp: DateTime<Utc>,
    pub accounts: Vec<AccountSnapshot>,
    pub groups: Vec<GroupSnapshot>,
    pub stats: SnapshotStats,
}

/// 账号快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub priority: i32,
    pub concurrent_count: u32,
    pub last_used: Option<DateTime<Utc>>,
    pub model_mapping: serde_json::Value,
}

/// 分组快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSnapshot {
    pub id: i64,
    pub name: String,
    pub account_ids: Vec<i64>,
    pub capacity: u32,
    pub used: u32,
}

/// 快照统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotStats {
    pub total_accounts: usize,
    pub active_accounts: usize,
    pub total_groups: usize,
    pub total_concurrent: u32,
}

/// 快照配置
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub snapshot_interval_seconds: u64,
    pub max_snapshots: usize,
    pub compression_enabled: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_seconds: 60,
            max_snapshots: 10,
            compression_enabled: true,
        }
    }
}

/// 快照服务统计
#[derive(Debug, Clone, Default)]
pub struct SnapshotServiceStats {
    pub total_snapshots: u64,
    pub last_snapshot_time: Option<DateTime<Utc>>,
    pub snapshot_size_bytes: u64,
    pub snapshot_duration_ms: u64,
}

/// 调度器快照服务
pub struct SchedulerSnapshotService {
    config: SnapshotConfig,
    snapshots: Arc<RwLock<Vec<SchedulerSnapshot>>>,
    stats: Arc<RwLock<SnapshotServiceStats>>,
    next_id: Arc<RwLock<i64>>,
}

impl SchedulerSnapshotService {
    /// 创建新的快照服务
    pub fn new(config: SnapshotConfig) -> Self {
        Self {
            config,
            snapshots: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(SnapshotServiceStats::default())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// 创建快照
    pub async fn create_snapshot(
        &self,
        accounts: Vec<AccountSnapshot>,
        groups: Vec<GroupSnapshot>,
    ) -> Result<SchedulerSnapshot> {
        let start = std::time::Instant::now();

        let id = {
            let mut next_id = self.next_id.write().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let version = self.get_next_version().await;

        let stats = SnapshotStats {
            total_accounts: accounts.len(),
            active_accounts: accounts.iter().filter(|a| a.status == "active").count(),
            total_groups: groups.len(),
            total_concurrent: accounts.iter().map(|a| a.concurrent_count).sum(),
        };

        let snapshot = SchedulerSnapshot {
            id,
            version,
            timestamp: Utc::now(),
            accounts,
            groups,
            stats,
        };

        // 保存快照
        self.save_snapshot(snapshot.clone()).await?;

        // 更新统计
        let duration_ms = start.elapsed().as_millis() as u64;
        let mut stats = self.stats.write().await;
        stats.total_snapshots += 1;
        stats.last_snapshot_time = Some(Utc::now());
        stats.snapshot_duration_ms = duration_ms;

        Ok(snapshot)
    }

    /// 保存快照
    async fn save_snapshot(&self, snapshot: SchedulerSnapshot) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;

        // 计算快照大小
        let size = serde_json::to_vec(&snapshot)?.len() as u64;
        let mut stats = self.stats.write().await;
        stats.snapshot_size_bytes = size;

        snapshots.push(snapshot);

        // 限制快照数量
        while snapshots.len() > self.config.max_snapshots {
            snapshots.remove(0);
        }

        Ok(())
    }

    /// 获取最新快照
    pub async fn get_latest(&self) -> Option<SchedulerSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.last().cloned()
    }

    /// 获取指定版本的快照
    pub async fn get_by_version(&self, version: u64) -> Option<SchedulerSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.iter().find(|s| s.version == version).cloned()
    }

    /// 获取所有快照
    pub async fn get_all(&self) -> Vec<SchedulerSnapshot> {
        self.snapshots.read().await.clone()
    }

    /// 删除旧快照
    pub async fn cleanup_old_snapshots(&self, keep_count: usize) -> usize {
        let mut snapshots = self.snapshots.write().await;
        let before = snapshots.len();

        while snapshots.len() > keep_count {
            snapshots.remove(0);
        }

        before - snapshots.len()
    }

    /// 获取下一个版本号
    async fn get_next_version(&self) -> u64 {
        let snapshots = self.snapshots.read().await;
        snapshots.last().map(|s| s.version + 1).unwrap_or(1)
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> SnapshotServiceStats {
        self.stats.read().await.clone()
    }

    /// 恢复快照
    pub async fn restore(&self, _version: Option<u64>) -> Result<()> {
        // TODO: 实现从快照恢复调度器状态
        // 1. 获取快照
        // 2. 重建账号状态
        // 3. 重建分组状态
        // 4. 更新调度器

        Ok(())
    }

    /// 启动后台快照任务
    pub fn start_background_snapshot(
        self: Arc<Self>,
        scheduler: Arc<RwLock<crate::service::SchedulerService>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                self.config.snapshot_interval_seconds,
            ));

            loop {
                interval.tick().await;

                // 获取调度器状态并创建快照
                let sched = scheduler.read().await;
                // TODO: 从调度器获取账号和分组信息
                // let accounts = sched.get_account_snapshots().await;
                // let groups = sched.get_group_snapshots().await;

                // if let Err(e) = self.create_snapshot(accounts, groups).await {
                //     tracing::error!("Failed to create snapshot: {}", e);
                // }
                let _ = sched; // 避免 unused 警告
            }
        })
    }
}

impl Default for SchedulerSnapshotService {
    fn default() -> Self {
        Self::new(SnapshotConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_snapshot() {
        let service = SchedulerSnapshotService::default();

        let accounts = vec![AccountSnapshot {
            id: 1,
            name: "test".to_string(),
            provider: "openai".to_string(),
            status: "active".to_string(),
            priority: 100,
            concurrent_count: 2,
            last_used: None,
            model_mapping: serde_json::json!({}),
        }];

        let groups = vec![];

        let snapshot = service.create_snapshot(accounts, groups).await.unwrap();
        assert_eq!(snapshot.version, 1);
        assert_eq!(snapshot.stats.total_accounts, 1);
    }

    #[tokio::test]
    async fn test_get_latest() {
        let service = SchedulerSnapshotService::default();

        let accounts = vec![AccountSnapshot {
            id: 1,
            name: "test".to_string(),
            provider: "openai".to_string(),
            status: "active".to_string(),
            priority: 100,
            concurrent_count: 2,
            last_used: None,
            model_mapping: serde_json::json!({}),
        }];

        service
            .create_snapshot(accounts.clone(), vec![])
            .await
            .unwrap();
        service.create_snapshot(accounts, vec![]).await.unwrap();

        let latest = service.get_latest().await;
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, 2);
    }

    #[tokio::test]
    async fn test_max_snapshots() {
        let config = SnapshotConfig {
            max_snapshots: 2,
            ..Default::default()
        };
        let service = SchedulerSnapshotService::new(config);

        let accounts = vec![AccountSnapshot {
            id: 1,
            name: "test".to_string(),
            provider: "openai".to_string(),
            status: "active".to_string(),
            priority: 100,
            concurrent_count: 2,
            last_used: None,
            model_mapping: serde_json::json!({}),
        }];

        for _ in 0..5 {
            service
                .create_snapshot(accounts.clone(), vec![])
                .await
                .unwrap();
        }

        let snapshots = service.get_all().await;
        assert_eq!(snapshots.len(), 2);
    }
}
