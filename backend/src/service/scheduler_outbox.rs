//! 调度器发件箱服务
//!
//! 管理待发送的调度器事件，确保事件不丢失

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::scheduler_events::SchedulerEvent;

/// 发件箱状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Sent,
    Failed,
}

/// 发件箱条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub id: i64,
    pub event: SchedulerEvent,
    pub status: OutboxStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

/// 发件箱配置
#[derive(Debug, Clone)]
pub struct OutboxConfig {
    pub max_entries: usize,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub batch_size: usize,
}

impl Default for OutboxConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            max_retries: 3,
            retry_delay_ms: 1000,
            batch_size: 100,
        }
    }
}

/// 发件箱统计
#[derive(Debug, Clone, Default)]
pub struct OutboxStats {
    pub total_events: u64,
    pub pending_events: u64,
    pub sent_events: u64,
    pub failed_events: u64,
}

/// 调度器发件箱服务
pub struct SchedulerOutboxService {
    entries: Arc<RwLock<VecDeque<OutboxEntry>>>,
    config: OutboxConfig,
    stats: Arc<RwLock<OutboxStats>>,
    next_id: Arc<RwLock<i64>>,
}

impl SchedulerOutboxService {
    /// 创建新的发件箱服务
    pub fn new(config: OutboxConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::new())),
            config,
            stats: Arc::new(RwLock::new(OutboxStats::default())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// 添加事件到发件箱
    pub async fn enqueue(&self, event: SchedulerEvent) -> Result<i64> {
        let mut entries = self.entries.write().await;
        
        // 检查容量
        if entries.len() >= self.config.max_entries {
            // 移除最旧的已完成条目
            while entries.len() >= self.config.max_entries {
                if let Some(front) = entries.front() {
                    if front.status == OutboxStatus::Sent || front.status == OutboxStatus::Failed {
                        entries.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }

        let id = {
            let mut next_id = self.next_id.write().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let entry = OutboxEntry {
            id,
            event,
            status: OutboxStatus::Pending,
            retry_count: 0,
            max_retries: self.config.max_retries,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_error: None,
        };

        entries.push_back(entry);

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.total_events += 1;
        stats.pending_events += 1;

        Ok(id)
    }

    /// 获取待处理事件
    pub async fn get_pending(&self, limit: usize) -> Vec<OutboxEntry> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| e.status == OutboxStatus::Pending)
            .take(limit)
            .cloned()
            .collect()
    }

    /// 标记事件为处理中
    pub async fn mark_processing(&self, id: i64) -> Result<()> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            // Don't change status if already failed or sent
            if entry.status == OutboxStatus::Failed || entry.status == OutboxStatus::Sent {
                return Ok(());
            }
            entry.status = OutboxStatus::Processing;
            entry.updated_at = Utc::now();
        }
        Ok(())
    }

    /// 标记事件为已发送
    pub async fn mark_sent(&self, id: i64) -> Result<()> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            entry.status = OutboxStatus::Sent;
            entry.updated_at = Utc::now();
            
            let mut stats = self.stats.write().await;
            stats.pending_events = stats.pending_events.saturating_sub(1);
            stats.sent_events += 1;
        }
        Ok(())
    }

    /// 标记事件为失败
    pub async fn mark_failed(&self, id: i64, error: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
            // If already failed, don't change anything
            if entry.status == OutboxStatus::Failed {
                return Ok(());
            }
            
            entry.retry_count += 1;
            entry.last_error = Some(error.to_string());
            entry.updated_at = Utc::now();

            if entry.retry_count >= entry.max_retries {
                entry.status = OutboxStatus::Failed;
                let mut stats = self.stats.write().await;
                stats.pending_events = stats.pending_events.saturating_sub(1);
                stats.failed_events += 1;
            } else {
                entry.status = OutboxStatus::Pending;
            }
        }
        Ok(())
    }

    /// 重试失败事件
    pub async fn retry_failed(&self) -> Result<usize> {
        let mut entries = self.entries.write().await;
        let mut retry_count = 0;

        for entry in entries.iter_mut() {
            if entry.status == OutboxStatus::Failed && entry.retry_count < entry.max_retries {
                entry.status = OutboxStatus::Pending;
                entry.updated_at = Utc::now();
                retry_count += 1;

                let mut stats = self.stats.write().await;
                stats.pending_events += 1;
                stats.failed_events = stats.failed_events.saturating_sub(1);
            }
        }

        Ok(retry_count)
    }

    /// 清理已发送和失败的事件
    pub async fn cleanup(&self) -> usize {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        
        entries.retain(|e| e.status != OutboxStatus::Sent && e.status != OutboxStatus::Failed);
        
        before - entries.len()
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> OutboxStats {
        self.stats.read().await.clone()
    }

    /// 获取队列大小
    pub async fn size(&self) -> usize {
        self.entries.read().await.len()
    }
}

impl Default for SchedulerOutboxService {
    fn default() -> Self {
        Self::new(OutboxConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue() {
        let service = SchedulerOutboxService::default();
        let event = SchedulerEvent::account_changed(123);
        
        let id = service.enqueue(event).await.unwrap();
        assert_eq!(id, 1);
        
        let stats = service.get_stats().await;
        assert_eq!(stats.total_events, 1);
        assert_eq!(stats.pending_events, 1);
    }

    #[tokio::test]
    async fn test_get_pending() {
        let service = SchedulerOutboxService::default();
        service.enqueue(SchedulerEvent::account_changed(1)).await.unwrap();
        service.enqueue(SchedulerEvent::account_changed(2)).await.unwrap();
        
        let pending = service.get_pending(10).await;
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn test_mark_sent() {
        let service = SchedulerOutboxService::default();
        let id = service.enqueue(SchedulerEvent::account_changed(123)).await.unwrap();
        
        service.mark_processing(id).await.unwrap();
        service.mark_sent(id).await.unwrap();
        
        let stats = service.get_stats().await;
        assert_eq!(stats.sent_events, 1);
        assert_eq!(stats.pending_events, 0);
    }

    #[tokio::test]
    async fn test_mark_failed() {
        let service = SchedulerOutboxService::default();
        let id = service.enqueue(SchedulerEvent::account_changed(123)).await.unwrap();
        
        service.mark_processing(id).await.unwrap();
        service.mark_failed(id, "test error").await.unwrap();
        
        // 第一次失败，应该还可以重试
        let pending = service.get_pending(10).await;
        assert!(!pending.is_empty());
    }

    #[tokio::test]
    async fn test_max_retries() {
        let config = OutboxConfig { max_retries: 2, ..Default::default() };
        let service = SchedulerOutboxService::new(config);
        let id = service.enqueue(SchedulerEvent::account_changed(123)).await.unwrap();
        
        // 失败 3 次
        for _ in 0..3 {
            service.mark_processing(id).await.unwrap();
            service.mark_failed(id, "test error").await.unwrap();
        }
        
        let stats = service.get_stats().await;
        assert_eq!(stats.failed_events, 1);
    }
}
