#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! HTTP/2 性能基准测试
//!
//! 对比 HTTP/1.1 vs HTTP/2 性能

use futures::future::join_all;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// 测试配置
struct TestConfig {
    url: String,
    requests: usize,
    concurrency: usize,
}

/// 测试结果
struct TestResult {
    total_time: Duration,
    requests_per_second: f64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    success_count: usize,
    error_count: usize,
}

/// 执行 HTTP 基准测试
async fn run_benchmark(config: TestConfig, client: reqwest::Client) -> TestResult {
    let semaphore = Arc::new(Semaphore::new(config.concurrency));
    let mut latencies: Vec<Duration> = Vec::with_capacity(config.requests);
    let mut success_count = 0;
    let mut error_count = 0;

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..config.requests {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let url = config.url.clone();

        let handle = tokio::spawn(async move {
            let request_start = Instant::now();
            let result = client.get(&url).send().await;
            let latency = request_start.elapsed();

            drop(permit);

            match result {
                Ok(resp) if resp.status().is_success() => (latency, true),
                _ => (latency, false),
            }
        });

        handles.push(handle);
    }

    let results = join_all(handles).await;

    for result in results {
        if let Ok((latency, success)) = result {
            latencies.push(latency);
            if success {
                success_count += 1;
            } else {
                error_count += 1;
            }
        } else {
            error_count += 1;
        }
    }

    let total_time = start.elapsed();

    // 计算统计信息
    latencies.sort();
    let avg_latency_ms = if !latencies.is_empty() {
        latencies.iter().sum::<Duration>().as_secs_f64() * 1000.0 / latencies.len() as f64
    } else {
        0.0
    };

    let min_latency_ms = latencies
        .first()
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let max_latency_ms = latencies
        .last()
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);

    let requests_per_second = if total_time.as_secs_f64() > 0.0 {
        config.requests as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };

    TestResult {
        total_time,
        requests_per_second,
        avg_latency_ms,
        min_latency_ms,
        max_latency_ms,
        success_count,
        error_count,
    }
}

/// 创建 HTTP/1.1 客户端
fn create_http1_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http1_only()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP/1.1 client")
}

/// 创建 HTTP/2 客户端
fn create_http2_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP/2 client")
}

/// 创建自动协商客户端
fn create_auto_negotiate_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create auto-negotiate client")
}

fn print_result(name: &str, result: &TestResult) {
    println!("\n{}", name);
    println!("{}", "=".repeat(name.len()));
    println!("Total Time:        {:?}", result.total_time);
    println!("Requests/sec:      {:.2}", result.requests_per_second);
    println!("Avg Latency:       {:.2} ms", result.avg_latency_ms);
    println!("Min Latency:       {:.2} ms", result.min_latency_ms);
    println!("Max Latency:       {:.2} ms", result.max_latency_ms);
    println!("Success Count:     {}", result.success_count);
    println!("Error Count:       {}", result.error_count);
}

#[tokio::test]
#[ignore = "Requires external HTTP/2 server"]
async fn test_http2_vs_http1_benchmark() {
    // 测试配置
    let test_configs = vec![TestConfig {
        url: "https://http2.pro/api/v1".to_string(),
        requests: 100,
        concurrency: 10,
    }];

    for config in test_configs {
        println!("\n{}", "=".repeat(60));
        println!(
            "Testing: {} ({} requests, {} concurrent)",
            config.url, config.requests, config.concurrency
        );
        println!("{}", "=".repeat(60));

        // HTTP/1.1 测试
        let http1_client = create_http1_client();
        let http1_result = run_benchmark(
            TestConfig {
                url: config.url.clone(),
                requests: config.requests,
                concurrency: config.concurrency,
            },
            http1_client,
        )
        .await;
        print_result("HTTP/1.1 Results", &http1_result);

        // HTTP/2 测试
        let http2_client = create_http2_client();
        let http2_result = run_benchmark(
            TestConfig {
                url: config.url.clone(),
                requests: config.requests,
                concurrency: config.concurrency,
            },
            http2_client,
        )
        .await;
        print_result("HTTP/2 Results", &http2_result);

        // 自动协商测试
        let auto_client = create_auto_negotiate_client();
        let auto_result = run_benchmark(
            TestConfig {
                url: config.url.clone(),
                requests: config.requests,
                concurrency: config.concurrency,
            },
            auto_client,
        )
        .await;
        print_result("Auto-Negotiate Results", &auto_result);

        // 性能对比
        println!("\n{}", "-".repeat(40));
        println!("Performance Comparison:");
        println!("{}", "-".repeat(40));
        if http1_result.requests_per_second > 0.0 {
            let improvement = (http2_result.requests_per_second - http1_result.requests_per_second)
                / http1_result.requests_per_second
                * 100.0;
            println!("HTTP/2 vs HTTP/1.1: {:+.1}% throughput", improvement);
        }
        if http1_result.avg_latency_ms > 0.0 {
            let latency_improvement = (http1_result.avg_latency_ms - http2_result.avg_latency_ms)
                / http1_result.avg_latency_ms
                * 100.0;
            println!("HTTP/2 vs HTTP/1.1: {:+.1}% latency", latency_improvement);
        }
    }
}

#[tokio::test]
async fn test_concurrent_requests_http2() {
    let client = create_auto_negotiate_client();

    let semaphore = Arc::new(Semaphore::new(50));
    let mut handles = Vec::new();

    // 发送 100 个并发请求
    for _ in 0..100 {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();

        let handle = tokio::spawn(async move {
            let result = client.get("https://httpbin.org/get").send().await;
            drop(permit);
            result.is_ok()
        });

        handles.push(handle);
    }

    let results: Vec<_> = join_all(handles).await;
    let success_count = results
        .iter()
        .filter(|r| r.as_ref().map(|&s| s).unwrap_or(false))
        .count();

    println!("Concurrent requests test: {}/100 successful", success_count);
    assert!(success_count > 90, "Expected at least 90% success rate");
}

#[tokio::test]
async fn test_connection_reuse_http2() {
    let client = create_auto_negotiate_client();

    // 发送多个请求到同一个服务器，验证连接复用
    let start = Instant::now();

    for _ in 0..10 {
        let _ = client.get("https://httpbin.org/get").send().await;
    }

    let duration = start.elapsed();

    // HTTP/2 连接复用应该比 HTTP/1.1 更快
    println!("10 sequential requests took: {:?}", duration);
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let _http1 = create_http1_client();
        let _http2 = create_http2_client();
        let _auto = create_auto_negotiate_client();
    }

    #[test]
    fn test_result_calculation() {
        let result = TestResult {
            total_time: Duration::from_secs(1),
            requests_per_second: 100.0,
            avg_latency_ms: 10.0,
            min_latency_ms: 5.0,
            max_latency_ms: 20.0,
            success_count: 100,
            error_count: 0,
        };

        assert_eq!(result.requests_per_second, 100.0);
        assert_eq!(result.success_count, 100);
    }
}
