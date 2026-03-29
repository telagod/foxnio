#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 性能基准测试

use std::time::{Duration, Instant};

/// 基准测试结果
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
}

impl BenchmarkResult {
    pub fn ops_per_second(&self) -> f64 {
        self.iterations as f64 / self.total_duration.as_secs_f64()
    }
}

/// 基准测试运行器
pub struct Benchmark {
    name: String,
    iterations: usize,
}

impl Benchmark {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            iterations: 1000,
        }
    }

    pub fn iterations(mut self, n: usize) -> Self {
        self.iterations = n;
        self
    }

    pub fn run<F>(&self, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        let mut durations = Vec::with_capacity(self.iterations);

        for _ in 0..self.iterations {
            let start = Instant::now();
            f();
            durations.push(start.elapsed());
        }

        let total: Duration = durations.iter().sum();
        let avg = total / self.iterations as u32;
        let min = durations.iter().min().copied().unwrap();
        let max = durations.iter().max().copied().unwrap();

        BenchmarkResult {
            name: self.name.clone(),
            iterations: self.iterations,
            total_duration: total,
            avg_duration: avg,
            min_duration: min,
            max_duration: max,
        }
    }

    pub async fn run_async<F, Fut>(&self, mut f: F) -> BenchmarkResult
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut durations = Vec::with_capacity(self.iterations);

        for _ in 0..self.iterations {
            let start = Instant::now();
            f().await;
            durations.push(start.elapsed());
        }

        let total: Duration = durations.iter().sum();
        let avg = total / self.iterations as u32;
        let min = durations.iter().min().copied().unwrap();
        let max = durations.iter().max().copied().unwrap();

        BenchmarkResult {
            name: self.name.clone(),
            iterations: self.iterations,
            total_duration: total,
            avg_duration: avg,
            min_duration: min,
            max_duration: max,
        }
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {} iterations, avg={:?}, min={:?}, max={:?}, ops/s={:.2}",
            self.name,
            self.iterations,
            self.avg_duration,
            self.min_duration,
            self.max_duration,
            self.ops_per_second()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_simple() {
        let result = Benchmark::new("simple_addition").iterations(100).run(|| {
            let _ = 1 + 1;
        });

        assert_eq!(result.iterations, 100);
        assert!(result.avg_duration < Duration::from_millis(1));
        println!("{}", result);
    }

    #[tokio::test]
    async fn test_benchmark_async() {
        let result = Benchmark::new("async_operation")
            .iterations(10)
            .run_async(|| async {
                tokio::time::sleep(Duration::from_micros(1)).await;
            })
            .await;

        assert_eq!(result.iterations, 10);
        println!("{}", result);
    }

    #[test]
    fn test_benchmark_string_concat() {
        let result = Benchmark::new("string_concat").iterations(1000).run(|| {
            let _ = format!("prefix-{}-suffix", 12345);
        });

        println!("{}", result);
        assert!(result.ops_per_second() > 10000.0);
    }

    #[test]
    fn test_benchmark_json_parse() {
        let json = r#"{"name":"test","value":123}"#;

        let result = Benchmark::new("json_parse").iterations(1000).run(|| {
            let _: serde_json::Value = serde_json::from_str(json).unwrap();
        });

        println!("{}", result);
    }

    #[test]
    fn test_benchmark_uuid_generation() {
        let result = Benchmark::new("uuid_v4").iterations(1000).run(|| {
            let _ = uuid::Uuid::new_v4();
        });

        println!("{}", result);
        assert!(result.ops_per_second() > 10000.0);
    }
}
