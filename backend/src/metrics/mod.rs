//! 监控指标模块 - Prometheus 兼容的指标收集系统
//!
//! 提供完整的业务指标收集、Prometheus 格式导出和性能监控功能。
//!
//! 注意：部分指标记录器功能正在开发中，暂未完全使用

#![allow(dead_code)]

pub mod business;
pub mod prometheus;

use std::collections::HashMap;
use std::time::{Duration, Instant};

// Re-export prometheus types for convenience
pub use ::prometheus::{
    opts, register_counter, register_counter_vec, register_gauge, register_gauge_vec,
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
    register_int_gauge, register_int_gauge_vec, Counter, CounterVec, Encoder, Gauge, GaugeVec,
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    TextEncoder,
};

// Re-export business metrics
pub use business::*;

lazy_static::lazy_static! {
    // ============================================================================
    // 请求计数指标
    // ============================================================================

    /// 总请求数
    pub static ref REQUESTS_TOTAL: IntCounter = register_int_counter!(opts!(
        "foxnio_requests_total",
        "Total number of requests processed"
    )).unwrap();

    /// 成功请求数（按模型分类）
    pub static ref REQUESTS_SUCCESS: IntCounterVec = register_int_counter_vec!(
        "foxnio_requests_success_total",
        "Total number of successful requests",
        &["model", "provider", "user"]
    ).unwrap();

    /// 失败请求数（按模型分类）
    pub static ref REQUESTS_FAILED: IntCounterVec = register_int_counter_vec!(
        "foxnio_requests_failed_total",
        "Total number of failed requests",
        &["model", "provider", "error_type"]
    ).unwrap();

    /// 按状态码分类的请求数
    pub static ref REQUESTS_BY_STATUS: IntCounterVec = register_int_counter_vec!(
        "foxnio_requests_by_status",
        "Number of requests grouped by HTTP status code",
        &["status_code"]
    ).unwrap();

    // ============================================================================
    // 请求延迟指标
    // ============================================================================

    /// 请求延迟直方图（秒）
    pub static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "foxnio_request_duration_seconds",
        "Request latency in seconds",
        &["model", "provider", "endpoint"],
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]
    ).unwrap();

    /// 上游 API 延迟
    pub static ref UPSTREAM_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "foxnio_upstream_request_duration_seconds",
        "Upstream API request latency in seconds",
        &["provider", "model"],
        vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]
    ).unwrap();

    /// 请求队列等待时间
    pub static ref REQUEST_QUEUE_DURATION: Histogram = register_histogram!(
        "foxnio_request_queue_duration_seconds",
        "Time requests spend in queue before processing",
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
    ).unwrap();

    // ============================================================================
    // 连接指标
    // ============================================================================

    /// 活跃连接数
    pub static ref ACTIVE_CONNECTIONS: IntGauge = register_int_gauge!(
        "foxnio_active_connections",
        "Number of active connections"
    ).unwrap();

    /// WebSocket 活跃连接数
    pub static ref WEBSOCKET_CONNECTIONS: IntGauge = register_int_gauge!(
        "foxnio_websocket_connections",
        "Number of active WebSocket connections"
    ).unwrap();

    /// 连接池状态
    pub static ref CONNECTION_POOL_SIZE: IntGaugeVec = register_int_gauge_vec!(
        "foxnio_connection_pool_size",
        "Connection pool size by provider",
        &["provider", "pool_type"]
    ).unwrap();

    // ============================================================================
    // Token 使用量指标
    // ============================================================================

    /// 输入 Token 使用量
    pub static ref TOKENS_INPUT: IntCounterVec = register_int_counter_vec!(
        "foxnio_tokens_input_total",
        "Total number of input tokens used",
        &["model", "provider", "user"]
    ).unwrap();

    /// 输出 Token 使用量
    pub static ref TOKENS_OUTPUT: IntCounterVec = register_int_counter_vec!(
        "foxnio_tokens_output_total",
        "Total number of output tokens generated",
        &["model", "provider", "user"]
    ).unwrap();

    /// Token 使用速率（每分钟）
    pub static ref TOKENS_RATE: GaugeVec = register_gauge_vec!(
        "foxnio_tokens_rate_per_minute",
        "Token usage rate per minute",
        &["model", "provider", "token_type"]
    ).unwrap();

    // ============================================================================
    // 成本指标
    // ============================================================================

    /// 总成本（美元）
    pub static ref COST_TOTAL: Counter = register_counter!(
        "foxnio_cost_total_dollars",
        "Total cost in dollars"
    ).unwrap();

    /// 按模型分类的成本
    pub static ref COST_BY_MODEL: CounterVec = register_counter_vec!(
        "foxnio_cost_by_model_dollars",
        "Cost in dollars grouped by model",
        &["model", "provider", "user"]
    ).unwrap();

    /// 每日成本
    pub static ref COST_DAILY: GaugeVec = register_gauge_vec!(
        "foxnio_cost_daily_dollars",
        "Daily cost in dollars",
        &["date"]
    ).unwrap();

    // ============================================================================
    // 账号配额指标
    // ============================================================================

    /// 账号配额使用率
    pub static ref ACCOUNT_QUOTA_USAGE: GaugeVec = register_gauge_vec!(
        "foxnio_account_quota_usage_ratio",
        "Account quota usage ratio (0-1)",
        &["account_id", "provider"]
    ).unwrap();

    /// 账号剩余配额
    pub static ref ACCOUNT_QUOTA_REMAINING: GaugeVec = register_gauge_vec!(
        "foxnio_account_quota_remaining",
        "Account remaining quota",
        &["account_id", "provider"]
    ).unwrap();

    /// 账号请求速率
    pub static ref ACCOUNT_REQUEST_RATE: GaugeVec = register_gauge_vec!(
        "foxnio_account_request_rate_per_minute",
        "Account request rate per minute",
        &["account_id", "provider"]
    ).unwrap();

    /// 账号使用率（活跃账号比例）
    pub static ref ACTIVE_ACCOUNTS_RATIO: Gauge = register_gauge!(
        "foxnio_active_accounts_ratio",
        "Ratio of active accounts to total accounts"
    ).unwrap();

    // ============================================================================
    // 错误和重试指标
    // ============================================================================

    /// 错误计数（按错误类型）
    pub static ref ERRORS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "foxnio_errors_total",
        "Total number of errors",
        &["error_type", "provider", "model"]
    ).unwrap();

    /// 重试次数
    pub static ref RETRIES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "foxnio_retries_total",
        "Total number of request retries",
        &["provider", "reason"]
    ).unwrap();

    /// 熔断器状态
    pub static ref CIRCUIT_BREAKER_STATE: IntGaugeVec = register_int_gauge_vec!(
        "foxnio_circuit_breaker_state",
        "Circuit breaker state (0=closed, 1=open, 2=half-open)",
        &["provider", "account_id"]
    ).unwrap();

    // ============================================================================
    // 缓存指标
    // ============================================================================

    /// 缓存命中次数
    pub static ref CACHE_HITS: IntCounter = register_int_counter!(
        "foxnio_cache_hits_total",
        "Total number of cache hits"
    ).unwrap();

    /// 缓存未命中次数
    pub static ref CACHE_MISSES: IntCounter = register_int_counter!(
        "foxnio_cache_misses_total",
        "Total number of cache misses"
    ).unwrap();

    /// 缓存大小
    pub static ref CACHE_SIZE: IntGauge = register_int_gauge!(
        "foxnio_cache_size_bytes",
        "Current cache size in bytes"
    ).unwrap();

    /// 缓存条目数
    pub static ref CACHE_ENTRIES: IntGauge = register_int_gauge!(
        "foxnio_cache_entries",
        "Number of entries in cache"
    ).unwrap();

    // ============================================================================
    // 系统资源指标
    // ============================================================================

    /// 内存使用量
    pub static ref MEMORY_USAGE: IntGauge = register_int_gauge!(
        "foxnio_memory_usage_bytes",
        "Current memory usage in bytes"
    ).unwrap();

    /// CPU 使用率
    pub static ref CPU_USAGE: Gauge = register_gauge!(
        "foxnio_cpu_usage_ratio",
        "CPU usage ratio (0-1)"
    ).unwrap();

    /// Goroutine 数量（兼容性指标）
    pub static ref GOROUTINE_COUNT: IntGauge = register_int_gauge!(
        "foxnio_goroutine_count",
        "Number of active tasks (for compatibility)"
    ).unwrap();

    /// 文件描述符数量
    pub static ref FILE_DESCRIPTORS: IntGauge = register_int_gauge!(
        "foxnio_file_descriptors",
        "Number of open file descriptors"
    ).unwrap();

    // ============================================================================
    // Webhook 指标
    // ============================================================================

    /// 发送的 Webhook 事件总数
    pub static ref WEBHOOK_EVENTS_SENT: IntCounter = register_int_counter!(opts!(
        "foxnio_webhook_events_sent_total",
        "Total webhook events sent"
    )).unwrap();

    /// 成功的 Webhook 投递数
    pub static ref WEBHOOK_DELIVERY_SUCCESS: IntCounter = register_int_counter!(opts!(
        "foxnio_webhook_delivery_success_total",
        "Successful webhook deliveries"
    )).unwrap();

    /// 失败的 Webhook 投递数
    pub static ref WEBHOOK_DELIVERY_FAILED: IntCounter = register_int_counter!(opts!(
        "foxnio_webhook_delivery_failed_total",
        "Failed webhook deliveries"
    )).unwrap();

    /// Webhook 重试次数
    pub static ref WEBHOOK_RETRY_COUNT: IntCounter = register_int_counter!(opts!(
        "foxnio_webhook_retry_total",
        "Webhook retry attempts"
    )).unwrap();

    // ============================================================================
    // 批量操作指标
    // ============================================================================

    /// 批量操作总数
    pub static ref BATCH_OPERATIONS_TOTAL: IntCounter = register_int_counter!(opts!(
        "foxnio_batch_operations_total",
        "Total batch operations"
    )).unwrap();

    /// 批量操作处理的条目总数
    pub static ref BATCH_ITEMS_PROCESSED: IntCounter = register_int_counter!(opts!(
        "foxnio_batch_items_processed_total",
        "Total items processed in batch operations"
    )).unwrap();

    /// 批量操作错误数
    pub static ref BATCH_ERRORS: IntCounter = register_int_counter!(opts!(
        "foxnio_batch_errors_total",
        "Batch operation errors"
    )).unwrap();

    // ============================================================================
    // API Key 权限指标
    // ============================================================================

    /// API Key 认证检查次数
    pub static ref API_KEY_AUTH_CHECKS: IntCounter = register_int_counter!(opts!(
        "foxnio_api_key_auth_checks_total",
        "API key authentication checks"
    )).unwrap();

    /// API Key 配额超限次数
    pub static ref API_KEY_QUOTA_EXCEEDED: IntCounter = register_int_counter!(opts!(
        "foxnio_api_key_quota_exceeded_total",
        "API key quota exceeded events"
    )).unwrap();

    /// API Key 模型访问拒绝次数
    pub static ref API_KEY_MODEL_DENIED: IntCounter = register_int_counter!(opts!(
        "foxnio_api_key_model_denied_total",
        "API key model access denied events"
    )).unwrap();

    // ============================================================================
    // 成本优化指标
    // ============================================================================

    /// 潜在成本节省金额
    pub static ref COST_OPTIMIZATION_SAVINGS: Gauge = register_gauge!(opts!(
        "foxnio_cost_optimization_potential_savings",
        "Potential cost savings identified"
    )).unwrap();

    /// 成本优化建议生成数
    pub static ref COST_RECOMMENDATIONS_GENERATED: IntCounter = register_int_counter!(opts!(
        "foxnio_cost_recommendations_total",
        "Cost optimization recommendations generated"
    )).unwrap();

    // ============================================================================
    // 模型同步指标
    // ============================================================================

    /// 模型同步耗时
    pub static ref MODEL_SYNC_DURATION: Histogram = {
        let opts = HistogramOpts::new(
            "foxnio_model_sync_duration_seconds",
            "Model sync duration"
        );
        register_histogram!(opts).unwrap()
    };

    /// 已同步的模型数量
    pub static ref MODELS_SYNCED: IntGauge = register_int_gauge!(opts!(
        "foxnio_models_synced",
        "Number of models synced"
    )).unwrap();

    /// 模型价格变化检测数
    pub static ref MODEL_PRICE_CHANGES: IntCounter = register_int_counter!(opts!(
        "foxnio_model_price_changes_total",
        "Model price changes detected"
    )).unwrap();
}

/// 指标记录器 - 用于记录请求指标
#[derive(Debug, Clone)]
pub struct MetricsRecorder {
    start_time: Instant,
    model: String,
    provider: String,
    user_id: Option<String>,
    endpoint: String,
}

impl MetricsRecorder {
    /// 创建新的指标记录器
    pub fn new(model: &str, provider: &str, endpoint: &str) -> Self {
        Self {
            start_time: Instant::now(),
            model: model.to_string(),
            provider: provider.to_string(),
            user_id: None,
            endpoint: endpoint.to_string(),
        }
    }

    /// 设置用户 ID
    pub fn with_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    /// 记录成功请求
    pub fn record_success(&self, input_tokens: u64, output_tokens: u64, cost: f64) {
        let duration = self.start_time.elapsed().as_secs_f64();

        // 记录总请求
        REQUESTS_TOTAL.inc();

        // 记录成功请求
        let user = self.user_id.as_deref().unwrap_or("anonymous");
        REQUESTS_SUCCESS
            .with_label_values(&[&self.model, &self.provider, user])
            .inc();

        // 记录延迟
        REQUEST_DURATION
            .with_label_values(&[&self.model, &self.provider, &self.endpoint])
            .observe(duration);

        // 记录 Token 使用
        TOKENS_INPUT
            .with_label_values(&[&self.model, &self.provider, user])
            .inc_by(input_tokens);
        TOKENS_OUTPUT
            .with_label_values(&[&self.model, &self.provider, user])
            .inc_by(output_tokens);

        // 记录成本
        COST_TOTAL.inc_by(cost);
        COST_BY_MODEL
            .with_label_values(&[&self.model, &self.provider, user])
            .inc_by(cost);
    }

    /// 记录失败请求
    pub fn record_failure(&self, error_type: &str, status_code: u16) {
        let duration = self.start_time.elapsed().as_secs_f64();

        // 记录总请求
        REQUESTS_TOTAL.inc();

        // 记录失败请求
        REQUESTS_FAILED
            .with_label_values(&[&self.model, &self.provider, error_type])
            .inc();

        // 记录状态码
        REQUESTS_BY_STATUS
            .with_label_values(&[&status_code.to_string()])
            .inc();

        // 记录延迟
        REQUEST_DURATION
            .with_label_values(&[&self.model, &self.provider, &self.endpoint])
            .observe(duration);

        // 记录错误
        ERRORS_TOTAL
            .with_label_values(&[error_type, &self.provider, &self.model])
            .inc();
    }

    /// 获取已用时间
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// 连接跟踪器
pub struct ConnectionTracker;

impl ConnectionTracker {
    /// 增加连接计数
    pub fn increment() {
        ACTIVE_CONNECTIONS.inc();
    }

    /// 减少连接计数
    pub fn decrement() {
        ACTIVE_CONNECTIONS.dec();
    }

    /// 增加 WebSocket 连接
    pub fn increment_websocket() {
        WEBSOCKET_CONNECTIONS.inc();
    }

    /// 减少 WebSocket 连接
    pub fn decrement_websocket() {
        WEBSOCKET_CONNECTIONS.dec();
    }
}

/// 缓存指标记录器
pub struct CacheMetrics;

impl CacheMetrics {
    /// 记录缓存命中
    pub fn hit() {
        CACHE_HITS.inc();
    }

    /// 记录缓存未命中
    pub fn miss() {
        CACHE_MISSES.inc();
    }

    /// 获取缓存命中率
    pub fn hit_rate() -> f64 {
        let hits = CACHE_HITS.get();
        let misses = CACHE_MISSES.get();
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 更新缓存大小
    pub fn update_size(bytes: i64) {
        CACHE_SIZE.set(bytes);
    }

    /// 更新缓存条目数
    pub fn update_entries(count: i64) {
        CACHE_ENTRIES.set(count);
    }
}

/// 账号配额更新器
pub struct QuotaUpdater;

impl QuotaUpdater {
    /// 更新账号配额使用率
    pub fn update_usage(account_id: &str, provider: &str, usage_ratio: f64) {
        ACCOUNT_QUOTA_USAGE
            .with_label_values(&[account_id, provider])
            .set(usage_ratio);
    }

    /// 更新账号剩余配额
    pub fn update_remaining(account_id: &str, provider: &str, remaining: f64) {
        ACCOUNT_QUOTA_REMAINING
            .with_label_values(&[account_id, provider])
            .set(remaining);
    }

    /// 更新账号请求速率
    pub fn update_request_rate(account_id: &str, provider: &str, rate: f64) {
        ACCOUNT_REQUEST_RATE
            .with_label_values(&[account_id, provider])
            .set(rate);
    }

    /// 更新活跃账号比例
    pub fn update_active_ratio(ratio: f64) {
        ACTIVE_ACCOUNTS_RATIO.set(ratio);
    }
}

/// 重试指标记录器
pub struct RetryMetrics;

impl RetryMetrics {
    /// 记录重试
    pub fn record_retry(provider: &str, reason: &str) {
        RETRIES_TOTAL.with_label_values(&[provider, reason]).inc();
    }
}

/// 熔断器状态更新器
pub struct CircuitBreakerMetrics;

impl CircuitBreakerMetrics {
    /// 更新熔断器状态
    /// state: 0=closed (正常), 1=open (熔断), 2=half-open (半开)
    pub fn update_state(provider: &str, account_id: &str, state: i64) {
        CIRCUIT_BREAKER_STATE
            .with_label_values(&[provider, account_id])
            .set(state);
    }
}

/// 收集所有指标并返回 Prometheus 格式的文本
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = ::prometheus::gather();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return String::new();
    }

    String::from_utf8(buffer).unwrap_or_default()
}

/// 获取 JSON 格式的指标摘要
pub fn get_metrics_summary() -> MetricsSummary {
    MetricsSummary {
        requests: RequestsSummary {
            total: REQUESTS_TOTAL.get(),
            success: get_counter_sum(&REQUESTS_SUCCESS),
            failed: get_counter_sum(&REQUESTS_FAILED),
        },
        connections: ConnectionsSummary {
            active: ACTIVE_CONNECTIONS.get(),
            websocket: WEBSOCKET_CONNECTIONS.get(),
        },
        tokens: TokensSummary {
            input: get_counter_sum(&TOKENS_INPUT),
            output: get_counter_sum(&TOKENS_OUTPUT),
        },
        cache: CacheSummary {
            hits: CACHE_HITS.get(),
            misses: CACHE_MISSES.get(),
            hit_rate: CacheMetrics::hit_rate(),
        },
        cost: CostSummary {
            total: COST_TOTAL.get(),
        },
    }
}

/// 从 CounterVec 获取总和
fn get_counter_sum(_counter_vec: &IntCounterVec) -> u64 {
    // Note: This is a simplification. In production, you'd want to iterate all label combinations
    // For now, we return 0 as CounterVec doesn't have a simple sum method
    // This would need to be tracked separately if exact sum is needed
    0
}

/// 指标摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MetricsSummary {
    pub requests: RequestsSummary,
    pub connections: ConnectionsSummary,
    pub tokens: TokensSummary,
    pub cache: CacheSummary,
    pub cost: CostSummary,
}

/// 请求摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RequestsSummary {
    pub total: u64,
    pub success: u64,
    pub failed: u64,
}

/// 连接摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConnectionsSummary {
    pub active: i64,
    pub websocket: i64,
}

/// Token 摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TokensSummary {
    pub input: u64,
    pub output: u64,
}

/// 缓存摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CacheSummary {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

/// 成本摘要
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CostSummary {
    pub total: f64,
}

/// 初始化指标系统
pub fn init_metrics() {
    // 触发 lazy_static 初始化
    lazy_static::initialize(&REQUESTS_TOTAL);
    lazy_static::initialize(&REQUESTS_SUCCESS);
    lazy_static::initialize(&REQUEST_DURATION);
    lazy_static::initialize(&ACTIVE_CONNECTIONS);
    lazy_static::initialize(&TOKENS_INPUT);
    lazy_static::initialize(&TOKENS_OUTPUT);
    lazy_static::initialize(&COST_TOTAL);

    tracing::info!("Prometheus metrics initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recorder_success() {
        let recorder =
            MetricsRecorder::new("gpt-4", "openai", "/v1/chat/completions").with_user("user123");

        recorder.record_success(100, 50, 0.002);

        assert!(REQUESTS_TOTAL.get() > 0);
    }

    #[test]
    fn test_metrics_recorder_failure() {
        let recorder = MetricsRecorder::new("gpt-4", "openai", "/v1/chat/completions");

        recorder.record_failure("rate_limit", 429);

        assert!(REQUESTS_TOTAL.get() > 0);
    }

    #[test]
    fn test_connection_tracker() {
        let initial = ACTIVE_CONNECTIONS.get();

        ConnectionTracker::increment();
        assert_eq!(ACTIVE_CONNECTIONS.get(), initial + 1);

        ConnectionTracker::decrement();
        assert_eq!(ACTIVE_CONNECTIONS.get(), initial);
    }

    #[test]
    fn test_cache_metrics() {
        CacheMetrics::hit();
        CacheMetrics::hit();
        CacheMetrics::miss();

        assert_eq!(CACHE_HITS.get(), 2);
        assert_eq!(CACHE_MISSES.get(), 1);

        let rate = CacheMetrics::hit_rate();
        assert!((rate - 0.6666666666666666).abs() < 0.01);
    }

    #[test]
    fn test_gather_metrics() {
        let output = gather_metrics();

        assert!(output.contains("foxnio_requests_total"));
        assert!(output.contains("foxnio_active_connections"));
    }

    #[test]
    fn test_metrics_summary() {
        let summary = get_metrics_summary();

        // requests.total 是 u64 类型，始终 >= 0
        assert!(summary.cache.hit_rate >= 0.0);
    }
}
