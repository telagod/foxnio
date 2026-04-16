//! 运维指标收集器 - Ops Metrics Collector
//!
//! 收集和聚合系统运行指标

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 指标类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// 指标值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub name: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// 指标聚合结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAggregation {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

/// 系统指标快照（从数据库实时查询）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_users: i64,
    pub active_users_24h: i64,
    pub today_requests: i64,
    pub today_tokens: i64,
    pub today_cost_cents: i64,
    pub error_rate_1h: f64,
    pub avg_response_time_ms: f64,
    pub snapshot_at: DateTime<Utc>,
}

/// 指标收集器配置
#[derive(Debug, Clone)]
pub struct MetricsCollectorConfig {
    pub collection_interval_secs: u64,
    pub retention_hours: u64,
    pub max_metrics_per_type: usize,
    pub aggregation_window_secs: u64,
}

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self {
            collection_interval_secs: 60,
            retention_hours: 24,
            max_metrics_per_type: 10000,
            aggregation_window_secs: 300,
        }
    }
}

/// 指标收集器
pub struct MetricsCollector {
    config: MetricsCollectorConfig,
    db: sea_orm::DatabaseConnection,

    // 指标缓冲区
    counters: Arc<RwLock<HashMap<String, Vec<MetricValue>>>>,
    gauges: Arc<RwLock<HashMap<String, Vec<MetricValue>>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<MetricValue>>>>,

    // 停止信号
    stop_signal: Arc<RwLock<bool>>,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new(db: sea_orm::DatabaseConnection, config: MetricsCollectorConfig) -> Self {
        Self {
            config,
            db,
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }

    /// 从数据库收集一次完整的指标快照
    pub async fn collect_snapshot(&self) -> Result<MetricsSnapshot> {
        let now = Utc::now();
        let twenty_four_hours_ago = now - Duration::hours(24);
        let one_hour_ago = now - Duration::hours(1);

        // 1. total users + active users (last 24h via usages)
        let user_row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    (SELECT COUNT(*) FROM users) AS total_users,
                    (SELECT COUNT(DISTINCT user_id) FROM usages WHERE created_at >= $1) AS active_users_24h
                "#,
                [twenty_four_hours_ago.into()],
            ))
            .await?;

        let (total_users, active_users_24h) = match user_row {
            Some(ref r) => {
                let t: i64 = r.try_get_by_index(0).unwrap_or(0);
                let a: i64 = r.try_get_by_index(1).unwrap_or(0);
                (t, a)
            }
            None => (0, 0),
        };

        // 2. today's requests, tokens, cost
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight");
        let today_start_utc = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

        let usage_row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    COUNT(*) AS total_requests,
                    COALESCE(SUM(input_tokens + output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(cost), 0) AS total_cost
                FROM usages
                WHERE created_at >= $1
                "#,
                [today_start_utc.into()],
            ))
            .await?;

        let (today_requests, today_tokens, today_cost_cents) = match usage_row {
            Some(ref r) => {
                let req: i64 = r.try_get_by_index(0).unwrap_or(0);
                let tok: i64 = r.try_get_by_index(1).unwrap_or(0);
                let cost: i64 = r.try_get_by_index(2).unwrap_or(0);
                (req, tok, cost)
            }
            None => (0, 0, 0),
        };

        // 3. error rate (failed / total in last hour)
        let error_row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN success = false THEN 1 ELSE 0 END), 0) AS failed
                FROM usages
                WHERE created_at >= $1
                "#,
                [one_hour_ago.into()],
            ))
            .await?;

        let error_rate_1h = match error_row {
            Some(ref r) => {
                let total: i64 = r.try_get_by_index(0).unwrap_or(0);
                let failed: i64 = r.try_get_by_index(1).unwrap_or(0);
                if total > 0 {
                    failed as f64 / total as f64
                } else {
                    0.0
                }
            }
            None => 0.0,
        };

        // 4. avg response time from usages.metadata->>'response_time_ms' (last hour)
        let rt_row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT AVG((metadata->>'response_time_ms')::float)
                FROM usages
                WHERE created_at >= $1
                  AND metadata IS NOT NULL
                  AND metadata->>'response_time_ms' IS NOT NULL
                "#,
                [one_hour_ago.into()],
            ))
            .await?;

        let avg_response_time_ms: f64 = rt_row
            .as_ref()
            .and_then(|r| r.try_get_by_index::<Option<f64>>(0).ok())
            .flatten()
            .unwrap_or(0.0);

        Ok(MetricsSnapshot {
            total_users,
            active_users_24h,
            today_requests,
            today_tokens,
            today_cost_cents,
            error_rate_1h,
            avg_response_time_ms,
            snapshot_at: now,
        })
    }

    /// 启动指标收集
    pub async fn start(&self) -> Result<()> {
        tracing::info!("启动指标收集器");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.collection_interval_secs,
        ));

        loop {
            if *self.stop_signal.read().await {
                break;
            }

            interval.tick().await;

            // 收集指标
            if let Err(e) = self.collect_metrics().await {
                tracing::error!("指标收集失败: {}", e);
            }

            // 聚合指标
            if let Err(e) = self.aggregate_metrics().await {
                tracing::error!("指标聚合失败: {}", e);
            }

            // 清理过期指标
            if let Err(e) = self.cleanup_expired_metrics().await {
                tracing::error!("清理过期指标失败: {}", e);
            }
        }

        Ok(())
    }

    /// 停止指标收集
    pub async fn stop(&self) -> Result<()> {
        let mut stop = self.stop_signal.write().await;
        *stop = true;
        Ok(())
    }

    /// 记录计数器
    pub async fn record_counter(
        &self,
        name: &str,
        value: f64,
        labels: HashMap<String, String>,
    ) -> Result<()> {
        let metric = MetricValue {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value,
            labels,
            timestamp: Utc::now(),
        };

        let mut counters = self.counters.write().await;
        counters
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(metric);

        // 限制大小
        if let Some(vec) = counters.get_mut(name) {
            if vec.len() > self.config.max_metrics_per_type {
                let excess = vec.len() - self.config.max_metrics_per_type;
                vec.drain(0..excess);
            }
        }

        Ok(())
    }

    /// 记录仪表盘
    pub async fn record_gauge(
        &self,
        name: &str,
        value: f64,
        labels: HashMap<String, String>,
    ) -> Result<()> {
        let metric = MetricValue {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            value,
            labels,
            timestamp: Utc::now(),
        };

        let mut gauges = self.gauges.write().await;
        gauges
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(metric);

        // 限制大小
        if let Some(vec) = gauges.get_mut(name) {
            if vec.len() > self.config.max_metrics_per_type {
                let excess = vec.len() - self.config.max_metrics_per_type;
                vec.drain(0..excess);
            }
        }

        Ok(())
    }

    /// 记录直方图
    pub async fn record_histogram(
        &self,
        name: &str,
        value: f64,
        labels: HashMap<String, String>,
    ) -> Result<()> {
        let metric = MetricValue {
            name: name.to_string(),
            metric_type: MetricType::Histogram,
            value,
            labels,
            timestamp: Utc::now(),
        };

        let mut histograms = self.histograms.write().await;
        histograms
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(metric);

        // 限制大小
        if let Some(vec) = histograms.get_mut(name) {
            if vec.len() > self.config.max_metrics_per_type {
                let excess = vec.len() - self.config.max_metrics_per_type;
                vec.drain(0..excess);
            }
        }

        Ok(())
    }

    /// 递增计数器
    pub async fn increment_counter(
        &self,
        name: &str,
        labels: HashMap<String, String>,
    ) -> Result<()> {
        self.record_counter(name, 1.0, labels).await
    }

    /// 收集系统指标
    async fn collect_metrics(&self) -> Result<()> {
        // 收集请求计数
        self.collect_request_metrics().await?;

        // 收集账号指标
        self.collect_account_metrics().await?;

        // 收集系统资源指标
        self.collect_system_metrics().await?;

        Ok(())
    }

    /// 收集请求指标
    async fn collect_request_metrics(&self) -> Result<()> {
        use crate::entity::{accounts, usages};
        use sea_orm::{DbBackend, FromQueryResult, Statement};

        #[derive(Debug, FromQueryResult)]
        struct ProviderStats {
            provider: String,
            cnt: i64,
        }

        let rows = ProviderStats::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT a.provider, COUNT(u.id)::bigint AS cnt
               FROM usages u
               INNER JOIN accounts a ON u.account_id = a.id
               WHERE u.created_at >= NOW() - INTERVAL '1 hour'
               GROUP BY a.provider"#,
            [],
        ))
        .all(&self.db)
        .await
        .unwrap_or_default();

        for row in rows {
            let mut labels = HashMap::new();
            labels.insert("platform".to_string(), row.provider);
            self.record_gauge("requests_1h", row.cnt as f64, labels).await?;
        }

        // Active connections from Prometheus metric
        let active = crate::metrics::ACTIVE_CONNECTIONS.get();
        let mut labels = HashMap::new();
        labels.insert("platform".to_string(), "all".to_string());
        self.record_gauge("requests_active", active as f64, labels).await?;

        Ok(())
    }

    /// 收集账号指标
    async fn collect_account_metrics(&self) -> Result<()> {
        use crate::entity::accounts;
        use sea_orm::{DbBackend, FromQueryResult, Statement};

        #[derive(Debug, FromQueryResult)]
        struct AccountStats {
            provider: String,
            status: String,
            cnt: i64,
        }

        let rows = AccountStats::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT provider, status, COUNT(*)::bigint AS cnt
               FROM accounts GROUP BY provider, status"#,
            [],
        ))
        .all(&self.db)
        .await
        .unwrap_or_default();

        for row in rows {
            let mut labels = HashMap::new();
            labels.insert("platform".to_string(), row.provider.clone());
            labels.insert("status".to_string(), row.status.clone());
            self.record_gauge("accounts_total", row.cnt as f64, labels).await?;

            // 更新 Prometheus 指标
            crate::metrics::ACCOUNT_POOL_STATUS
                .with_label_values(&[&row.provider, &row.status])
                .set(row.cnt);
        }

        Ok(())
    }

    /// 收集系统资源指标
    async fn collect_system_metrics(&self) -> Result<()> {
        // 收集内存使用
        let memory_usage = self.get_memory_usage().await;
        let mut labels = HashMap::new();
        labels.insert("type".to_string(), "memory".to_string());
        self.record_gauge("system_memory_usage_bytes", memory_usage, labels)
            .await?;

        // 收集 CPU 使用
        let cpu_usage = self.get_cpu_usage().await;
        let mut labels = HashMap::new();
        labels.insert("type".to_string(), "cpu".to_string());
        self.record_gauge("system_cpu_usage_percent", cpu_usage, labels)
            .await?;

        Ok(())
    }

    /// 获取内存使用（RSS，字节）
    async fn get_memory_usage(&self) -> f64 {
        // 从 /proc/self/status 读取 VmRSS（Linux）
        if let Ok(status) = tokio::fs::read_to_string("/proc/self/status").await {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let kb: f64 = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0.0);
                    return kb * 1024.0; // KB → bytes
                }
            }
        }
        0.0
    }

    /// 获取 CPU 使用率（近似，0-100）
    async fn get_cpu_usage(&self) -> f64 {
        // 从 /proc/self/stat 读取 utime+stime
        if let Ok(stat) = tokio::fs::read_to_string("/proc/self/stat").await {
            let parts: Vec<&str> = stat.split_whitespace().collect();
            if parts.len() > 14 {
                let utime: f64 = parts[13].parse().unwrap_or(0.0);
                let stime: f64 = parts[14].parse().unwrap_or(0.0);
                // 粗略估算：总 CPU ticks / uptime
                let total_ticks = utime + stime;
                if let Ok(uptime) = tokio::fs::read_to_string("/proc/uptime").await {
                    let uptime_secs: f64 = uptime
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1.0);
                    let clock_ticks = 100.0; // sysconf(_SC_CLK_TCK) 通常为 100
                    return (total_ticks / clock_ticks / uptime_secs) * 100.0;
                }
            }
        }
        0.0
    }

    /// 聚合指标
    async fn aggregate_metrics(&self) -> Result<()> {
        let now = Utc::now();
        let window_start = now - Duration::seconds(self.config.aggregation_window_secs as i64);

        // 聚合计数器
        self.aggregate_counters(window_start, now).await?;

        // 聚合仪表盘
        self.aggregate_gauges(window_start, now).await?;

        // 聚合直方图
        self.aggregate_histograms(window_start, now).await?;

        Ok(())
    }

    /// 聚合计数器
    async fn aggregate_counters(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
        let counters = self.counters.read().await;

        for (name, values) in counters.iter() {
            let filtered: Vec<_> = values
                .iter()
                .filter(|v| v.timestamp >= start && v.timestamp <= end)
                .collect();

            if filtered.is_empty() {
                continue;
            }

            let sum: f64 = filtered.iter().map(|v| v.value).sum();
            let count = filtered.len() as u64;

            // TODO: 存储聚合结果到数据库
            tracing::debug!("计数器 {}: sum={}, count={}", name, sum, count);
        }

        Ok(())
    }

    /// 聚合仪表盘
    async fn aggregate_gauges(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
        let gauges = self.gauges.read().await;

        for (name, values) in gauges.iter() {
            let filtered: Vec<_> = values
                .iter()
                .filter(|v| v.timestamp >= start && v.timestamp <= end)
                .collect();

            if filtered.is_empty() {
                continue;
            }

            let values: Vec<f64> = filtered.iter().map(|v| v.value).collect();
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            // TODO: 存储聚合结果到数据库
            tracing::debug!("仪表盘 {}: avg={}, min={}, max={}", name, avg, min, max);
        }

        Ok(())
    }

    /// 聚合直方图
    async fn aggregate_histograms(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<()> {
        let histograms = self.histograms.read().await;

        for (name, values) in histograms.iter() {
            let filtered: Vec<_> = values
                .iter()
                .filter(|v| v.timestamp >= start && v.timestamp <= end)
                .collect();

            if filtered.is_empty() {
                continue;
            }

            let mut values: Vec<f64> = filtered.iter().map(|v| v.value).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let count = values.len() as u64;
            let sum: f64 = values.iter().sum();
            let avg = sum / count as f64;
            let min = values[0];
            let max = values[count as usize - 1];

            let p50 = percentile(&values, 50);
            let p95 = percentile(&values, 95);
            let p99 = percentile(&values, 99);

            let aggregation = MetricAggregation {
                name: name.clone(),
                count,
                sum,
                avg,
                min,
                max,
                p50,
                p95,
                p99,
                start_time: start,
                end_time: end,
            };

            // TODO: 存储聚合结果到数据库
            tracing::debug!("直方图聚合: {:?}", aggregation);
        }

        Ok(())
    }

    /// 清理过期指标
    async fn cleanup_expired_metrics(&self) -> Result<()> {
        let cutoff = Utc::now() - Duration::hours(self.config.retention_hours as i64);

        {
            let mut counters = self.counters.write().await;
            for (_, values) in counters.iter_mut() {
                values.retain(|v| v.timestamp > cutoff);
            }
        }

        {
            let mut gauges = self.gauges.write().await;
            for (_, values) in gauges.iter_mut() {
                values.retain(|v| v.timestamp > cutoff);
            }
        }

        {
            let mut histograms = self.histograms.write().await;
            for (_, values) in histograms.iter_mut() {
                values.retain(|v| v.timestamp > cutoff);
            }
        }

        Ok(())
    }

    /// 获取指标统计
    pub async fn get_metrics_summary(&self) -> HashMap<String, usize> {
        let mut summary = HashMap::new();

        summary.insert("counters".to_string(), self.counters.read().await.len());
        summary.insert("gauges".to_string(), self.gauges.read().await.len());
        summary.insert("histograms".to_string(), self.histograms.read().await.len());

        summary
    }
}

/// 计算百分位数
fn percentile(sorted_values: &[f64], p: u32) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let idx = ((sorted_values.len() - 1) as f64 * p as f64 / 100.0).round() as usize;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_metrics_collector() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = MetricsCollectorConfig::default();
        let collector = MetricsCollector::new(db, config);

        // 记录指标
        let mut labels = HashMap::new();
        labels.insert("platform".to_string(), "openai".to_string());

        collector
            .increment_counter("requests_total", labels.clone())
            .await
            .unwrap();
        collector
            .record_gauge("requests_active", 5.0, labels.clone())
            .await
            .unwrap();
        collector
            .record_histogram("request_duration_ms", 150.0, labels)
            .await
            .unwrap();

        let summary = collector.get_metrics_summary().await;
        assert_eq!(summary.get("counters"), Some(&1));
        assert_eq!(summary.get("gauges"), Some(&1));
        assert_eq!(summary.get("histograms"), Some(&1));
    }

    #[test]
    fn test_percentile() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(percentile(&values, 50), 30.0);
        assert_eq!(percentile(&values, 0), 10.0);
        assert_eq!(percentile(&values, 100), 50.0);
    }
}
