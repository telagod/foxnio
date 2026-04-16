//! QPS 监控服务
//!
//! 提供实时 QPS 监控、历史数据追踪、Prometheus 指标导出

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// QPS 数据点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QpsDataPoint {
    pub timestamp: DateTime<Utc>,
    pub qps: f64,
    pub request_count: u64,
    pub error_count: u64,
}

/// 账号 QPS 统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQpsStats {
    pub account_id: Uuid,
    pub current_qps: f64,
    pub avg_qps_1m: f64,
    pub avg_qps_5m: f64,
    pub avg_qps_15m: f64,
    pub peak_qps: f64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub last_updated: DateTime<Utc>,
}

/// 模型 QPS 统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQpsStats {
    pub model: String,
    pub current_qps: f64,
    pub avg_qps_1m: f64,
    pub avg_qps_5m: f64,
    pub total_requests: u64,
    pub total_errors: u64,
}

/// 全局 QPS 统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalQpsStats {
    pub current_qps: f64,
    pub avg_qps_1m: f64,
    pub avg_qps_5m: f64,
    pub avg_qps_15m: f64,
    pub peak_qps: f64,
    pub total_requests: u64,
    pub total_errors: u64,
    pub active_accounts: usize,
    pub active_models: usize,
    pub last_updated: DateTime<Utc>,
}

/// 时间窗口数据
#[derive(Debug, Clone, Default)]
struct WindowData {
    requests: u64,
    errors: u64,
}

/// QPS 监控配置
#[derive(Debug, Clone)]
pub struct QpsMonitorConfig {
    /// 统计间隔（秒）
    pub interval_seconds: u64,
    /// 1 分钟窗口大小
    pub window_1m: u64,
    /// 5 分钟窗口大小
    pub window_5m: u64,
    /// 15 分钟窗口大小
    pub window_15m: u64,
    /// 最大历史数据点数
    pub max_history_points: usize,
}

impl Default for QpsMonitorConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 1,
            window_1m: 60,
            window_5m: 300,
            window_15m: 900,
            max_history_points: 1000,
        }
    }
}

/// QPS 监控服务
pub struct QpsMonitor {
    config: QpsMonitorConfig,
    /// 全局请求计数
    global_requests: Arc<RwLock<u64>>,
    global_errors: Arc<RwLock<u64>>,
    /// 时间窗口数据
    window_data: Arc<RwLock<HashMap<i64, WindowData>>>,
    /// 账号级统计
    account_stats: Arc<RwLock<HashMap<Uuid, AccountQpsStats>>>,
    /// 模型级统计
    model_stats: Arc<RwLock<HashMap<String, ModelQpsStats>>>,
    /// 历史数据点
    history: Arc<RwLock<Vec<QpsDataPoint>>>,
    /// 峰值 QPS
    peak_qps: Arc<RwLock<f64>>,
    /// 最后更新时间
    last_updated: Arc<RwLock<DateTime<Utc>>>,
}

impl QpsMonitor {
    pub fn new(config: QpsMonitorConfig) -> Self {
        Self {
            config,
            global_requests: Arc::new(RwLock::new(0)),
            global_errors: Arc::new(RwLock::new(0)),
            window_data: Arc::new(RwLock::new(HashMap::new())),
            account_stats: Arc::new(RwLock::new(HashMap::new())),
            model_stats: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            peak_qps: Arc::new(RwLock::new(0.0)),
            last_updated: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// 记录请求
    pub async fn record_request(
        &self,
        account_id: Option<Uuid>,
        model: Option<&str>,
        is_error: bool,
    ) {
        let now = Utc::now();
        let timestamp = now.timestamp();

        // 更新全局计数
        {
            let mut requests = self.global_requests.write().await;
            *requests += 1;
        }
        if is_error {
            let mut errors = self.global_errors.write().await;
            *errors += 1;
        }

        // 更新时间窗口
        {
            let mut window = self.window_data.write().await;
            let data = window.entry(timestamp).or_insert_with(WindowData::default);
            data.requests += 1;
            if is_error {
                data.errors += 1;
            }
        }

        // 更新账号统计
        if let Some(id) = account_id {
            let mut stats = self.account_stats.write().await;
            let stat = stats.entry(id).or_insert_with(|| AccountQpsStats {
                account_id: id,
                current_qps: 0.0,
                avg_qps_1m: 0.0,
                avg_qps_5m: 0.0,
                avg_qps_15m: 0.0,
                peak_qps: 0.0,
                total_requests: 0,
                total_errors: 0,
                last_updated: now,
            });
            stat.total_requests += 1;
            if is_error {
                stat.total_errors += 1;
            }
            stat.last_updated = now;
        }

        // 更新模型统计
        if let Some(m) = model {
            let mut stats = self.model_stats.write().await;
            let stat = stats.entry(m.to_string()).or_insert_with(|| ModelQpsStats {
                model: m.to_string(),
                current_qps: 0.0,
                avg_qps_1m: 0.0,
                avg_qps_5m: 0.0,
                total_requests: 0,
                total_errors: 0,
            });
            stat.total_requests += 1;
            if is_error {
                stat.total_errors += 1;
            }
        }

        // 更新最后更新时间
        {
            let mut last = self.last_updated.write().await;
            *last = now;
        }
    }

    /// 计算当前 QPS
    pub async fn calculate_current_qps(&self) -> f64 {
        let now = Utc::now();
        let window_start = now.timestamp() - self.config.window_1m as i64;

        let window = self.window_data.read().await;
        let total_requests: u64 = window
            .iter()
            .filter(|(ts, _)| **ts >= window_start)
            .map(|(_, data)| data.requests)
            .sum();

        total_requests as f64 / self.config.window_1m as f64
    }

    /// 计算 N 分钟平均 QPS
    pub async fn calculate_avg_qps(&self, window_seconds: u64) -> f64 {
        let now = Utc::now();
        let window_start = now.timestamp() - window_seconds as i64;

        let window = self.window_data.read().await;
        let total_requests: u64 = window
            .iter()
            .filter(|(ts, _)| **ts >= window_start)
            .map(|(_, data)| data.requests)
            .sum();

        if window_seconds > 0 {
            total_requests as f64 / window_seconds as f64
        } else {
            0.0
        }
    }

    /// 获取全局统计
    pub async fn get_global_stats(&self) -> GlobalQpsStats {
        let current_qps = self.calculate_current_qps().await;
        let avg_qps_1m = self.calculate_avg_qps(self.config.window_1m).await;
        let avg_qps_5m = self.calculate_avg_qps(self.config.window_5m).await;
        let avg_qps_15m = self.calculate_avg_qps(self.config.window_15m).await;

        // 更新峰值
        {
            let mut peak = self.peak_qps.write().await;
            if current_qps > *peak {
                *peak = current_qps;
            }
        }

        let peak_qps = *self.peak_qps.read().await;
        let total_requests = *self.global_requests.read().await;
        let total_errors = *self.global_errors.read().await;
        let active_accounts = self.account_stats.read().await.len();
        let active_models = self.model_stats.read().await.len();
        let last_updated = *self.last_updated.read().await;

        GlobalQpsStats {
            current_qps,
            avg_qps_1m,
            avg_qps_5m,
            avg_qps_15m,
            peak_qps,
            total_requests,
            total_errors,
            active_accounts,
            active_models,
            last_updated,
        }
    }

    /// 获取账号统计
    pub async fn get_account_stats(&self, account_id: Uuid) -> Option<AccountQpsStats> {
        let stats = self.account_stats.read().await;
        stats.get(&account_id).cloned()
    }

    /// 获取所有账号统计
    pub async fn get_all_account_stats(&self) -> Vec<AccountQpsStats> {
        let stats = self.account_stats.read().await;
        stats.values().cloned().collect()
    }

    /// 获取模型统计
    pub async fn get_model_stats(&self, model: &str) -> Option<ModelQpsStats> {
        let stats = self.model_stats.read().await;
        stats.get(model).cloned()
    }

    /// 获取所有模型统计
    pub async fn get_all_model_stats(&self) -> Vec<ModelQpsStats> {
        let stats = self.model_stats.read().await;
        stats.values().cloned().collect()
    }

    /// 获取历史数据
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<QpsDataPoint> {
        let history = self.history.read().await;
        let limit = limit.unwrap_or(100).min(history.len());
        history.iter().rev().take(limit).cloned().collect()
    }

    /// 更新统计数据（定时任务调用）
    pub async fn update_stats(&self) -> Result<()> {
        let now = Utc::now();
        let current_qps = self.calculate_current_qps().await;

        // 添加历史数据点
        {
            let mut history = self.history.write().await;
            let total_requests = *self.global_requests.read().await;
            let total_errors = *self.global_errors.read().await;

            history.push(QpsDataPoint {
                timestamp: now,
                qps: current_qps,
                request_count: total_requests,
                error_count: total_errors,
            });

            // 限制历史数据点数量
            if history.len() > self.config.max_history_points {
                history.remove(0);
            }
        }

        // 更新账号 QPS
        {
            let mut stats = self.account_stats.write().await;
            let len = stats.len().max(1) as f64;
            for stat in stats.values_mut() {
                // 简化：使用全局 QPS 作为账号 QPS 的估计
                // 实际应该基于账号的时间窗口计算
                stat.current_qps = current_qps / len;
                stat.avg_qps_1m = current_qps / len;
                stat.avg_qps_5m = current_qps / len;
                stat.avg_qps_15m = current_qps / len;
            }
        }

        // 更新模型 QPS
        {
            let mut stats = self.model_stats.write().await;
            let len = stats.len().max(1) as f64;
            for stat in stats.values_mut() {
                stat.current_qps = current_qps / len;
                stat.avg_qps_1m = current_qps / len;
                stat.avg_qps_5m = current_qps / len;
            }
        }

        Ok(())
    }

    /// 清理过期数据
    pub async fn cleanup(&self) -> Result<()> {
        let now = Utc::now();
        let cutoff = now.timestamp() - self.config.window_15m as i64 * 2;

        // 清理时间窗口数据
        {
            let mut window = self.window_data.write().await;
            window.retain(|ts, _| *ts >= cutoff);
        }

        Ok(())
    }

    /// 重置统计
    pub async fn reset(&self) {
        *self.global_requests.write().await = 0;
        *self.global_errors.write().await = 0;
        self.window_data.write().await.clear();
        self.account_stats.write().await.clear();
        self.model_stats.write().await.clear();
        self.history.write().await.clear();
        *self.peak_qps.write().await = 0.0;
    }

    /// 导出 Prometheus 指标格式
    pub async fn export_prometheus_metrics(&self) -> String {
        let stats = self.get_global_stats().await;

        let mut metrics = String::new();

        // 全局指标
        metrics.push_str("# HELP foxnio_qps_current Current requests per second\n");
        metrics.push_str("# TYPE foxnio_qps_current gauge\n");
        metrics.push_str(&format!("foxnio_qps_current {}\n", stats.current_qps));

        metrics.push_str("# HELP foxnio_qps_avg_1m Average QPS over 1 minute\n");
        metrics.push_str("# TYPE foxnio_qps_avg_1m gauge\n");
        metrics.push_str(&format!("foxnio_qps_avg_1m {}\n", stats.avg_qps_1m));

        metrics.push_str("# HELP foxnio_qps_avg_5m Average QPS over 5 minutes\n");
        metrics.push_str("# TYPE foxnio_qps_avg_5m gauge\n");
        metrics.push_str(&format!("foxnio_qps_avg_5m {}\n", stats.avg_qps_5m));

        metrics.push_str("# HELP foxnio_qps_peak Peak QPS\n");
        metrics.push_str("# TYPE foxnio_qps_peak gauge\n");
        metrics.push_str(&format!("foxnio_qps_peak {}\n", stats.peak_qps));

        metrics.push_str("# HELP foxnio_requests_total Total requests\n");
        metrics.push_str("# TYPE foxnio_requests_total counter\n");
        metrics.push_str(&format!("foxnio_requests_total {}\n", stats.total_requests));

        metrics.push_str("# HELP foxnio_errors_total Total errors\n");
        metrics.push_str("# TYPE foxnio_errors_total counter\n");
        metrics.push_str(&format!("foxnio_errors_total {}\n", stats.total_errors));

        metrics.push_str("# HELP foxnio_active_accounts Active accounts\n");
        metrics.push_str("# TYPE foxnio_active_accounts gauge\n");
        metrics.push_str(&format!(
            "foxnio_active_accounts {}\n",
            stats.active_accounts
        ));

        metrics.push_str("# HELP foxnio_active_models Active models\n");
        metrics.push_str("# TYPE foxnio_active_models gauge\n");
        metrics.push_str(&format!("foxnio_active_models {}\n", stats.active_models));

        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qps_monitor_basic() {
        let monitor = QpsMonitor::new(QpsMonitorConfig::default());

        // 记录一些请求
        for _ in 0..100 {
            monitor
                .record_request(Some(Uuid::new_v4()), Some("gpt-4"), false)
                .await;
        }

        let stats = monitor.get_global_stats().await;
        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.active_accounts, 100);
    }

    #[tokio::test]
    async fn test_qps_monitor_with_errors() {
        let monitor = QpsMonitor::new(QpsMonitorConfig::default());

        for i in 0..50 {
            let is_error = i % 5 == 0;
            monitor
                .record_request(None, Some("claude-3"), is_error)
                .await;
        }

        let stats = monitor.get_global_stats().await;
        assert_eq!(stats.total_requests, 50);
        assert_eq!(stats.total_errors, 10);
    }

    #[tokio::test]
    async fn test_prometheus_export() {
        let monitor = QpsMonitor::new(QpsMonitorConfig::default());

        for _ in 0..10 {
            monitor.record_request(None, None, false).await;
        }

        let metrics = monitor.export_prometheus_metrics().await;
        assert!(metrics.contains("foxnio_qps_current"));
        assert!(metrics.contains("foxnio_requests_total 10"));
    }

    #[tokio::test]
    async fn test_model_stats() {
        let monitor = QpsMonitor::new(QpsMonitorConfig::default());

        monitor.record_request(None, Some("gpt-4"), false).await;
        monitor.record_request(None, Some("gpt-4"), false).await;
        monitor.record_request(None, Some("claude-3"), false).await;

        let all_stats = monitor.get_all_model_stats().await;
        assert_eq!(all_stats.len(), 2);

        let gpt4_stats = monitor.get_model_stats("gpt-4").await.unwrap();
        assert_eq!(gpt4_stats.total_requests, 2);
    }
}
