//! 负载测试配置

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 负载测试配置
pub struct LoadTestConfig {
    pub concurrent_users: usize,
    pub requests_per_user: usize,
    pub target_url: String,
    pub api_key: String,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_users: 10,
            requests_per_user: 100,
            target_url: "http://localhost:3000/v1/chat/completions".to_string(),
            api_key: "foxnio-test-key".to_string(),
        }
    }
}

/// 负载测试结果
#[derive(Debug, Clone)]
pub struct LoadTestResult {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_duration: Duration,
    pub avg_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub requests_per_second: f64,
}

/// 运行负载测试
pub async fn run_load_test(config: LoadTestConfig) -> LoadTestResult {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    
    let total_requests = Arc::new(AtomicU64::new(0));
    let successful_requests = Arc::new(AtomicU64::new(0));
    let failed_requests = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::new()));
    
    let start = Instant::now();
    
    let mut handles = vec![];
    
    for _ in 0..config.concurrent_users {
        let client = client.clone();
        let url = config.target_url.clone();
        let api_key = config.api_key.clone();
        let total = total_requests.clone();
        let success = successful_requests.clone();
        let failed = failed_requests.clone();
        let lats = latencies.clone();
        
        let handle = tokio::spawn(async move {
            for _ in 0..config.requests_per_user {
                total.fetch_add(1, Ordering::SeqCst);
                
                let req_start = Instant::now();
                
                let result = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&serde_json::json!({
                        "model": "gpt-4",
                        "messages": [{"role": "user", "content": "test"}],
                        "max_tokens": 10
                    }))
                    .send()
                    .await;
                
                let latency = req_start.elapsed();
                lats.lock().unwrap().push(latency);
                
                match result {
                    Ok(resp) if resp.status().is_success() => {
                        success.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {
                        failed.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有请求完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    let latencies = latencies.lock().unwrap();
    
    let avg_latency = if !latencies.is_empty() {
        latencies.iter().sum::<Duration>() / latencies.len() as u32
    } else {
        Duration::from_secs(0)
    };
    
    let min_latency = latencies.iter().min().copied().unwrap_or(Duration::from_secs(0));
    let max_latency = latencies.iter().max().copied().unwrap_or(Duration::from_secs(0));
    
    let total = total_requests.load(Ordering::SeqCst);
    let rps = total as f64 / duration.as_secs_f64();
    
    LoadTestResult {
        total_requests: total,
        successful_requests: successful_requests.load(Ordering::SeqCst),
        failed_requests: failed_requests.load(Ordering::SeqCst),
        total_duration: duration,
        avg_latency,
        min_latency,
        max_latency,
        requests_per_second: rps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // 需要运行的服务
    async fn test_light_load() {
        let config = LoadTestConfig {
            concurrent_users: 5,
            requests_per_user: 10,
            ..Default::default()
        };
        
        let result = run_load_test(config).await;
        
        println!("\n=== Load Test Results ===");
        println!("Total Requests: {}", result.total_requests);
        println!("Successful: {}", result.successful_requests);
        println!("Failed: {}", result.failed_requests);
        println!("Duration: {:?}", result.total_duration);
        println!("Avg Latency: {:?}", result.avg_latency);
        println!("Min Latency: {:?}", result.min_latency);
        println!("Max Latency: {:?}", result.max_latency);
        println!("Requests/sec: {:.2}", result.requests_per_second);
        
        // 验证成功率
        let success_rate = result.successful_requests as f64 / result.total_requests as f64;
        assert!(success_rate > 0.9, "Success rate should be > 90%");
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_medium_load() {
        let config = LoadTestConfig {
            concurrent_users: 20,
            requests_per_user: 50,
            ..Default::default()
        };
        
        let result = run_load_test(config).await;
        
        println!("\n=== Medium Load Test ===");
        println!("Requests/sec: {:.2}", result.requests_per_second);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_heavy_load() {
        let config = LoadTestConfig {
            concurrent_users: 50,
            requests_per_user: 100,
            ..Default::default()
        };
        
        let result = run_load_test(config).await;
        
        println!("\n=== Heavy Load Test ===");
        println!("Requests/sec: {:.2}", result.requests_per_second);
        println!("Failed: {}", result.failed_requests);
    }
}
