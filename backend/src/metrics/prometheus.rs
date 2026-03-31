//! Prometheus 格式导出模块
//!
//! 提供 Prometheus 兼容的指标导出功能，包括自定义指标注册和标签支持。
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec,
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder,
};
use std::collections::HashMap;

/// 自定义 Prometheus 注册表
pub struct PrometheusRegistry {
    registry: Registry,
}

impl PrometheusRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
        }
    }

    /// 使用默认注册表
    pub fn default_registry() -> Self {
        Self {
            registry: prometheus::default_registry().clone(),
        }
    }

    /// 注册计数器
    pub fn register_counter(&self, name: &str, help: &str) -> Result<Counter, String> {
        let counter =
            Counter::new(name, help).map_err(|e| format!("Failed to create counter: {e}"))?;
        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| format!("Failed to register counter: {e}"))?;
        Ok(counter)
    }

    /// 注册带标签的计数器
    pub fn register_counter_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<CounterVec, String> {
        let counter = CounterVec::new(Opts::new(name, help), labels)
            .map_err(|e| format!("Failed to create counter vec: {e}"))?;
        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| format!("Failed to register counter vec: {e}"))?;
        Ok(counter)
    }

    /// 注册整数计数器
    pub fn register_int_counter(&self, name: &str, help: &str) -> Result<IntCounter, String> {
        let counter = IntCounter::new(name, help)
            .map_err(|e| format!("Failed to create int counter: {e}"))?;
        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| format!("Failed to register int counter: {e}"))?;
        Ok(counter)
    }

    /// 注册带标签的整数计数器
    pub fn register_int_counter_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<IntCounterVec, String> {
        let counter = IntCounterVec::new(Opts::new(name, help), labels)
            .map_err(|e| format!("Failed to create int counter vec: {e}"))?;
        self.registry
            .register(Box::new(counter.clone()))
            .map_err(|e| format!("Failed to register int counter vec: {e}"))?;
        Ok(counter)
    }

    /// 注册计量器
    pub fn register_gauge(&self, name: &str, help: &str) -> Result<Gauge, String> {
        let gauge = Gauge::new(name, help).map_err(|e| format!("Failed to create gauge: {e}"))?;
        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| format!("Failed to register gauge: {e}"))?;
        Ok(gauge)
    }

    /// 注册带标签的计量器
    pub fn register_gauge_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<GaugeVec, String> {
        let gauge = GaugeVec::new(Opts::new(name, help), labels)
            .map_err(|e| format!("Failed to create gauge vec: {e}"))?;
        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| format!("Failed to register gauge vec: {e}"))?;
        Ok(gauge)
    }

    /// 注册整数计量器
    pub fn register_int_gauge(&self, name: &str, help: &str) -> Result<IntGauge, String> {
        let gauge =
            IntGauge::new(name, help).map_err(|e| format!("Failed to create int gauge: {e}"))?;
        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| format!("Failed to register int gauge: {e}"))?;
        Ok(gauge)
    }

    /// 注册带标签的整数计量器
    pub fn register_int_gauge_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<IntGaugeVec, String> {
        let gauge = IntGaugeVec::new(Opts::new(name, help), labels)
            .map_err(|e| format!("Failed to create int gauge vec: {e}"))?;
        self.registry
            .register(Box::new(gauge.clone()))
            .map_err(|e| format!("Failed to register int gauge vec: {e}"))?;
        Ok(gauge)
    }

    /// 注册直方图
    pub fn register_histogram(
        &self,
        name: &str,
        help: &str,
        buckets: Vec<f64>,
    ) -> Result<Histogram, String> {
        let histogram = Histogram::with_opts(HistogramOpts::new(name, help).buckets(buckets))
            .map_err(|e| format!("Failed to create histogram: {e}"))?;
        self.registry
            .register(Box::new(histogram.clone()))
            .map_err(|e| format!("Failed to register histogram: {e}"))?;
        Ok(histogram)
    }

    /// 注册带标签的直方图
    pub fn register_histogram_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
        buckets: Vec<f64>,
    ) -> Result<HistogramVec, String> {
        let histogram = HistogramVec::new(HistogramOpts::new(name, help).buckets(buckets), labels)
            .map_err(|e| format!("Failed to create histogram vec: {e}"))?;
        self.registry
            .register(Box::new(histogram.clone()))
            .map_err(|e| format!("Failed to register histogram vec: {e}"))?;
        Ok(histogram)
    }

    /// 导出为 Prometheus 文本格式
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();

        let mut buffer = Vec::new();
        if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
            tracing::error!("Failed to encode metrics: {}", e);
            return String::new();
        }

        String::from_utf8(buffer).unwrap_or_default()
    }
}

impl Default for PrometheusRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

/// Prometheus 标签构建器
#[derive(Debug, Clone)]
pub struct LabelBuilder {
    labels: HashMap<String, String>,
}

impl LabelBuilder {
    /// 创建新的标签构建器
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
        }
    }

    /// 添加模型标签
    pub fn model(mut self, model: &str) -> Self {
        self.labels.insert("model".to_string(), model.to_string());
        self
    }

    /// 添加提供商标签
    pub fn provider(mut self, provider: &str) -> Self {
        self.labels
            .insert("provider".to_string(), provider.to_string());
        self
    }

    /// 添加用户标签
    pub fn user(mut self, user: &str) -> Self {
        self.labels.insert("user".to_string(), user.to_string());
        self
    }

    /// 添加端点标签
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.labels
            .insert("endpoint".to_string(), endpoint.to_string());
        self
    }

    /// 添加状态码标签
    pub fn status_code(mut self, code: u16) -> Self {
        self.labels
            .insert("status_code".to_string(), code.to_string());
        self
    }

    /// 添加错误类型标签
    pub fn error_type(mut self, error_type: &str) -> Self {
        self.labels
            .insert("error_type".to_string(), error_type.to_string());
        self
    }

    /// 添加自定义标签
    pub fn custom(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// 构建标签列表
    pub fn build(&self) -> Vec<(&str, &str)> {
        self.labels
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    /// 构建标签值列表（按顺序）
    pub fn build_values(&self, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .map(|k| {
                self.labels
                    .get(*k)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .collect()
    }
}

impl Default for LabelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Prometheus 指标格式化器
pub struct MetricsFormatter;

impl MetricsFormatter {
    /// 格式化计数器
    pub fn format_counter(name: &str, value: u64, labels: &[(&str, &str)]) -> String {
        if labels.is_empty() {
            format!("{name} {value}\n")
        } else {
            let label_str = labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(",");
            format!("{}{{{}}} {}\n", name, label_str, value)
        }
    }

    /// 格式化计量器
    pub fn format_gauge(name: &str, value: f64, labels: &[(&str, &str)]) -> String {
        if labels.is_empty() {
            format!("{name} {value}\n")
        } else {
            let label_str = labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(",");
            format!("{}{{{}}} {}\n", name, label_str, value)
        }
    }

    /// 格式化直方图
    pub fn format_histogram(
        name: &str,
        buckets: &[f64],
        counts: &[u64],
        sum: f64,
        count: u64,
    ) -> String {
        let mut output = String::new();
        let mut cumulative = 0u64;

        for (i, bucket) in buckets.iter().enumerate() {
            cumulative += counts.get(i).copied().unwrap_or(0);
            output.push_str(&format!(
                "{}_bucket{{le=\"{}\"}} {}\n",
                name, bucket, cumulative
            ));
        }
        output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", name, count));
        output.push_str(&format!("{name}_sum {sum}\n"));
        output.push_str(&format!("{name}_count {count}\n"));

        output
    }

    /// 添加 HELP 注释
    pub fn add_help(name: &str, help: &str) -> String {
        format!("# HELP {name} {help}\n")
    }

    /// 添加 TYPE 注释
    pub fn add_type(name: &str, metric_type: &str) -> String {
        format!("# TYPE {name} {metric_type}\n")
    }
}

/// 延迟直方图助手
pub struct LatencyHistogram {
    buckets: Vec<f64>,
    counts: Vec<u64>,
    sum: f64,
    total_count: u64,
}

impl LatencyHistogram {
    /// 创建默认延迟直方图
    pub fn new() -> Self {
        Self {
            buckets: vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
            ],
            counts: vec![0; 14], // 13 buckets + 1 for +Inf
            sum: 0.0,
            total_count: 0,
        }
    }

    /// 创建自定义延迟直方图
    pub fn with_buckets(buckets: Vec<f64>) -> Self {
        let count = buckets.len() + 1;
        Self {
            buckets,
            counts: vec![0; count],
            sum: 0.0,
            total_count: 0,
        }
    }

    /// 观察一个值
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.total_count += 1;

        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                self.counts[i] += 1;
            }
        }
        // +Inf bucket
        self.counts[self.buckets.len()] += 1;
    }

    /// 获取百分位数
    pub fn percentile(&self, p: f64) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }

        let target = (self.total_count as f64 * p / 100.0) as u64;
        let mut cumulative = 0u64;

        for (i, bucket) in self.buckets.iter().enumerate() {
            cumulative += self.counts[i];
            if cumulative >= target {
                return *bucket;
            }
        }

        *self.buckets.last().unwrap_or(&60.0)
    }

    /// 获取 P50
    pub fn p50(&self) -> f64 {
        self.percentile(50.0)
    }

    /// 获取 P95
    pub fn p95(&self) -> f64 {
        self.percentile(95.0)
    }

    /// 获取 P99
    pub fn p99(&self) -> f64 {
        self.percentile(99.0)
    }

    /// 获取平均值
    pub fn avg(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            self.sum / self.total_count as f64
        }
    }

    /// 重置
    pub fn reset(&mut self) {
        for count in &mut self.counts {
            *count = 0;
        }
        self.sum = 0.0;
        self.total_count = 0;
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// 导出指标快照
pub struct MetricsSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metrics: HashMap<String, MetricValue>,
}

/// 指标值
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram {
        buckets: Vec<f64>,
        counts: Vec<u64>,
        sum: f64,
        count: u64,
    },
}

/// 导出系统指标快照
pub fn export_snapshot() -> MetricsSnapshot {
    use super::*;

    let mut metrics = HashMap::new();

    // 收集主要指标
    metrics.insert(
        "requests_total".to_string(),
        MetricValue::Counter(REQUESTS_TOTAL.get()),
    );
    metrics.insert(
        "active_connections".to_string(),
        MetricValue::Gauge(ACTIVE_CONNECTIONS.get() as f64),
    );
    metrics.insert(
        "cache_hits".to_string(),
        MetricValue::Counter(CACHE_HITS.get()),
    );
    metrics.insert(
        "cache_misses".to_string(),
        MetricValue::Counter(CACHE_MISSES.get()),
    );
    metrics.insert(
        "cost_total".to_string(),
        MetricValue::Gauge(COST_TOTAL.get()),
    );

    MetricsSnapshot {
        timestamp: chrono::Utc::now(),
        metrics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_registry_counter() {
        let registry = PrometheusRegistry::new();
        let counter = registry.register_counter("test_counter", "Test counter");

        assert!(counter.is_ok());
    }

    #[test]
    fn test_prometheus_registry_gauge() {
        let registry = PrometheusRegistry::new();
        let gauge = registry.register_gauge("test_gauge", "Test gauge");

        assert!(gauge.is_ok());
    }

    #[test]
    fn test_prometheus_registry_histogram() {
        let registry = PrometheusRegistry::new();
        let histogram = registry.register_histogram(
            "test_histogram",
            "Test histogram",
            vec![0.1, 0.5, 1.0, 5.0],
        );

        assert!(histogram.is_ok());
    }

    #[test]
    fn test_prometheus_registry_counter_vec() {
        let registry = PrometheusRegistry::new();
        let counter = registry.register_counter_vec(
            "test_counter_vec",
            "Test counter vec",
            &["model", "provider"],
        );

        assert!(counter.is_ok());
    }

    #[test]
    fn test_label_builder() {
        let binding = LabelBuilder::new()
            .model("gpt-4")
            .provider("openai")
            .user("user123");
        let labels = binding.build();

        assert_eq!(labels.len(), 3);
        assert!(labels.iter().any(|(k, _)| *k == "model"));
    }

    #[test]
    fn test_metrics_formatter_counter() {
        let output = MetricsFormatter::format_counter(
            "requests_total",
            100,
            &[("method", "GET"), ("status", "200")],
        );

        assert!(output.contains("requests_total"));
        assert!(output.contains("100"));
    }

    #[test]
    fn test_latency_histogram() {
        let mut hist = LatencyHistogram::new();

        hist.observe(0.01);
        hist.observe(0.05);
        hist.observe(0.1);
        hist.observe(1.0);
        hist.observe(5.0);

        assert_eq!(hist.total_count, 5);
        assert!(hist.avg() > 0.0);
        assert!(hist.p50() > 0.0);
        assert!(hist.p95() > 0.0);
        assert!(hist.p99() > 0.0);
    }

    #[test]
    fn test_latency_histogram_percentiles() {
        let mut hist = LatencyHistogram::new();

        // 添加 100 个值
        for i in 0..100 {
            hist.observe((i + 1) as f64 / 100.0);
        }

        let p50 = hist.p50();
        let p95 = hist.p95();
        let p99 = hist.p99();

        // 验证 P99 > P95 > P50
        assert!(p99 >= p95);
        assert!(p95 >= p50);
    }

    #[test]
    fn test_export_snapshot() {
        let snapshot = export_snapshot();

        assert!(!snapshot.metrics.is_empty());
        assert!(snapshot.metrics.contains_key("requests_total"));
        assert!(snapshot.metrics.contains_key("active_connections"));
    }

    #[test]
    fn test_prometheus_registry_export() {
        let registry = PrometheusRegistry::new();

        if let Ok(counter) = registry.register_counter("export_test_counter", "Export test") {
            counter.inc();
            counter.inc();
            counter.inc();
        }

        let output = registry.export();

        assert!(output.contains("export_test_counter"));
    }
}
