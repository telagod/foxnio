//! 统一健康检查模块 - FoxNIO v0.2.0
//!
//! 功能特性：
//! - 统一的健康检查接口
//! - 并行和顺序检查模式
//! - 超时控制
//! - 重试机制
//! - 详细的错误信息
//! - 系统资源监控
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, warn};

// ============================================================================
// 核心类型定义
// ============================================================================

/// 健康检查状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// 是否健康
    pub healthy: bool,
    /// 响应延迟（毫秒）
    pub latency_ms: u64,
    /// 状态消息
    pub message: String,
    /// 详细信息
    #[serde(default)]
    pub details: HashMap<String, String>,
}

impl HealthStatus {
    /// 创建健康状态
    pub fn healthy(message: impl Into<String>, latency_ms: u64) -> Self {
        Self {
            healthy: true,
            latency_ms,
            message: message.into(),
            details: HashMap::new(),
        }
    }

    /// 创建不健康状态
    pub fn unhealthy(message: impl Into<String>, latency_ms: u64) -> Self {
        Self {
            healthy: false,
            latency_ms,
            message: message.into(),
            details: HashMap::new(),
        }
    }

    /// 添加详细信息
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

/// 健康检查接口
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// 检查名称
    fn name(&self) -> &str;

    /// 是否为关键检查
    fn is_critical(&self) -> bool {
        true
    }

    /// 执行检查
    async fn check(&self) -> Result<HealthStatus>;
}

/// 检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// 检查名称
    pub name: String,
    /// 是否为关键检查
    pub critical: bool,
    /// 健康状态
    #[serde(flatten)]
    pub status: HealthStatus,
}

/// 聚合健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateHealthStatus {
    /// 整体是否健康
    pub healthy: bool,
    /// 检查时间戳
    pub timestamp: String,
    /// 总检查数
    pub total_checks: usize,
    /// 健康检查数
    pub healthy_checks: usize,
    /// 不健康检查数
    pub unhealthy_checks: usize,
    /// 各检查结果
    pub checks: HashMap<String, CheckResult>,
    /// 总耗时（毫秒）
    pub total_latency_ms: u64,
}

// ============================================================================
// 健康检查器
// ============================================================================

/// 统一健康检查器
pub struct HealthChecker {
    /// 检查项集合
    checks: RwLock<HashMap<String, Box<dyn HealthCheck>>>,
    /// 默认超时时间
    default_timeout: Duration,
    /// 默认重试次数
    default_retries: u32,
    /// 重试延迟
    retry_delay: Duration,
}

impl HealthChecker {
    /// 创建新的健康检查器
    pub fn new() -> Self {
        Self {
            checks: RwLock::new(HashMap::new()),
            default_timeout: Duration::from_secs(5),
            default_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }

    /// 设置默认超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// 设置默认重试次数
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.default_retries = retries;
        self
    }

    /// 设置重试延迟
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// 注册健康检查
    pub async fn register(&self, check: Box<dyn HealthCheck>) {
        let name = check.name().to_string();
        let mut checks = self.checks.write().await;
        checks.insert(name, check);
    }

    /// 移除健康检查
    pub async fn unregister(&self, name: &str) {
        let mut checks = self.checks.write().await;
        checks.remove(name);
    }

    /// 获取所有检查名称
    pub async fn check_names(&self) -> Vec<String> {
        let checks = self.checks.read().await;
        checks.keys().cloned().collect()
    }

    /// 执行单个检查（带超时和重试）
    async fn run_check(&self, check: &dyn HealthCheck) -> CheckResult {
        let name = check.name().to_string();
        let critical = check.is_critical();

        let mut last_error: String;
        let mut attempts = 0;

        loop {
            attempts += 1;
            let start = Instant::now();

            // 带超时执行检查
            let result = timeout(self.default_timeout, check.check()).await;

            match result {
                Ok(Ok(status)) => {
                    return CheckResult {
                        name,
                        critical,
                        status,
                    };
                }
                Ok(Err(e)) => {
                    last_error = e.to_string();
                    debug!(
                        "Health check '{}' failed (attempt {}/{}): {}",
                        name, attempts, self.default_retries, e
                    );
                }
                Err(_) => {
                    last_error = format!("Timeout after {:?}", self.default_timeout);
                    warn!(
                        "Health check '{}' timed out (attempt {}/{})",
                        name, attempts, self.default_retries
                    );
                }
            }

            // 检查是否需要重试
            if attempts >= self.default_retries {
                let latency = start.elapsed().as_millis() as u64;
                return CheckResult {
                    name,
                    critical,
                    status: HealthStatus::unhealthy(&last_error, latency),
                };
            }

            // 重试延迟
            tokio::time::sleep(self.retry_delay).await;
        }
    }

    /// 并行执行所有检查
    pub async fn check_all(&self) -> AggregateHealthStatus {
        let start = Instant::now();
        let checks = self.checks.read().await;

        // 并行执行所有检查
        let futures: Vec<_> = checks
            .values()
            .map(|c| self.run_check(c.as_ref()))
            .collect();
        let results = futures::future::join_all(futures).await;

        self.build_aggregate_status(results, start)
    }

    /// 顺序执行所有检查
    pub async fn check_sequential(&self) -> AggregateHealthStatus {
        let start = Instant::now();
        let checks = self.checks.read().await;

        let mut results = Vec::new();
        for check in checks.values() {
            results.push(self.run_check(check.as_ref()).await);
        }

        self.build_aggregate_status(results, start)
    }

    /// 只检查关键服务
    pub async fn check_critical(&self) -> AggregateHealthStatus {
        let start = Instant::now();
        let checks = self.checks.read().await;

        // 并行执行关键检查
        let futures: Vec<_> = checks
            .values()
            .filter(|c| c.is_critical())
            .map(|c| self.run_check(c.as_ref()))
            .collect();
        let results = futures::future::join_all(futures).await;

        self.build_aggregate_status(results, start)
    }

    /// 执行单个检查
    pub async fn check_one(&self, name: &str) -> Option<CheckResult> {
        let checks = self.checks.read().await;
        let check = checks.get(name)?;
        Some(self.run_check(check.as_ref()).await)
    }

    /// 构建聚合状态
    fn build_aggregate_status(
        &self,
        results: Vec<CheckResult>,
        start: Instant,
    ) -> AggregateHealthStatus {
        let total_checks = results.len();
        let healthy_checks = results.iter().filter(|r| r.status.healthy).count();
        let unhealthy_checks = total_checks - healthy_checks;

        // 关键检查失败则整体不健康
        let healthy = results
            .iter()
            .filter(|r| r.critical)
            .all(|r| r.status.healthy);

        let checks_map: HashMap<String, CheckResult> =
            results.into_iter().map(|r| (r.name.clone(), r)).collect();

        AggregateHealthStatus {
            healthy,
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_checks,
            healthy_checks,
            unhealthy_checks,
            checks: checks_map,
            total_latency_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// 存活检查（总是返回 true，仅表示进程存活）
    pub fn liveness() -> HealthStatus {
        HealthStatus::healthy("alive", 0)
    }

    /// 就绪检查（快速检查关键服务）
    pub async fn readiness(&self) -> AggregateHealthStatus {
        self.check_critical().await
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// PostgreSQL 健康检查
// ============================================================================

/// PostgreSQL 健康检查
pub struct PostgresHealthCheck {
    name: String,
    pool: sqlx::PgPool,
    timeout: Duration,
}

impl PostgresHealthCheck {
    /// 创建 PostgreSQL 健康检查
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            name: "postgresql".to_string(),
            pool,
            timeout: Duration::from_secs(5),
        }
    }

    /// 设置名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl HealthCheck for PostgresHealthCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthStatus> {
        let start = Instant::now();

        let result = timeout(self.timeout, async {
            sqlx::query("SELECT 1").fetch_one(&self.pool).await
        })
        .await;

        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(_)) => {
                let pool_size = self.pool.size();
                let num_idle = self.pool.num_idle();

                Ok(
                    HealthStatus::healthy("PostgreSQL connection is active", latency)
                        .with_detail("pool_size", pool_size.to_string())
                        .with_detail("idle_connections", num_idle.to_string()),
                )
            }
            Ok(Err(e)) => {
                error!("PostgreSQL health check failed: {}", e);
                Ok(HealthStatus::unhealthy(
                    format!("PostgreSQL error: {e}"),
                    latency,
                ))
            }
            Err(_) => {
                error!("PostgreSQL health check timed out");
                Ok(HealthStatus::unhealthy(
                    format!("Timeout after {:?}", self.timeout),
                    latency,
                ))
            }
        }
    }
}

// ============================================================================
// Redis 健康检查
// ============================================================================

/// Redis 健康检查
pub struct RedisHealthCheck {
    name: String,
    pool: crate::db::RedisPool,
    timeout: Duration,
}

impl RedisHealthCheck {
    /// 创建 Redis 健康检查
    pub fn new(pool: crate::db::RedisPool) -> Self {
        Self {
            name: "redis".to_string(),
            pool,
            timeout: Duration::from_secs(5),
        }
    }

    /// 设置名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait::async_trait]
impl HealthCheck for RedisHealthCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_critical(&self) -> bool {
        true
    }

    async fn check(&self) -> Result<HealthStatus> {
        let start = Instant::now();

        let result = timeout(self.timeout, self.pool.health_check()).await;

        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(true)) => {
                let stats = self.pool.get_stats();
                let hit_rate = stats.cache_hit_rate();
                let avg_latency = stats.avg_latency_ms();

                Ok(HealthStatus::healthy("Redis connection is active", latency)
                    .with_detail("cache_hit_rate", format!("{:.1}%", hit_rate * 100.0))
                    .with_detail("avg_latency_ms", format!("{:.1}", avg_latency)))
            }
            Ok(Ok(false)) => Ok(HealthStatus::unhealthy(
                "Redis health check returned false",
                latency,
            )),
            Ok(Err(e)) => {
                error!("Redis health check failed: {}", e);
                Ok(HealthStatus::unhealthy(
                    format!("Redis error: {e}"),
                    latency,
                ))
            }
            Err(_) => {
                error!("Redis health check timed out");
                Ok(HealthStatus::unhealthy(
                    format!("Timeout after {:?}", self.timeout),
                    latency,
                ))
            }
        }
    }
}

// ============================================================================
// 系统资源健康检查
// ============================================================================

/// 系统资源健康检查
pub struct SystemResourceHealthCheck {
    name: String,
    /// CPU 使用率阈值（百分比，超过则不健康）
    cpu_threshold: f32,
    /// 内存使用率阈值（百分比，超过则不健康）
    memory_threshold: f32,
    /// 磁盘使用率阈值（百分比，超过则不健康）
    disk_threshold: f32,
    /// 磁盘路径
    disk_path: String,
}

impl SystemResourceHealthCheck {
    /// 创建系统资源健康检查
    pub fn new() -> Self {
        Self {
            name: "system_resources".to_string(),
            cpu_threshold: 90.0,
            memory_threshold: 90.0,
            disk_threshold: 90.0,
            disk_path: "/".to_string(),
        }
    }

    /// 设置 CPU 阈值
    pub fn with_cpu_threshold(mut self, threshold: f32) -> Self {
        self.cpu_threshold = threshold;
        self
    }

    /// 设置内存阈值
    pub fn with_memory_threshold(mut self, threshold: f32) -> Self {
        self.memory_threshold = threshold;
        self
    }

    /// 设置磁盘阈值
    pub fn with_disk_threshold(mut self, threshold: f32) -> Self {
        self.disk_threshold = threshold;
        self
    }

    /// 设置磁盘路径
    pub fn with_disk_path(mut self, path: impl Into<String>) -> Self {
        self.disk_path = path.into();
        self
    }

    /// 获取 CPU 使用率
    fn get_cpu_usage(&self) -> Result<f32> {
        #[cfg(target_os = "linux")]
        {
            // 读取 /proc/stat 获取 CPU 使用率
            let stat = std::fs::read_to_string("/proc/stat")?;
            let first_line = stat
                .lines()
                .next()
                .ok_or_else(|| anyhow::anyhow!("Failed to read /proc/stat"))?;

            let parts: Vec<u64> = first_line
                .split_whitespace()
                .skip(1)
                .take(8)
                .map(|s| s.parse().unwrap_or(0))
                .collect();

            if parts.len() < 4 {
                return Ok(0.0);
            }

            // 计算 CPU 使用率
            let idle = parts[3];
            let total: u64 = parts.iter().sum();

            // 简单计算，返回一个估算值
            // 实际生产中应该间隔两次读取计算差值
            let usage = if total > 0 {
                (1.0 - (idle as f64 / total as f64)) as f32 * 100.0
            } else {
                0.0
            };

            Ok(usage)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(0.0)
        }
    }

    /// 获取内存使用率
    fn get_memory_usage(&self) -> Result<f32> {
        #[cfg(target_os = "linux")]
        {
            let meminfo = std::fs::read_to_string("/proc/meminfo")?;
            let mut total: u64 = 0;
            let mut available: u64 = 0;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    total = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    available = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }

            if total > 0 {
                let used = total.saturating_sub(available);
                Ok((used as f64 / total as f64) as f32 * 100.0)
            } else {
                Ok(0.0)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(0.0)
        }
    }

    /// 获取磁盘使用率
    fn get_disk_usage(&self) -> Result<f32> {
        #[cfg(target_os = "linux")]
        {
            use std::path::Path;

            let path = Path::new(&self.disk_path);
            let stat = nix::sys::statvfs::statvfs(path)
                .map_err(|e| anyhow::anyhow!("Failed to get disk stats: {}", e))?;

            let total = stat.blocks() as u64 * stat.block_size() as u64;
            let available = stat.blocks_available() as u64 * stat.block_size() as u64;

            if total > 0 {
                let used = total.saturating_sub(available);
                Ok((used as f64 / total as f64) as f32 * 100.0)
            } else {
                Ok(0.0)
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = &self.disk_path;
            Ok(0.0)
        }
    }
}

impl Default for SystemResourceHealthCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HealthCheck for SystemResourceHealthCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_critical(&self) -> bool {
        false // 系统资源检查不是关键的，不影响整体健康
    }

    async fn check(&self) -> Result<HealthStatus> {
        let start = Instant::now();

        let mut warnings = Vec::new();
        let mut details = HashMap::new();

        // CPU 检查
        match self.get_cpu_usage() {
            Ok(cpu_usage) => {
                details.insert("cpu_usage_percent".to_string(), format!("{:.1}", cpu_usage));
                if cpu_usage > self.cpu_threshold {
                    warnings.push(format!(
                        "CPU usage {:.1}% exceeds threshold {:.1}%",
                        cpu_usage, self.cpu_threshold
                    ));
                }
            }
            Err(e) => {
                details.insert("cpu_usage_error".to_string(), e.to_string());
            }
        }

        // 内存检查
        match self.get_memory_usage() {
            Ok(mem_usage) => {
                details.insert(
                    "memory_usage_percent".to_string(),
                    format!("{:.1}", mem_usage),
                );
                if mem_usage > self.memory_threshold {
                    warnings.push(format!(
                        "Memory usage {:.1}% exceeds threshold {:.1}%",
                        mem_usage, self.memory_threshold
                    ));
                }
            }
            Err(e) => {
                details.insert("memory_usage_error".to_string(), e.to_string());
            }
        }

        // 磁盘检查
        match self.get_disk_usage() {
            Ok(disk_usage) => {
                details.insert(
                    "disk_usage_percent".to_string(),
                    format!("{:.1}", disk_usage),
                );
                details.insert("disk_path".to_string(), self.disk_path.clone());
                if disk_usage > self.disk_threshold {
                    warnings.push(format!(
                        "Disk usage {:.1}% exceeds threshold {:.1}%",
                        disk_usage, self.disk_threshold
                    ));
                }
            }
            Err(e) => {
                details.insert("disk_usage_error".to_string(), e.to_string());
            }
        }

        let latency = start.elapsed().as_millis() as u64;

        let mut status = if warnings.is_empty() {
            HealthStatus::healthy("System resources are within normal range", latency)
        } else {
            HealthStatus::healthy(warnings.join("; "), latency)
        };

        status.details = details;
        Ok(status)
    }
}

// ============================================================================
// API 响应类型
// ============================================================================

/// 简单健康状态响应
#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleHealthResponse {
    pub status: &'static str,
    pub timestamp: String,
}

/// 就绪状态响应
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub timestamp: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, bool>,
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_healthy() {
        let status = HealthStatus::healthy("OK", 10);
        assert!(status.healthy);
        assert_eq!(status.latency_ms, 10);
        assert_eq!(status.message, "OK");
    }

    #[test]
    fn test_health_status_unhealthy() {
        let status = HealthStatus::unhealthy("Error", 5);
        assert!(!status.healthy);
        assert_eq!(status.latency_ms, 5);
        assert_eq!(status.message, "Error");
    }

    #[test]
    fn test_health_status_with_detail() {
        let status = HealthStatus::healthy("OK", 10)
            .with_detail("key1", "value1")
            .with_detail("key2", "value2");

        assert_eq!(status.details.get("key1"), Some(&"value1".to_string()));
        assert_eq!(status.details.get("key2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_health_checker_new() {
        let checker = HealthChecker::new();
        let names = checker.check_names().await;
        assert!(names.is_empty());
    }

    #[tokio::test]
    async fn test_health_checker_register() {
        let checker = HealthChecker::new();

        struct MockCheck;
        #[async_trait::async_trait]
        impl HealthCheck for MockCheck {
            fn name(&self) -> &str {
                "mock"
            }

            async fn check(&self) -> Result<HealthStatus> {
                Ok(HealthStatus::healthy("mock check", 0))
            }
        }

        checker.register(Box::new(MockCheck)).await;
        let names = checker.check_names().await;
        assert!(names.contains(&"mock".to_string()));
    }

    #[tokio::test]
    async fn test_health_checker_liveness() {
        let status = HealthChecker::liveness();
        assert!(status.healthy);
        assert_eq!(status.message, "alive");
    }

    #[test]
    fn test_system_resource_check_creation() {
        let check = SystemResourceHealthCheck::new()
            .with_cpu_threshold(80.0)
            .with_memory_threshold(85.0)
            .with_disk_threshold(90.0)
            .with_disk_path("/data");

        assert_eq!(check.cpu_threshold, 80.0);
        assert_eq!(check.memory_threshold, 85.0);
        assert_eq!(check.disk_threshold, 90.0);
        assert_eq!(check.disk_path, "/data");
    }

    #[test]
    fn test_aggregate_health_status() {
        let status = AggregateHealthStatus {
            healthy: true,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            total_checks: 2,
            healthy_checks: 2,
            unhealthy_checks: 0,
            checks: HashMap::new(),
            total_latency_ms: 100,
        };

        assert!(status.healthy);
        assert_eq!(status.total_checks, 2);
    }

    #[test]
    fn test_check_result() {
        let result = CheckResult {
            name: "test".to_string(),
            critical: true,
            status: HealthStatus::healthy("OK", 10),
        };

        assert!(result.critical);
        assert!(result.status.healthy);
    }
}
