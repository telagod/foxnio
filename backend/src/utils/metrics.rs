//! 监控指标收集

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// 指标类型
#[derive(Debug, Clone)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// 指标值
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

/// 指标
#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// 指标收集器
pub struct MetricsCollector {
    counters: HashMap<String, Arc<AtomicU64>>,
    gauges: HashMap<String, Arc<std::sync::Mutex<f64>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
        }
    }
    
    /// 注册计数器
    pub fn register_counter(&mut self, name: &str) {
        self.counters.insert(name.to_string(), Arc::new(AtomicU64::new(0)));
    }
    
    /// 注册计量器
    pub fn register_gauge(&mut self, name: &str) {
        self.gauges.insert(name.to_string(), Arc::new(std::sync::Mutex::new(0.0)));
    }
    
    /// 增加计数器
    pub fn increment_counter(&self, name: &str, delta: u64) {
        if let Some(counter) = self.counters.get(name) {
            counter.fetch_add(delta, Ordering::SeqCst);
        }
    }
    
    /// 设置计量器
    pub fn set_gauge(&self, name: &str, value: f64) {
        if let Some(gauge) = self.gauges.get(name) {
            let mut g = gauge.lock().unwrap();
            *g = value;
        }
    }
    
    /// 获取计数器值
    pub fn get_counter(&self, name: &str) -> u64 {
        self.counters
            .get(name)
            .map(|c| c.load(Ordering::SeqCst))
            .unwrap_or(0)
    }
    
    /// 获取计量器值
    pub fn get_gauge(&self, name: &str) -> f64 {
        self.gauges
            .get(name)
            .map(|g| *g.lock().unwrap())
            .unwrap_or(0.0)
    }
    
    /// 导出为 Prometheus 格式
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // 导出计数器
        for (name, counter) in &self.counters {
            let value = counter.load(Ordering::SeqCst);
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }
        
        // 导出计量器
        for (name, gauge) in &self.gauges {
            let value = *gauge.lock().unwrap();
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }
        
        output
    }
    
    /// 获取所有指标
    pub fn get_all_metrics(&self) -> HashMap<String, MetricValue> {
        let mut metrics = HashMap::new();
        
        for (name, counter) in &self.counters {
            metrics.insert(
                name.clone(),
                MetricValue::Counter(counter.load(Ordering::SeqCst)),
            );
        }
        
        for (name, gauge) in &self.gauges {
            metrics.insert(
                name.clone(),
                MetricValue::Gauge(*gauge.lock().unwrap()),
            );
        }
        
        metrics
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 应用指标
pub struct AppMetrics {
    pub requests_total: Arc<AtomicU64>,
    pub requests_success: Arc<AtomicU64>,
    pub requests_failed: Arc<AtomicU64>,
    pub requests_latency_ms: Arc<std::sync::Mutex<Vec<u64>>>,
    pub active_connections: Arc<AtomicU64>,
}

impl AppMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            requests_success: Arc::new(AtomicU64::new(0)),
            requests_failed: Arc::new(AtomicU64::new(0)),
            requests_latency_ms: Arc::new(std::sync::Mutex::new(Vec::new())),
            active_connections: Arc::new(AtomicU64::new(0)),
        }
    }
    
    pub fn record_request(&self, success: bool, latency_ms: u64) {
        self.requests_total.fetch_add(1, Ordering::SeqCst);
        
        if success {
            self.requests_success.fetch_add(1, Ordering::SeqCst);
        } else {
            self.requests_failed.fetch_add(1, Ordering::SeqCst);
        }
        
        self.requests_latency_ms.lock().unwrap().push(latency_ms);
        
        // 保持最近 1000 个延迟记录
        let mut latencies = self.requests_latency_ms.lock().unwrap();
        if latencies.len() > 1000 {
            latencies.remove(0);
        }
    }
    
    pub fn increment_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn decrement_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }
    
    pub fn get_avg_latency(&self) -> f64 {
        let latencies = self.requests_latency_ms.lock().unwrap();
        if latencies.is_empty() {
            return 0.0;
        }
        
        let sum: u64 = latencies.iter().sum();
        sum as f64 / latencies.len() as f64
    }
    
    pub fn get_p99_latency(&self) -> u64 {
        let mut latencies = self.requests_latency_ms.lock().unwrap().clone();
        if latencies.is_empty() {
            return 0;
        }
        
        latencies.sort();
        let idx = (latencies.len() as f64 * 0.99) as usize;
        latencies[idx.min(latencies.len() - 1)]
    }
}

impl Default for AppMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new();
        
        collector.register_counter("requests_total");
        collector.register_gauge("cpu_usage");
        
        collector.increment_counter("requests_total", 1);
        collector.increment_counter("requests_total", 1);
        collector.set_gauge("cpu_usage", 75.5);
        
        assert_eq!(collector.get_counter("requests_total"), 2);
        assert_eq!(collector.get_gauge("cpu_usage"), 75.5);
    }
    
    #[test]
    fn test_prometheus_export() {
        let mut collector = MetricsCollector::new();
        
        collector.register_counter("http_requests_total");
        collector.increment_counter("http_requests_total", 100);
        
        let output = collector.export_prometheus();
        
        assert!(output.contains("http_requests_total"));
        assert!(output.contains("100"));
    }
    
    #[test]
    fn test_app_metrics() {
        let metrics = AppMetrics::new();
        
        metrics.record_request(true, 50);
        metrics.record_request(true, 100);
        metrics.record_request(false, 200);
        
        assert_eq!(metrics.requests_total.load(Ordering::SeqCst), 3);
        assert_eq!(metrics.requests_success.load(Ordering::SeqCst), 2);
        assert_eq!(metrics.requests_failed.load(Ordering::SeqCst), 1);
        
        let avg = metrics.get_avg_latency();
        assert_eq!(avg, 116.66666666666667);
    }
    
    #[test]
    fn test_app_metrics_p99() {
        let metrics = AppMetrics::new();
        
        // 添加 100 个延迟记录
        for i in 0..100 {
            metrics.record_request(true, i + 1);
        }
        
        let p99 = metrics.get_p99_latency();
        assert!(p99 >= 99);
    }
    
    #[test]
    fn test_active_connections() {
        let metrics = AppMetrics::new();
        
        metrics.increment_connections();
        metrics.increment_connections();
        assert_eq!(metrics.active_connections.load(Ordering::SeqCst), 2);
        
        metrics.decrement_connections();
        assert_eq!(metrics.active_connections.load(Ordering::SeqCst), 1);
    }
}
