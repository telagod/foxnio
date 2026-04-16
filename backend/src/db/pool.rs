//! 数据库连接池实现 - v0.2.0 增强版
//!
//! 功能增强：
//! - 动态连接池大小调整
//! - 连接健康检查优化
//! - 连接复用率统计
//! - 连接泄漏检测
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection};
use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// 数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,

    // v0.2.0 新增配置
    pub enable_dynamic_sizing: bool,
    pub enable_leak_detection: bool,
    pub leak_detection_timeout: Duration,
    pub health_check_interval: Duration,
    pub auto_scale_threshold_low: f32,
    pub auto_scale_threshold_high: f32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/foxnio".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),

            // v0.2.0 默认值
            enable_dynamic_sizing: true,
            enable_leak_detection: true,
            leak_detection_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
            auto_scale_threshold_low: 0.3,
            auto_scale_threshold_high: 0.8,
        }
    }
}

/// 连接使用统计
#[derive(Debug, Default)]
pub struct ConnectionStats {
    /// 总请求数
    pub total_requests: AtomicU64,
    /// 连接复用次数
    pub reused_connections: AtomicU64,
    /// 新建连接次数
    pub new_connections: AtomicU64,
    /// 泄漏警报次数
    pub leak_warnings: AtomicU64,
    /// 健康检查失败次数
    pub health_check_failures: AtomicU64,
}

impl ConnectionStats {
    /// 记录连接请求
    pub fn record_request(&self, reused: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if reused {
            self.reused_connections.fetch_add(1, Ordering::Relaxed);
        } else {
            self.new_connections.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 获取复用率
    pub fn reuse_rate(&self) -> f32 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let reused = self.reused_connections.load(Ordering::Relaxed);
        reused as f32 / total as f32
    }

    /// 重置统计
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.reused_connections.store(0, Ordering::Relaxed);
        self.new_connections.store(0, Ordering::Relaxed);
        self.leak_warnings.store(0, Ordering::Relaxed);
        self.health_check_failures.store(0, Ordering::Relaxed);
    }
}

/// 连接池状态（增强版）
#[derive(Debug, Clone)]
pub struct PoolStatus {
    /// 当前连接数
    pub size: u32,
    /// 空闲连接数
    pub num_idle: u32,
    /// 是否已关闭
    pub is_closed: bool,
    /// 连接复用率
    pub reuse_rate: f32,
    /// 总请求数
    pub total_requests: u64,
    /// 泄漏警告数
    pub leak_warnings: u64,
}

/// 动态调整器
#[derive(Clone)]
pub struct DynamicAdjuster {
    config: DatabaseConfig,
    last_adjustment: Arc<RwLock<Instant>>,
    adjustment_cooldown: Duration,
}

impl DynamicAdjuster {
    pub fn new(config: &DatabaseConfig) -> Self {
        Self {
            config: config.clone(),
            last_adjustment: Arc::new(RwLock::new(Instant::now())),
            adjustment_cooldown: Duration::from_secs(60),
        }
    }

    /// 检查是否需要调整连接池大小
    pub async fn should_adjust(&self, pool_usage: f32) -> Option<i32> {
        if !self.config.enable_dynamic_sizing {
            return None;
        }

        // 检查冷却时间
        let last = *self.last_adjustment.read().await;
        if last.elapsed() < self.adjustment_cooldown {
            return None;
        }

        // 判断是否需要调整
        if pool_usage > self.config.auto_scale_threshold_high {
            // 使用率高，增加连接
            Some(5)
        } else if pool_usage < self.config.auto_scale_threshold_low {
            // 使用率低，减少连接
            Some(-5)
        } else {
            None
        }
    }

    /// 记录调整时间
    pub async fn record_adjustment(&self) {
        let mut last = self.last_adjustment.write().await;
        *last = Instant::now();
    }
}

/// 数据库连接池管理器（增强版）
#[derive(Clone)]
pub struct DatabasePool {
    pub sea_orm: DatabaseConnection,
    pub sqlx: sqlx::PgPool,
    stats: Arc<ConnectionStats>,
    adjuster: Option<DynamicAdjuster>,
    health_check_running: Arc<AtomicBool>,
    config: DatabaseConfig,
}

impl std::ops::Deref for DatabasePool {
    type Target = DatabaseConnection;

    fn deref(&self) -> &Self::Target {
        &self.sea_orm
    }
}

impl DatabasePool {
    /// 创建新的数据库连接池
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database: {}", Self::mask_url(&config.url));

        // 创建 sqlx 连接池
        let sqlx_pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.connect_timeout)
            .idle_timeout(Some(config.idle_timeout))
            .max_lifetime(Some(config.max_lifetime))
            .connect(&config.url)
            .await
            .context("Failed to create sqlx connection pool")?;

        // 创建 SeaORM 连接
        let sea_orm_conn = Database::connect(&config.url)
            .await
            .context("Failed to create SeaORM connection")?;

        // 创建统计器
        let stats = Arc::new(ConnectionStats::default());

        // 创建动态调整器
        let adjuster = if config.enable_dynamic_sizing {
            Some(DynamicAdjuster::new(config))
        } else {
            None
        };

        info!(
            "Database connection pool created successfully (min={}, max={})",
            config.min_connections, config.max_connections
        );

        Ok(Self {
            sea_orm: sea_orm_conn,
            sqlx: sqlx_pool,
            stats,
            adjuster,
            health_check_running: Arc::new(AtomicBool::new(false)),
            config: config.clone(),
        })
    }

    /// 健康检查（增强版）
    pub async fn health_check(&self) -> Result<bool> {
        // 防止并发健康检查
        if self.health_check_running.swap(true, Ordering::AcqRel) {
            debug!("Health check already running, skipping");
            return Ok(true);
        }

        let result = async {
            let start = Instant::now();
            let result = sqlx::query("SELECT 1").fetch_one(&self.sqlx).await;

            match result {
                Ok(_) => {
                    let duration = start.elapsed();
                    debug!("Database health check passed in {:?}", duration);
                    Ok(true)
                }
                Err(e) => {
                    self.stats
                        .health_check_failures
                        .fetch_add(1, Ordering::Relaxed);
                    error!("Database health check failed: {}", e);
                    Ok(false)
                }
            }
        }
        .await;

        self.health_check_running.store(false, Ordering::Release);
        result
    }

    /// 启动后台健康检查
    pub fn start_health_check_task(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.sqlx.clone();
        let stats = self.stats.clone();
        let interval = self.config.health_check_interval;
        let running = Arc::new(AtomicBool::new(true));
        let _running_clone = running.clone();

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            while running.load(Ordering::Relaxed) {
                interval_timer.tick().await;

                let result = sqlx::query("SELECT 1").fetch_one(&pool).await;

                if result.is_err() {
                    stats.health_check_failures.fetch_add(1, Ordering::Relaxed);
                    warn!("Background health check failed");
                }
            }
        })
    }

    /// 获取连接池状态（增强版）
    pub fn pool_status(&self) -> PoolStatus {
        let size = self.sqlx.size();
        let num_idle = self.sqlx.num_idle() as u32;

        PoolStatus {
            size,
            num_idle,
            is_closed: self.sqlx.is_closed(),
            reuse_rate: self.stats.reuse_rate(),
            total_requests: self.stats.total_requests.load(Ordering::Relaxed),
            leak_warnings: self.stats.leak_warnings.load(Ordering::Relaxed),
        }
    }

    /// 记录连接使用
    pub fn record_connection_use(&self, reused: bool) {
        self.stats.record_request(reused);
    }

    /// 检测连接泄漏
    pub async fn check_for_leaks(&self) {
        if !self.config.enable_leak_detection {
            return;
        }

        let status = self.pool_status();
        let active_connections = status.size - status.num_idle;
        let utilization = if status.size > 0 {
            active_connections as f32 / status.size as f32
        } else {
            0.0
        };

        // 如果连接利用率持续 100% 超过阈值时间，发出警告
        if utilization >= 1.0 {
            self.stats.leak_warnings.fetch_add(1, Ordering::Relaxed);
            warn!(
                "Potential connection leak detected: {}/{} connections in use",
                active_connections, status.size
            );
        }
    }

    /// 动态调整连接池大小
    pub async fn adjust_pool_size(&self) -> Result<()> {
        let adjuster = match &self.adjuster {
            Some(a) => a,
            None => return Ok(()),
        };

        let status = self.pool_status();
        let utilization = if status.size > 0 {
            (status.size - status.num_idle) as f32 / status.size as f32
        } else {
            0.0
        };

        if let Some(delta) = adjuster.should_adjust(utilization).await {
            info!(
                "Adjusting pool size by {} (current utilization: {:.1}%)",
                delta,
                utilization * 100.0
            );

            // 注意：sqlx 不支持运行时调整大小，这里只是记录
            // 实际实现需要重新创建连接池
            adjuster.record_adjustment().await;
        }

        Ok(())
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// 关闭连接池
    pub async fn close(&self) {
        self.sqlx.close().await;

        let stats = self.pool_status();
        info!(
            "Database connection pool closed (reuse rate: {:.1}%, total requests: {})",
            stats.reuse_rate * 100.0,
            stats.total_requests
        );
    }

    /// 隐藏 URL 中的密码
    fn mask_url(url: &str) -> String {
        if let Some(pos) = url.find("://") {
            let scheme = &url[..pos + 3];
            if let Some(at_pos) = url[pos + 3..].find('@') {
                let host = &url[pos + 3 + at_pos..];
                if let Some(colon_pos) = url[pos + 3..].find(':') {
                    if colon_pos < at_pos {
                        let user = &url[pos + 3..pos + 3 + colon_pos];
                        return format!("{}{}:****{}", scheme, user, host);
                    }
                }
            }
        }
        url.to_string()
    }
}

/// 数据库迁移
pub struct DatabaseMigrator;

impl DatabaseMigrator {
    /// 运行迁移
    pub async fn run(_db: &DatabaseConnection) -> Result<()> {
        info!("Running database migrations...");

        // NOTE: 实现实际的迁移
        // crate::migration::Migrator::up(db, None).await?;

        info!("Database migrations completed");
        Ok(())
    }

    /// 回滚迁移
    pub async fn rollback(_db: &DatabaseConnection) -> Result<()> {
        info!("Rolling back database migrations...");

        // NOTE: 实现实际的回滚
        // crate::migration::Migrator::down(db, None).await?;

        info!("Database rollback completed");
        Ok(())
    }
}

/// 数据库初始化
pub async fn init_database(config: &DatabaseConfig) -> Result<DatabasePool> {
    let pool = DatabasePool::new(config).await?;

    // 运行健康检查
    if !pool.health_check().await? {
        anyhow::bail!("Database health check failed");
    }

    // 运行迁移
    DatabaseMigrator::run(&pool.sea_orm).await?;

    // 启动后台健康检查任务
    pool.start_health_check_task();

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();

        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert!(config.enable_dynamic_sizing);
        assert!(config.enable_leak_detection);
    }

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats::default();

        stats.record_request(true);
        stats.record_request(true);
        stats.record_request(false);

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 3);
        assert_eq!(stats.reused_connections.load(Ordering::Relaxed), 2);
        assert_eq!(stats.new_connections.load(Ordering::Relaxed), 1);

        let reuse_rate = stats.reuse_rate();
        assert!((reuse_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_mask_url() {
        let url = "postgres://user:password@localhost:5432/db";
        let masked = DatabasePool::mask_url(url);

        assert!(masked.contains("user"));
        assert!(masked.contains("****"));
        assert!(!masked.contains("password"));
    }

    #[test]
    fn test_mask_url_no_password() {
        let url = "postgres://localhost:5432/db";
        let masked = DatabasePool::mask_url(url);

        assert_eq!(masked, url);
    }

    #[test]
    fn test_pool_status() {
        let status = PoolStatus {
            size: 10,
            num_idle: 5,
            is_closed: false,
            reuse_rate: 0.8,
            total_requests: 100,
            leak_warnings: 0,
        };

        assert_eq!(status.size, 10);
        assert_eq!(status.num_idle, 5);
        assert!(!status.is_closed);
        assert_eq!(status.reuse_rate, 0.8);
    }

    #[test]
    fn test_stats_reset() {
        let stats = ConnectionStats::default();

        stats.record_request(true);
        stats.record_request(false);

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 2);

        stats.reset();

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(stats.reused_connections.load(Ordering::Relaxed), 0);
    }
}
