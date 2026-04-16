//! 实时监控服务
//!
//! 提供 WebSocket 推送、实时告警、Dashboard 数据更新

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// 实时监控事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RealtimeEvent {
    /// QPS 更新
    QpsUpdate {
        current_qps: f64,
        avg_qps_1m: f64,
        avg_qps_5m: f64,
        timestamp: DateTime<Utc>,
    },
    /// 账号状态变更
    AccountStatusChange {
        account_id: Uuid,
        old_status: String,
        new_status: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// 错误告警
    ErrorAlert {
        account_id: Option<Uuid>,
        model: Option<String>,
        error_type: String,
        error_message: String,
        severity: AlertSeverity,
        timestamp: DateTime<Utc>,
    },
    /// 健康分数更新
    HealthScoreUpdate {
        account_id: Uuid,
        score: f64,
        factors: HealthFactorsData,
        timestamp: DateTime<Utc>,
    },
    /// 系统状态更新
    SystemStatus {
        cpu_usage: f64,
        memory_usage: f64,
        active_connections: u64,
        timestamp: DateTime<Utc>,
    },
    /// Dashboard 数据更新
    DashboardUpdate {
        overview: DashboardOverview,
        timestamp: DateTime<Utc>,
    },
}

/// 告警严重级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// 健康因素数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFactorsData {
    pub success_rate: f64,
    pub avg_latency_ms: u64,
    pub error_rate: f64,
    pub rate_limit_rate: f64,
}

/// Dashboard 概览数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardOverview {
    pub total_requests: u64,
    pub total_errors: u64,
    pub error_rate: f64,
    pub avg_latency_ms: u64,
    pub active_accounts: usize,
    pub active_models: usize,
    pub current_qps: f64,
    pub health_score: f64,
}

/// 实时监控配置
#[derive(Debug, Clone)]
pub struct RealtimeMonitorConfig {
    /// 广播通道容量
    pub broadcast_capacity: usize,
    /// 事件缓冲区大小
    pub event_buffer_size: usize,
    /// 推送间隔（毫秒）
    pub push_interval_ms: u64,
}

impl Default for RealtimeMonitorConfig {
    fn default() -> Self {
        Self {
            broadcast_capacity: 1000,
            event_buffer_size: 100,
            push_interval_ms: 1000,
        }
    }
}

/// 实时监控服务
pub struct RealtimeMonitor {
    config: RealtimeMonitorConfig,
    /// 事件广播通道
    event_sender: broadcast::Sender<RealtimeEvent>,
    /// 事件历史
    event_history: Arc<RwLock<Vec<RealtimeEvent>>>,
    /// 订阅者计数
    subscriber_count: Arc<RwLock<u64>>,
    /// 最后推送时间
    last_push: Arc<RwLock<DateTime<Utc>>>,
}

impl RealtimeMonitor {
    pub fn new(config: RealtimeMonitorConfig) -> Self {
        let (sender, _) = broadcast::channel(config.broadcast_capacity);

        Self {
            config,
            event_sender: sender,
            event_history: Arc::new(RwLock::new(Vec::new())),
            subscriber_count: Arc::new(RwLock::new(0)),
            last_push: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeEvent> {
        self.event_sender.subscribe()
    }

    /// 发布事件
    pub async fn publish(&self, event: RealtimeEvent) -> Result<()> {
        // 存入历史
        {
            let mut history = self.event_history.write().await;
            history.push(event.clone());

            // 限制历史大小
            if history.len() > self.config.event_buffer_size {
                history.remove(0);
            }
        }

        // 广播
        let _ = self.event_sender.send(event);

        Ok(())
    }

    /// 发布 QPS 更新
    pub async fn publish_qps_update(
        &self,
        current_qps: f64,
        avg_qps_1m: f64,
        avg_qps_5m: f64,
    ) -> Result<()> {
        self.publish(RealtimeEvent::QpsUpdate {
            current_qps,
            avg_qps_1m,
            avg_qps_5m,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 发布账号状态变更
    pub async fn publish_account_status_change(
        &self,
        account_id: Uuid,
        old_status: String,
        new_status: String,
        reason: String,
    ) -> Result<()> {
        self.publish(RealtimeEvent::AccountStatusChange {
            account_id,
            old_status,
            new_status,
            reason,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 发布错误告警
    pub async fn publish_error_alert(
        &self,
        account_id: Option<Uuid>,
        model: Option<String>,
        error_type: String,
        error_message: String,
        severity: AlertSeverity,
    ) -> Result<()> {
        self.publish(RealtimeEvent::ErrorAlert {
            account_id,
            model,
            error_type,
            error_message,
            severity,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 发布健康分数更新
    pub async fn publish_health_score_update(
        &self,
        account_id: Uuid,
        score: f64,
        factors: HealthFactorsData,
    ) -> Result<()> {
        self.publish(RealtimeEvent::HealthScoreUpdate {
            account_id,
            score,
            factors,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 发布系统状态
    pub async fn publish_system_status(
        &self,
        cpu_usage: f64,
        memory_usage: f64,
        active_connections: u64,
    ) -> Result<()> {
        self.publish(RealtimeEvent::SystemStatus {
            cpu_usage,
            memory_usage,
            active_connections,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 发布 Dashboard 更新
    pub async fn publish_dashboard_update(&self, overview: DashboardOverview) -> Result<()> {
        self.publish(RealtimeEvent::DashboardUpdate {
            overview,
            timestamp: Utc::now(),
        })
        .await
    }

    /// 获取事件历史
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<RealtimeEvent> {
        let history = self.event_history.read().await;
        let limit = limit.unwrap_or(50).min(history.len());
        history.iter().rev().take(limit).cloned().collect()
    }

    /// 获取订阅者数量
    pub async fn get_subscriber_count(&self) -> u64 {
        *self.subscriber_count.read().await
    }

    /// 增加订阅者计数
    pub async fn add_subscriber(&self) {
        let mut count = self.subscriber_count.write().await;
        *count += 1;
    }

    /// 减少订阅者计数
    pub async fn remove_subscriber(&self) {
        let mut count = self.subscriber_count.write().await;
        if *count > 0 {
            *count -= 1;
        }
    }

    /// 清理过期事件
    pub async fn cleanup(&self, max_age_seconds: i64) -> Result<()> {
        let cutoff = Utc::now().timestamp() - max_age_seconds;

        let mut history = self.event_history.write().await;
        history.retain(|event| {
            let timestamp = match event {
                RealtimeEvent::QpsUpdate { timestamp, .. } => timestamp.timestamp(),
                RealtimeEvent::AccountStatusChange { timestamp, .. } => timestamp.timestamp(),
                RealtimeEvent::ErrorAlert { timestamp, .. } => timestamp.timestamp(),
                RealtimeEvent::HealthScoreUpdate { timestamp, .. } => timestamp.timestamp(),
                RealtimeEvent::SystemStatus { timestamp, .. } => timestamp.timestamp(),
                RealtimeEvent::DashboardUpdate { timestamp, .. } => timestamp.timestamp(),
            };
            timestamp >= cutoff
        });

        Ok(())
    }
}

/// 实时监控聚合器
///
/// 聚合多个数据源，生成 Dashboard 所需的实时数据
pub struct RealtimeAggregator {
    monitor: Arc<RealtimeMonitor>,
    /// 最后的概览数据
    last_overview: Arc<RwLock<DashboardOverview>>,
}

impl RealtimeAggregator {
    pub fn new(monitor: Arc<RealtimeMonitor>) -> Self {
        Self {
            monitor,
            last_overview: Arc::new(RwLock::new(DashboardOverview {
                total_requests: 0,
                total_errors: 0,
                error_rate: 0.0,
                avg_latency_ms: 0,
                active_accounts: 0,
                active_models: 0,
                current_qps: 0.0,
                health_score: 100.0,
            })),
        }
    }

    /// 更新并推送概览数据
    pub async fn update_and_push(&self, overview: DashboardOverview) -> Result<()> {
        // 存储最新数据
        {
            let mut last = self.last_overview.write().await;
            *last = overview.clone();
        }

        // 推送更新
        self.monitor.publish_dashboard_update(overview).await
    }

    /// 获取当前概览
    pub async fn get_overview(&self) -> DashboardOverview {
        self.last_overview.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_realtime_monitor_basic() {
        let monitor = RealtimeMonitor::new(RealtimeMonitorConfig::default());

        // 订阅
        let mut receiver = monitor.subscribe();

        // 发布事件
        monitor.publish_qps_update(100.0, 95.0, 90.0).await.unwrap();

        // 接收事件
        let event = receiver.recv().await.unwrap();
        match event {
            RealtimeEvent::QpsUpdate {
                current_qps,
                avg_qps_1m,
                avg_qps_5m,
                ..
            } => {
                assert_eq!(current_qps, 100.0);
                assert_eq!(avg_qps_1m, 95.0);
                assert_eq!(avg_qps_5m, 90.0);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_error_alert() {
        let monitor = RealtimeMonitor::new(RealtimeMonitorConfig::default());
        let mut receiver = monitor.subscribe();

        monitor
            .publish_error_alert(
                Some(Uuid::nil()),
                Some("gpt-4".to_string()),
                "rate_limit".to_string(),
                "Rate limit exceeded".to_string(),
                AlertSeverity::Warning,
            )
            .await
            .unwrap();

        let event = receiver.recv().await.unwrap();
        match event {
            RealtimeEvent::ErrorAlert {
                error_type,
                severity,
                ..
            } => {
                assert_eq!(error_type, "rate_limit");
                assert_eq!(severity, AlertSeverity::Warning);
            }
            _ => panic!("Unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_event_history() {
        let monitor = RealtimeMonitor::new(RealtimeMonitorConfig::default());

        // 发布多个事件
        for i in 0..5 {
            monitor
                .publish_qps_update(i as f64, i as f64, i as f64)
                .await
                .unwrap();
        }

        let history = monitor.get_history(Some(3)).await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_aggregator() {
        let monitor = Arc::new(RealtimeMonitor::new(RealtimeMonitorConfig::default()));
        let aggregator = RealtimeAggregator::new(monitor.clone());

        let overview = DashboardOverview {
            total_requests: 1000,
            total_errors: 10,
            error_rate: 0.01,
            avg_latency_ms: 150,
            active_accounts: 5,
            active_models: 3,
            current_qps: 50.0,
            health_score: 95.0,
        };

        aggregator.update_and_push(overview.clone()).await.unwrap();

        let retrieved = aggregator.get_overview().await;
        assert_eq!(retrieved.total_requests, 1000);
        assert_eq!(retrieved.current_qps, 50.0);
    }
}
