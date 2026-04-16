//! 调度器指标收集模块
//!
//! 提供实时指标收集、聚合和查询功能
//!
//! 预留功能：调度器指标（扩展功能）

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 单个账号的实时指标
#[derive(Debug)]
pub struct AccountMetrics {
    /// 活跃连接数
    pub active_connections: AtomicU32,
    /// 总请求数
    pub total_requests: AtomicU64,
    /// 成功请求数
    pub success_requests: AtomicU64,
    /// 失败请求数
    pub failed_requests: AtomicU64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: AtomicU64,
    /// 最后使用时间戳
    pub last_used: AtomicI64,
    /// 总成本（分）
    pub total_cost_cents: AtomicU64,
    /// 最近 N 次请求延迟
    recent_latencies: RwLock<Vec<u64>>,
    /// 请求速率（每分钟）
    pub requests_per_minute: AtomicU64,
    /// 错误率（每千次）
    pub error_rate_per_mille: AtomicU32,
}

impl AccountMetrics {
    pub fn new() -> Self {
        Self {
            active_connections: AtomicU32::new(0),
            total_requests: AtomicU64::new(0),
            success_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            avg_latency_ms: AtomicU64::new(0),
            last_used: AtomicI64::new(0),
            total_cost_cents: AtomicU64::new(0),
            recent_latencies: RwLock::new(Vec::with_capacity(100)),
            requests_per_minute: AtomicU64::new(0),
            error_rate_per_mille: AtomicU32::new(0),
        }
    }

    /// 记录请求开始
    pub fn record_request_start(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
        self.total_requests.fetch_add(1, Ordering::SeqCst);
        self.last_used
            .store(Utc::now().timestamp(), Ordering::SeqCst);
    }

    /// 记录请求成功
    pub async fn record_request_success(&self, latency_ms: u64, cost_cents: Option<u64>) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
        self.success_requests.fetch_add(1, Ordering::SeqCst);

        // 更新平均延迟
        self.update_avg_latency(latency_ms).await;

        // 更新成本
        if let Some(cost) = cost_cents {
            self.total_cost_cents.fetch_add(cost, Ordering::SeqCst);
        }

        // 更新错误率
        self.update_error_rate().await;
    }

    /// 记录请求失败
    pub async fn record_request_failure(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
        self.failed_requests.fetch_add(1, Ordering::SeqCst);

        // 更新错误率
        self.update_error_rate().await;
    }

    /// 更新平均延迟（指数移动平均）
    async fn update_avg_latency(&self, latency_ms: u64) {
        // 添加到最近延迟列表
        {
            let mut latencies = self.recent_latencies.write().await;
            latencies.push(latency_ms);
            if latencies.len() > 100 {
                latencies.remove(0);
            }
        }

        // 计算新的平均值
        let latencies = self.recent_latencies.read().await;
        if !latencies.is_empty() {
            let sum: u64 = latencies.iter().sum();
            let avg = sum / latencies.len() as u64;
            self.avg_latency_ms.store(avg, Ordering::SeqCst);
        }
    }

    /// 更新错误率
    async fn update_error_rate(&self) {
        let total = self.total_requests.load(Ordering::SeqCst);
        let failed = self.failed_requests.load(Ordering::SeqCst);

        if total > 0 {
            let rate = (failed * 1000 / total) as u32;
            self.error_rate_per_mille.store(rate, Ordering::SeqCst);
        }
    }

    /// 获取当前活跃连接数
    pub fn get_active_connections(&self) -> u32 {
        self.active_connections.load(Ordering::SeqCst)
    }

    /// 获取平均延迟
    pub fn get_avg_latency_ms(&self) -> u64 {
        self.avg_latency_ms.load(Ordering::SeqCst)
    }

    /// 获取成功率
    pub fn get_success_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::SeqCst);
        let success = self.success_requests.load(Ordering::SeqCst);

        if total == 0 {
            1.0
        } else {
            success as f64 / total as f64
        }
    }

    /// 获取错误率
    pub fn get_error_rate(&self) -> f64 {
        self.error_rate_per_mille.load(Ordering::SeqCst) as f64 / 1000.0
    }

    /// 获取总成本
    pub fn get_total_cost_cents(&self) -> u64 {
        self.total_cost_cents.load(Ordering::SeqCst)
    }

    /// 获取最后使用时间
    pub fn get_last_used(&self) -> Option<DateTime<Utc>> {
        let ts = self.last_used.load(Ordering::SeqCst);
        if ts > 0 {
            DateTime::from_timestamp(ts, 0)
        } else {
            None
        }
    }

    /// 获取快照
    pub async fn snapshot(&self) -> AccountMetricsSnapshot {
        AccountMetricsSnapshot {
            active_connections: self.get_active_connections(),
            total_requests: self.total_requests.load(Ordering::SeqCst),
            success_requests: self.success_requests.load(Ordering::SeqCst),
            failed_requests: self.failed_requests.load(Ordering::SeqCst),
            avg_latency_ms: self.get_avg_latency_ms(),
            last_used: self.get_last_used(),
            total_cost_cents: self.get_total_cost_cents(),
            error_rate: self.get_error_rate(),
        }
    }

    /// 重置指标
    pub fn reset(&self) {
        self.active_connections.store(0, Ordering::SeqCst);
        self.total_requests.store(0, Ordering::SeqCst);
        self.success_requests.store(0, Ordering::SeqCst);
        self.failed_requests.store(0, Ordering::SeqCst);
        self.avg_latency_ms.store(0, Ordering::SeqCst);
        self.last_used.store(0, Ordering::SeqCst);
        self.total_cost_cents.store(0, Ordering::SeqCst);
        self.error_rate_per_mille.store(0, Ordering::SeqCst);
    }
}

impl Default for AccountMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 账号指标快照（用于序列化和查询）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountMetricsSnapshot {
    pub active_connections: u32,
    pub total_requests: u64,
    pub success_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: u64,
    pub last_used: Option<DateTime<Utc>>,
    pub total_cost_cents: u64,
    pub error_rate: f64,
}

/// 调度器全局指标
#[derive(Debug)]
pub struct SchedulerMetrics {
    /// 各账号指标
    pub account_metrics: Arc<RwLock<HashMap<Uuid, Arc<AccountMetrics>>>>,
    /// 总请求数
    pub request_count: AtomicU64,
    /// 总延迟（毫秒）
    pub total_latency_ms: AtomicU64,
    /// 调度成功数
    pub schedule_success_count: AtomicU64,
    /// 调度失败数
    pub schedule_failure_count: AtomicU64,
    /// 开始时间
    pub start_time: Instant,
}

impl SchedulerMetrics {
    pub fn new() -> Self {
        Self {
            account_metrics: Arc::new(RwLock::new(HashMap::new())),
            request_count: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            schedule_success_count: AtomicU64::new(0),
            schedule_failure_count: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// 获取账号指标（只读，不创建）
    pub async fn get_account_metrics(&self, account_id: Uuid) -> Option<Arc<AccountMetrics>> {
        let metrics = self.account_metrics.read().await;
        metrics.get(&account_id).cloned()
    }

    /// 获取或创建账号指标
    pub async fn get_or_create_account_metrics(&self, account_id: Uuid) -> Arc<AccountMetrics> {
        let metrics = self.account_metrics.read().await;
        if let Some(m) = metrics.get(&account_id) {
            return Arc::clone(m);
        }
        drop(metrics);

        let mut metrics = self.account_metrics.write().await;
        metrics
            .entry(account_id)
            .or_insert_with(|| Arc::new(AccountMetrics::new()))
            .clone()
    }

    /// 记录调度成功
    pub fn record_schedule_success(&self, latency_ms: u64) {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
        self.schedule_success_count.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录调度失败
    pub fn record_schedule_failure(&self) {
        self.schedule_failure_count.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取平均延迟
    pub fn get_avg_latency_ms(&self) -> u64 {
        let count = self.request_count.load(Ordering::SeqCst);
        if count == 0 {
            0
        } else {
            self.total_latency_ms.load(Ordering::SeqCst) / count
        }
    }

    /// 获取运行时间（秒）
    pub fn get_uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// 获取全局快照
    pub async fn global_snapshot(&self) -> SchedulerMetricsSnapshot {
        let account_snapshots: HashMap<Uuid, AccountMetricsSnapshot> = {
            let metrics = self.account_metrics.read().await;
            let mut snapshots = HashMap::new();
            for (id, m) in metrics.iter() {
                snapshots.insert(*id, m.snapshot().await);
            }
            snapshots
        };

        SchedulerMetricsSnapshot {
            account_metrics: account_snapshots,
            request_count: self.request_count.load(Ordering::SeqCst),
            avg_latency_ms: self.get_avg_latency_ms(),
            schedule_success_count: self.schedule_success_count.load(Ordering::SeqCst),
            schedule_failure_count: self.schedule_failure_count.load(Ordering::SeqCst),
            uptime_secs: self.get_uptime_secs(),
        }
    }

    /// 重置所有指标
    pub async fn reset(&self) {
        self.request_count.store(0, Ordering::SeqCst);
        self.total_latency_ms.store(0, Ordering::SeqCst);
        self.schedule_success_count.store(0, Ordering::SeqCst);
        self.schedule_failure_count.store(0, Ordering::SeqCst);

        let metrics = self.account_metrics.read().await;
        for m in metrics.values() {
            m.reset();
        }
    }

    /// 清理不活跃账号指标
    pub async fn cleanup_inactive_accounts(&self, max_age_secs: i64) {
        let mut metrics = self.account_metrics.write().await;
        let now = Utc::now().timestamp();

        metrics.retain(|_, m| {
            let last_used = m.last_used.load(Ordering::SeqCst);
            now - last_used < max_age_secs
        });
    }
}

impl Default for SchedulerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 调度器全局指标快照
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchedulerMetricsSnapshot {
    pub account_metrics: HashMap<Uuid, AccountMetricsSnapshot>,
    pub request_count: u64,
    pub avg_latency_ms: u64,
    pub schedule_success_count: u64,
    pub schedule_failure_count: u64,
    pub uptime_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_account_metrics_creation() {
        let metrics = AccountMetrics::new();
        assert_eq!(metrics.get_active_connections(), 0);
        assert_eq!(metrics.get_avg_latency_ms(), 0);
        assert_eq!(metrics.get_success_rate(), 1.0);
    }

    #[tokio::test]
    async fn test_record_request() {
        let metrics = AccountMetrics::new();

        metrics.record_request_start();
        assert_eq!(metrics.get_active_connections(), 1);

        metrics.record_request_success(100, Some(50)).await;
        assert_eq!(metrics.get_active_connections(), 0);
        assert_eq!(metrics.get_avg_latency_ms(), 100);
        assert_eq!(metrics.get_total_cost_cents(), 50);
    }

    #[tokio::test]
    async fn test_error_rate_calculation() {
        let metrics = AccountMetrics::new();

        // 10 次请求，2 次失败
        for _ in 0..8 {
            metrics.record_request_start();
            metrics.record_request_success(50, None).await;
        }

        for _ in 0..2 {
            metrics.record_request_start();
            metrics.record_request_failure().await;
        }

        assert_eq!(metrics.total_requests.load(Ordering::SeqCst), 10);
        assert_eq!(metrics.failed_requests.load(Ordering::SeqCst), 2);

        // 错误率应该约为 0.2
        let error_rate = metrics.get_error_rate();
        assert!(error_rate > 0.15 && error_rate < 0.25);
    }

    #[tokio::test]
    async fn test_scheduler_metrics() {
        let metrics = SchedulerMetrics::new();

        metrics.record_schedule_success(100);
        metrics.record_schedule_success(200);
        metrics.record_schedule_failure();

        assert_eq!(metrics.request_count.load(Ordering::SeqCst), 2);
        assert_eq!(metrics.get_avg_latency_ms(), 150);
        assert_eq!(metrics.schedule_failure_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_get_or_create_account_metrics() {
        let metrics = SchedulerMetrics::new();
        let account_id = Uuid::new_v4();

        let m1 = metrics.get_or_create_account_metrics(account_id).await;
        let m2 = metrics.get_or_create_account_metrics(account_id).await;

        // 应该返回同一个实例
        assert!(Arc::ptr_eq(&m1, &m2));
    }

    #[tokio::test]
    async fn test_metrics_snapshot() {
        let metrics = AccountMetrics::new();

        metrics.record_request_start();
        metrics.record_request_success(150, Some(25)).await;

        let snapshot = metrics.snapshot().await;

        assert_eq!(snapshot.active_connections, 0);
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.success_requests, 1);
        assert_eq!(snapshot.avg_latency_ms, 150);
        assert_eq!(snapshot.total_cost_cents, 25);
    }
}
