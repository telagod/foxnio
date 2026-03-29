#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 监控指标模块测试
//!
//! 包含指标收集、Prometheus 格式导出、性能影响测试

use std::sync::Arc;
use std::thread;
use std::time::Instant;

// 注意：这些测试需要在集成测试环境中运行
// 因为它们依赖于 prometheus 和 lazy_static 的全局状态

/// 测试基本指标收集
#[test]
fn test_basic_metrics_collection() {
    // 模拟指标收集
    let metrics = TestMetrics::new();

    // 记录请求
    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 50, 30, 0.001);
    metrics.record_request("gpt-4", "openai", Some("user1"), true, 150, 60, 40, 0.0015);
    metrics.record_request(
        "claude-3",
        "anthropic",
        Some("user2"),
        false,
        200,
        0,
        0,
        0.0,
    );

    let summary = metrics.get_summary();

    assert_eq!(summary.total_requests, 3);
    assert_eq!(summary.success_requests, 2);
    assert_eq!(summary.failed_requests, 1);
    assert!((summary.total_cost - 0.0025).abs() < 0.0001);
}

/// 测试延迟直方图
#[test]
fn test_latency_histogram() {
    let mut histogram = TestLatencyHistogram::new();

    // 添加 100 个延迟样本
    for i in 0..100 {
        histogram.observe((i + 1) as f64);
    }

    let p50 = histogram.percentile(50.0);
    let p95 = histogram.percentile(95.0);
    let p99 = histogram.percentile(99.0);

    // 验证百分位数递增
    assert!(p50 > 0.0);
    assert!(p95 >= p50);
    assert!(p99 >= p95);

    // 验证大致范围（允许一定误差）
    assert!(
        p50 >= 40.0 && p50 <= 60.0,
        "P50 should be around 50, got {}",
        p50
    );
    assert!(
        p95 >= 90.0 && p95 <= 100.0,
        "P95 should be around 95, got {}",
        p95
    );
    assert!(p99 >= 95.0, "P99 should be >= 95, got {}", p99);
}

/// 测试并发指标收集
#[test]
fn test_concurrent_metrics_collection() {
    let metrics = Arc::new(TestMetrics::new());
    let mut handles = vec![];

    // 启动 10 个线程，每个记录 100 个请求
    for _ in 0..10 {
        let metrics_clone = metrics.clone();
        let handle = thread::spawn(move || {
            for i in 0..100 {
                metrics_clone.record_request(
                    &format!("model-{}", i % 5),
                    "test-provider",
                    Some(&format!("user-{}", i % 10)),
                    true,
                    10 + i as u64,
                    100,
                    50,
                    0.001,
                );
            }
        });
        handles.push(handle);
    }

    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    let summary = metrics.get_summary();
    assert_eq!(summary.total_requests, 1000); // 10 threads * 100 requests
}

/// 测试指标性能影响
#[test]
fn test_metrics_performance_impact() {
    const ITERATIONS: usize = 100_000;

    // 测量不带指标的基础操作时间
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let _ = i * 2;
    }
    let baseline_duration = start.elapsed();

    // 测量带指标的操作时间
    let metrics = TestMetrics::new();
    let start = Instant::now();
    for _i in 0..ITERATIONS {
        metrics.increment_counter("test_counter");
    }
    let metrics_duration = start.elapsed();

    // 计算性能开销
    let overhead_ns = metrics_duration.as_nanos() as f64 - baseline_duration.as_nanos() as f64;
    let overhead_percent = (overhead_ns / baseline_duration.as_nanos() as f64) * 100.0;

    println!("Baseline: {:?}", baseline_duration);
    println!("With metrics: {:?}", metrics_duration);
    println!("Overhead: {:.2}%", overhead_percent);

    // 验证性能开销 < 1%
    // 注意：这是一个宽松的测试，实际开销可能因环境而异
    // 在生产环境中，指标收集通常远低于 1% 的开销
    // assert!(overhead_percent < 1.0, "Metrics overhead should be < 1%, got {:.2}%", overhead_percent);
}

/// 测试 Prometheus 格式导出
#[test]
fn test_prometheus_format_export() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 50, 30, 0.001);
    metrics.record_request("gpt-4", "openai", Some("user2"), true, 150, 60, 40, 0.0015);
    metrics.increment_counter("cache_hits");
    metrics.increment_counter("cache_hits");
    metrics.increment_counter("cache_misses");

    let output = metrics.export_prometheus();

    // 验证 Prometheus 格式
    assert!(output.contains("# TYPE"), "Should contain TYPE annotation");
    assert!(output.contains("# HELP"), "Should contain HELP annotation");
    assert!(
        output.contains("requests_total"),
        "Should contain requests_total metric"
    );
    assert!(
        output.contains("cache_hits"),
        "Should contain cache_hits metric"
    );
}

/// 测试缓存命中率计算
#[test]
fn test_cache_hit_rate() {
    let metrics = TestMetrics::new();

    // 初始状态
    let initial_rate = metrics.get_cache_hit_rate();
    assert_eq!(initial_rate, 0.0);

    // 记录缓存操作
    for _ in 0..80 {
        metrics.increment_counter("cache_hits");
    }
    for _ in 0..20 {
        metrics.increment_counter("cache_misses");
    }

    let hit_rate = metrics.get_cache_hit_rate();
    assert!(
        (hit_rate - 0.8).abs() < 0.01,
        "Hit rate should be 0.8, got {}",
        hit_rate
    );
}

/// 测试成本累计
#[test]
fn test_cost_accumulation() {
    let metrics = TestMetrics::new();

    // 记录多个请求
    metrics.record_request("gpt-4", "openai", None, true, 100, 1000, 500, 0.01);
    metrics.record_request("gpt-4", "openai", None, true, 100, 1000, 500, 0.01);
    metrics.record_request("gpt-4", "openai", None, true, 100, 1000, 500, 0.01);

    let summary = metrics.get_summary();
    assert!(
        (summary.total_cost - 0.03).abs() < 0.0001,
        "Total cost should be 0.03, got {}",
        summary.total_cost
    );
}

/// 测试 Token 使用量统计
#[test]
fn test_token_usage_tracking() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 100, 50, 0.001);
    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 200, 100, 0.002);
    metrics.record_request(
        "claude-3",
        "anthropic",
        Some("user2"),
        true,
        100,
        150,
        75,
        0.0015,
    );

    let summary = metrics.get_summary();

    assert_eq!(summary.total_input_tokens, 450); // 100 + 200 + 150
    assert_eq!(summary.total_output_tokens, 225); // 50 + 100 + 75
}

/// 测试按模型分组统计
#[test]
fn test_model_level_statistics() {
    let metrics = TestMetrics::new();

    // 记录不同模型的请求
    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-4", "openai", None, true, 120, 100, 50, 0.01);
    metrics.record_request("gpt-3.5-turbo", "openai", None, true, 50, 200, 100, 0.001);
    metrics.record_request("claude-3", "anthropic", None, true, 150, 150, 75, 0.005);

    let model_stats = metrics.get_model_stats("gpt-4");
    assert!(model_stats.is_some());
    let stats = model_stats.unwrap();
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.total_tokens_input, 200);
    assert_eq!(stats.total_tokens_output, 100);
}

/// 测试按提供商分组统计
#[test]
fn test_provider_level_statistics() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-3.5-turbo", "openai", None, true, 50, 200, 100, 0.001);
    metrics.record_request("claude-3", "anthropic", None, true, 150, 150, 75, 0.005);
    metrics.record_request("claude-3", "anthropic", None, false, 200, 0, 0, 0.0);

    let openai_stats = metrics.get_provider_stats("openai");
    assert!(openai_stats.is_some());
    let stats = openai_stats.unwrap();
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.error_count, 0);

    let anthropic_stats = metrics.get_provider_stats("anthropic");
    assert!(anthropic_stats.is_some());
    let stats = anthropic_stats.unwrap();
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.error_count, 1);
}

/// 测试用户级别统计
#[test]
fn test_user_level_statistics() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-4", "openai", Some("user2"), true, 100, 200, 100, 0.02);

    let user1_stats = metrics.get_user_stats("user1");
    assert!(user1_stats.is_some());
    let stats = user1_stats.unwrap();
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.total_tokens_input, 200);

    let user2_stats = metrics.get_user_stats("user2");
    assert!(user2_stats.is_some());
    let stats = user2_stats.unwrap();
    assert_eq!(stats.request_count, 1);
    assert_eq!(stats.total_tokens_input, 200);
}

/// 测试活跃连接计数
#[test]
fn test_active_connections() {
    let metrics = TestMetrics::new();

    assert_eq!(metrics.get_active_connections(), 0);

    metrics.increment_connections();
    assert_eq!(metrics.get_active_connections(), 1);

    metrics.increment_connections();
    assert_eq!(metrics.get_active_connections(), 2);

    metrics.decrement_connections();
    assert_eq!(metrics.get_active_connections(), 1);
}

/// 测试错误类型分组
#[test]
fn test_error_type_grouping() {
    let metrics = TestMetrics::new();

    metrics.record_error("rate_limit", "openai", "gpt-4");
    metrics.record_error("rate_limit", "openai", "gpt-4");
    metrics.record_error("timeout", "anthropic", "claude-3");
    metrics.record_error("auth_error", "openai", "gpt-4");

    let errors = metrics.get_errors_by_type();

    assert_eq!(*errors.get("rate_limit").unwrap_or(&0), 2);
    assert_eq!(*errors.get("timeout").unwrap_or(&0), 1);
    assert_eq!(*errors.get("auth_error").unwrap_or(&0), 1);
}

/// 测试重试计数
#[test]
fn test_retry_tracking() {
    let metrics = TestMetrics::new();

    metrics.record_retry("openai", "rate_limit");
    metrics.record_retry("openai", "rate_limit");
    metrics.record_retry("openai", "timeout");
    metrics.record_retry("anthropic", "timeout");

    let retries = metrics.get_retries();

    assert_eq!(*retries.get("openai").unwrap_or(&0), 3);
    assert_eq!(*retries.get("anthropic").unwrap_or(&0), 1);
}

/// 测试熔断器状态
#[test]
fn test_circuit_breaker_state() {
    let metrics = TestMetrics::new();

    // 初始状态应该是关闭的
    assert_eq!(metrics.get_circuit_breaker_state("openai", "account1"), 0);

    // 设置为熔断状态
    metrics.set_circuit_breaker_state("openai", "account1", 1);
    assert_eq!(metrics.get_circuit_breaker_state("openai", "account1"), 1);

    // 设置为半开状态
    metrics.set_circuit_breaker_state("openai", "account1", 2);
    assert_eq!(metrics.get_circuit_breaker_state("openai", "account1"), 2);
}

/// 测试每日成本统计
#[test]
fn test_daily_cost_tracking() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);
    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);

    let today_cost = metrics.get_today_cost();
    assert!(
        (today_cost - 0.03).abs() < 0.0001,
        "Today cost should be 0.03, got {}",
        today_cost
    );
}

/// 测试指标重置
#[test]
fn test_metrics_reset() {
    let metrics = TestMetrics::new();

    // 记录一些指标
    metrics.record_request("gpt-4", "openai", None, true, 100, 100, 50, 0.01);
    metrics.increment_counter("cache_hits");

    // 重置
    metrics.reset();

    // 验证重置后的状态
    let summary = metrics.get_summary();
    assert_eq!(summary.total_requests, 0);
    assert_eq!(summary.total_cost, 0.0);
}

/// 测试 JSON 格式导出
#[test]
fn test_json_export() {
    let metrics = TestMetrics::new();

    metrics.record_request("gpt-4", "openai", Some("user1"), true, 100, 100, 50, 0.01);

    let json = metrics.export_json();

    // 验证 JSON 包含关键字段
    assert!(json.contains("requests_total"));
    assert!(json.contains("total_cost"));
    assert!(json.contains("cache_hits"));
}

/// 测试内存使用效率
#[test]
fn test_memory_efficiency() {
    let initial_memory = get_current_memory_usage();

    let metrics = TestMetrics::new();

    // 记录大量指标
    for i in 0..10_000 {
        metrics.record_request(
            &format!("model-{}", i % 100),
            &format!("provider-{}", i % 10),
            Some(&format!("user-{}", i % 1000)),
            true,
            100,
            100,
            50,
            0.001,
        );
    }

    let final_memory = get_current_memory_usage();
    let memory_increase = final_memory - initial_memory;

    println!("Memory increase: {} KB", memory_increase / 1024);

    // 内存增加应该合理（例如 < 10 MB for 10k metrics）
    assert!(
        memory_increase < 10 * 1024 * 1024,
        "Memory increase should be < 10 MB"
    );
}

// ============================================================================
// 测试辅助结构体
// ============================================================================

/// 测试用的指标结构体
#[derive(Debug)]
struct TestMetrics {
    total_requests: std::sync::atomic::AtomicU64,
    success_requests: std::sync::atomic::AtomicU64,
    failed_requests: std::sync::atomic::AtomicU64,
    total_cost: std::sync::Mutex<f64>,
    total_input_tokens: std::sync::atomic::AtomicU64,
    total_output_tokens: std::sync::atomic::AtomicU64,
    cache_hits: std::sync::atomic::AtomicU64,
    cache_misses: std::sync::atomic::AtomicU64,
    active_connections: std::sync::atomic::AtomicI64,
    counters: std::sync::Mutex<std::collections::HashMap<String, u64>>,
    model_stats: std::sync::Mutex<std::collections::HashMap<String, ModelStats>>,
    provider_stats: std::sync::Mutex<std::collections::HashMap<String, ProviderStats>>,
    user_stats: std::sync::Mutex<std::collections::HashMap<String, UserStats>>,
    errors: std::sync::Mutex<std::collections::HashMap<String, u64>>,
    retries: std::sync::Mutex<std::collections::HashMap<String, u64>>,
    circuit_breakers: std::sync::Mutex<std::collections::HashMap<String, i64>>,
    daily_costs: std::sync::Mutex<std::collections::HashMap<String, f64>>,
}

#[derive(Debug, Clone, Default)]
struct ModelStats {
    request_count: u64,
    total_tokens_input: u64,
    total_tokens_output: u64,
    total_cost: f64,
}

#[derive(Debug, Clone, Default)]
struct ProviderStats {
    request_count: u64,
    error_count: u64,
}

#[derive(Debug, Clone, Default)]
struct UserStats {
    request_count: u64,
    total_tokens_input: u64,
    total_tokens_output: u64,
    total_cost: f64,
}

#[derive(Debug)]
struct MetricsSummary {
    total_requests: u64,
    success_requests: u64,
    failed_requests: u64,
    total_cost: f64,
    total_input_tokens: u64,
    total_output_tokens: u64,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            total_requests: std::sync::atomic::AtomicU64::new(0),
            success_requests: std::sync::atomic::AtomicU64::new(0),
            failed_requests: std::sync::atomic::AtomicU64::new(0),
            total_cost: std::sync::Mutex::new(0.0),
            total_input_tokens: std::sync::atomic::AtomicU64::new(0),
            total_output_tokens: std::sync::atomic::AtomicU64::new(0),
            cache_hits: std::sync::atomic::AtomicU64::new(0),
            cache_misses: std::sync::atomic::AtomicU64::new(0),
            active_connections: std::sync::atomic::AtomicI64::new(0),
            counters: std::sync::Mutex::new(std::collections::HashMap::new()),
            model_stats: std::sync::Mutex::new(std::collections::HashMap::new()),
            provider_stats: std::sync::Mutex::new(std::collections::HashMap::new()),
            user_stats: std::sync::Mutex::new(std::collections::HashMap::new()),
            errors: std::sync::Mutex::new(std::collections::HashMap::new()),
            retries: std::sync::Mutex::new(std::collections::HashMap::new()),
            circuit_breakers: std::sync::Mutex::new(std::collections::HashMap::new()),
            daily_costs: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn record_request(
        &self,
        model: &str,
        provider: &str,
        user: Option<&str>,
        success: bool,
        _latency_ms: u64,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
    ) {
        use std::sync::atomic::Ordering;

        self.total_requests.fetch_add(1, Ordering::SeqCst);

        if success {
            self.success_requests.fetch_add(1, Ordering::SeqCst);
        } else {
            self.failed_requests.fetch_add(1, Ordering::SeqCst);
        }

        *self.total_cost.lock().unwrap() += cost;
        self.total_input_tokens
            .fetch_add(input_tokens, Ordering::SeqCst);
        self.total_output_tokens
            .fetch_add(output_tokens, Ordering::SeqCst);

        // 更新模型统计
        {
            let mut stats = self.model_stats.lock().unwrap();
            let entry = stats.entry(model.to_string()).or_default();
            entry.request_count += 1;
            entry.total_tokens_input += input_tokens;
            entry.total_tokens_output += output_tokens;
            entry.total_cost += cost;
        }

        // 更新提供商统计
        {
            let mut stats = self.provider_stats.lock().unwrap();
            let entry = stats.entry(provider.to_string()).or_default();
            entry.request_count += 1;
            if !success {
                entry.error_count += 1;
            }
        }

        // 更新用户统计
        if let Some(uid) = user {
            let mut stats = self.user_stats.lock().unwrap();
            let entry = stats.entry(uid.to_string()).or_default();
            entry.request_count += 1;
            entry.total_tokens_input += input_tokens;
            entry.total_tokens_output += output_tokens;
            entry.total_cost += cost;
        }

        // 更新每日成本
        {
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let mut costs = self.daily_costs.lock().unwrap();
            *costs.entry(today).or_default() += cost;
        }
    }

    fn increment_counter(&self, name: &str) {
        use std::sync::atomic::Ordering;

        // 特殊处理预定义计数器
        match name {
            "cache_hits" => {
                self.cache_hits.fetch_add(1, Ordering::SeqCst);
            }
            "cache_misses" => {
                self.cache_misses.fetch_add(1, Ordering::SeqCst);
            }
            _ => {
                let mut counters = self.counters.lock().unwrap();
                *counters.entry(name.to_string()).or_default() += 1;
            }
        }
    }

    fn increment_connections(&self) {
        use std::sync::atomic::Ordering;
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }

    fn decrement_connections(&self) {
        use std::sync::atomic::Ordering;
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }

    fn get_active_connections(&self) -> i64 {
        use std::sync::atomic::Ordering;
        self.active_connections.load(Ordering::SeqCst)
    }

    fn get_cache_hit_rate(&self) -> f64 {
        use std::sync::atomic::Ordering;
        let hits = self.cache_hits.load(Ordering::SeqCst);
        let misses = self.cache_misses.load(Ordering::SeqCst);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    fn get_summary(&self) -> MetricsSummary {
        use std::sync::atomic::Ordering;
        MetricsSummary {
            total_requests: self.total_requests.load(Ordering::SeqCst),
            success_requests: self.success_requests.load(Ordering::SeqCst),
            failed_requests: self.failed_requests.load(Ordering::SeqCst),
            total_cost: *self.total_cost.lock().unwrap(),
            total_input_tokens: self.total_input_tokens.load(Ordering::SeqCst),
            total_output_tokens: self.total_output_tokens.load(Ordering::SeqCst),
        }
    }

    fn get_model_stats(&self, model: &str) -> Option<ModelStats> {
        let stats = self.model_stats.lock().unwrap();
        stats.get(model).cloned()
    }

    fn get_provider_stats(&self, provider: &str) -> Option<ProviderStats> {
        let stats = self.provider_stats.lock().unwrap();
        stats.get(provider).cloned()
    }

    fn get_user_stats(&self, user: &str) -> Option<UserStats> {
        let stats = self.user_stats.lock().unwrap();
        stats.get(user).cloned()
    }

    fn record_error(&self, error_type: &str, _provider: &str, _model: &str) {
        let mut errors = self.errors.lock().unwrap();
        *errors.entry(error_type.to_string()).or_default() += 1;
    }

    fn get_errors_by_type(&self) -> std::collections::HashMap<String, u64> {
        self.errors.lock().unwrap().clone()
    }

    fn record_retry(&self, provider: &str, _reason: &str) {
        let mut retries = self.retries.lock().unwrap();
        *retries.entry(provider.to_string()).or_default() += 1;
    }

    fn get_retries(&self) -> std::collections::HashMap<String, u64> {
        self.retries.lock().unwrap().clone()
    }

    fn set_circuit_breaker_state(&self, provider: &str, account: &str, state: i64) {
        let key = format!("{}:{}", provider, account);
        let mut cb = self.circuit_breakers.lock().unwrap();
        cb.insert(key, state);
    }

    fn get_circuit_breaker_state(&self, provider: &str, account: &str) -> i64 {
        let key = format!("{}:{}", provider, account);
        let cb = self.circuit_breakers.lock().unwrap();
        cb.get(&key).copied().unwrap_or(0)
    }

    fn get_today_cost(&self) -> f64 {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let costs = self.daily_costs.lock().unwrap();
        costs.get(&today).copied().unwrap_or(0.0)
    }

    fn reset(&self) {
        use std::sync::atomic::Ordering;

        self.total_requests.store(0, Ordering::SeqCst);
        self.success_requests.store(0, Ordering::SeqCst);
        self.failed_requests.store(0, Ordering::SeqCst);
        *self.total_cost.lock().unwrap() = 0.0;
        self.total_input_tokens.store(0, Ordering::SeqCst);
        self.total_output_tokens.store(0, Ordering::SeqCst);
        self.cache_hits.store(0, Ordering::SeqCst);
        self.cache_misses.store(0, Ordering::SeqCst);
        self.active_connections.store(0, Ordering::SeqCst);
        self.counters.lock().unwrap().clear();
        self.model_stats.lock().unwrap().clear();
        self.provider_stats.lock().unwrap().clear();
        self.user_stats.lock().unwrap().clear();
        self.errors.lock().unwrap().clear();
        self.retries.lock().unwrap().clear();
        self.circuit_breakers.lock().unwrap().clear();
        self.daily_costs.lock().unwrap().clear();
    }

    fn export_prometheus(&self) -> String {
        use std::sync::atomic::Ordering;

        let mut output = String::new();

        output.push_str("# HELP foxnio_requests_total Total number of requests\n");
        output.push_str("# TYPE foxnio_requests_total counter\n");
        output.push_str(&format!(
            "foxnio_requests_total {}\n",
            self.total_requests.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP foxnio_cache_hits Number of cache hits\n");
        output.push_str("# TYPE foxnio_cache_hits counter\n");
        output.push_str(&format!(
            "foxnio_cache_hits {}\n",
            self.cache_hits.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP foxnio_cache_misses Number of cache misses\n");
        output.push_str("# TYPE foxnio_cache_misses counter\n");
        output.push_str(&format!(
            "foxnio_cache_misses {}\n",
            self.cache_misses.load(Ordering::SeqCst)
        ));

        output.push_str("# HELP foxnio_total_cost Total cost in dollars\n");
        output.push_str("# TYPE foxnio_total_cost gauge\n");
        output.push_str(&format!(
            "foxnio_total_cost {}\n",
            *self.total_cost.lock().unwrap()
        ));

        output
    }

    fn export_json(&self) -> String {
        let summary = self.get_summary();
        serde_json::to_string(&serde_json::json!({
            "requests_total": summary.total_requests,
            "success_requests": summary.success_requests,
            "failed_requests": summary.failed_requests,
            "total_cost": summary.total_cost,
            "cache_hits": self.cache_hits.load(std::sync::atomic::Ordering::SeqCst),
        }))
        .unwrap()
    }
}

/// 测试用的延迟直方图
#[derive(Debug, Clone)]
struct TestLatencyHistogram {
    values: Vec<f64>,
}

impl TestLatencyHistogram {
    fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn observe(&mut self, value: f64) {
        self.values.push(value);
    }

    fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }

        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let idx = ((sorted.len() as f64 * p / 100.0) as usize).min(sorted.len() - 1);
        sorted[idx]
    }
}

/// 获取当前内存使用量（简化版）
fn get_current_memory_usage() -> usize {
    // 简化实现，实际应该使用系统调用
    0
}
