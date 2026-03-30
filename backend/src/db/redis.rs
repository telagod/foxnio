//! Redis 连接池实现 - v0.2.0 增强版
//!
//! 功能增强：
//! - 连接池管理器（connection-manager）
//! - 连接统计和监控
//! - 智能重试机制
//! - 批量操作支持
//! - 本地内存缓存（LRU）

#![allow(dead_code)]
use anyhow::{Context, Result};
use lru::LruCache;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Redis 配置
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,

    // v0.2.0 新增
    pub enable_local_cache: bool,
    pub local_cache_size: usize,
    pub local_cache_ttl: Duration,
    pub enable_stats: bool,
    pub health_check_interval: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 10,
            timeout: Duration::from_secs(5),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(100),

            enable_local_cache: true,
            local_cache_size: 1000,
            local_cache_ttl: Duration::from_secs(60),
            enable_stats: true,
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// Redis 操作统计
#[derive(Debug, Default)]
pub struct RedisStats {
    /// 总请求数
    pub total_requests: AtomicU64,
    /// 缓存命中数
    pub cache_hits: AtomicU64,
    /// 缓存未命中数
    pub cache_misses: AtomicU64,
    /// 错误数
    pub errors: AtomicU64,
    /// 重试次数
    pub retries: AtomicU64,
    /// 平均延迟（毫秒）
    pub total_latency_ms: AtomicU64,
}

impl RedisStats {
    /// 记录请求
    pub fn record_request(&self, latency_ms: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
    }

    /// 记录缓存命中
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存未命中
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录错误
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录重试
    pub fn record_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f32 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            return 0.0;
        }

        hits as f32 / total as f32
    }

    /// 获取平均延迟（毫秒）
    pub fn avg_latency_ms(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);
        total_latency as f64 / total as f64
    }

    /// 重置统计
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.retries.store(0, Ordering::Relaxed);
        self.total_latency_ms.store(0, Ordering::Relaxed);
    }
}

/// 本地缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    expires_at: Instant,
}

impl CacheEntry {
    fn new(value: String, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Redis 连接池（增强版）
#[derive(Clone)]
pub struct RedisPool {
    client: Arc<Client>,
    connection_manager: Arc<RwLock<Option<ConnectionManager>>>,
    config: RedisConfig,
    stats: Arc<RedisStats>,
    local_cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
}

impl RedisPool {
    /// 创建新的 Redis 连接池
    pub fn new(config: &RedisConfig) -> Result<Self> {
        info!("Connecting to Redis: {}", config.url);

        let client = Client::open(config.url.as_str()).context("Failed to create Redis client")?;

        let local_cache = LruCache::new(NonZeroUsize::new(config.local_cache_size).unwrap());

        info!(
            "Redis client created successfully (pool_size={}, local_cache={})",
            config.pool_size, config.enable_local_cache
        );

        Ok(Self {
            client: Arc::new(client),
            connection_manager: Arc::new(RwLock::new(None)),
            config: config.clone(),
            stats: Arc::new(RedisStats::default()),
            local_cache: Arc::new(RwLock::new(local_cache)),
        })
    }

    /// 初始化连接管理器
    pub async fn init_connection_manager(&self) -> Result<()> {
        let manager = self
            .client
            .get_connection_manager()
            .await
            .context("Failed to create Redis connection manager")?;

        let mut cm = self.connection_manager.write().await;
        *cm = Some(manager);

        info!("Redis connection manager initialized");
        Ok(())
    }

    /// 获取连接
    async fn get_connection(&self) -> Result<ConnectionManager> {
        let cm = self.connection_manager.read().await;

        if let Some(manager) = cm.as_ref() {
            Ok(manager.clone())
        } else {
            // 如果没有连接管理器，创建一个新的
            drop(cm);
            self.init_connection_manager().await?;

            let cm = self.connection_manager.read().await;
            Ok(cm.as_ref().unwrap().clone())
        }
    }

    /// 健康检查（带重试）
    pub async fn health_check(&self) -> Result<bool> {
        let mut retries = 0;

        loop {
            match self.try_health_check().await {
                Ok(true) => {
                    debug!("Redis health check passed");
                    return Ok(true);
                }
                Ok(false) | Err(_) => {
                    retries += 1;
                    self.stats.record_error();

                    if retries >= self.config.retry_attempts {
                        error!("Redis health check failed after {} attempts", retries);
                        return Ok(false);
                    }

                    self.stats.record_retry();
                    tokio::time::sleep(self.config.retry_delay).await;
                }
            }
        }
    }

    async fn try_health_check(&self) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .context("Redis PING failed")?;

        Ok(result == "PONG")
    }

    /// 设置键值（带本地缓存）
    pub async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()> {
        let start = Instant::now();

        // 设置 Redis
        let result = self.set_redis(key, value, ttl).await;

        // 更新本地缓存
        if result.is_ok() && self.config.enable_local_cache {
            let cache_ttl = ttl.unwrap_or(self.config.local_cache_ttl);
            let entry = CacheEntry::new(value.to_string(), cache_ttl);

            let mut cache = self.local_cache.write().await;
            cache.put(key.to_string(), entry);
        }

        self.stats
            .record_request(start.elapsed().as_millis() as u64);
        result
    }

    async fn set_redis(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.get_connection().await?;

        if let Some(ttl) = ttl {
            conn.set_ex::<_, _, ()>(key, value, ttl.as_secs())
                .await
                .context("Failed to set Redis key with TTL")?;
        } else {
            conn.set::<_, _, ()>(key, value)
                .await
                .context("Failed to set Redis key")?;
        }

        Ok(())
    }

    /// 获取键值（带本地缓存）
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let start = Instant::now();

        // 先检查本地缓存
        if self.config.enable_local_cache {
            let mut cache = self.local_cache.write().await;

            if let Some(entry) = cache.get(key) {
                if !entry.is_expired() {
                    self.stats.record_cache_hit();
                    self.stats
                        .record_request(start.elapsed().as_millis() as u64);
                    return Ok(Some(entry.value.clone()));
                }
                // 过期，删除
                cache.pop(key);
            }
        }

        // 本地缓存未命中，从 Redis 获取
        let result = self.get_redis(key).await;

        // 更新本地缓存
        if let Ok(Some(ref value)) = result {
            if self.config.enable_local_cache {
                let entry = CacheEntry::new(value.clone(), self.config.local_cache_ttl);
                let mut cache = self.local_cache.write().await;
                cache.put(key.to_string(), entry);
            }
            self.stats.record_cache_miss();
        }

        self.stats
            .record_request(start.elapsed().as_millis() as u64);
        result
    }

    async fn get_redis(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;

        let result: Option<String> = conn.get(key).await.context("Failed to get Redis key")?;

        Ok(result)
    }

    /// 删除键（同时清除本地缓存）
    pub async fn del(&self, key: &str) -> Result<bool> {
        let start = Instant::now();

        // 删除本地缓存
        if self.config.enable_local_cache {
            let mut cache = self.local_cache.write().await;
            cache.pop(key);
        }

        // 删除 Redis
        let mut conn = self.get_connection().await?;
        let result: i32 = conn.del(key).await.context("Failed to delete Redis key")?;

        self.stats
            .record_request(start.elapsed().as_millis() as u64);
        Ok(result > 0)
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: bool = conn
            .exists(key)
            .await
            .context("Failed to check Redis key existence")?;

        Ok(result)
    }

    /// 设置过期时间
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: bool = conn
            .expire(key, ttl.as_secs() as i64)
            .await
            .context("Failed to set Redis key TTL")?;

        Ok(result)
    }

    /// 获取剩余过期时间
    pub async fn ttl(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;

        let result: i64 = conn.ttl(key).await.context("Failed to get Redis key TTL")?;

        Ok(result)
    }

    /// 自增
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;

        let result: i64 = conn
            .incr(key, 1)
            .await
            .context("Failed to increment Redis key")?;

        Ok(result)
    }

    /// 自减
    pub async fn decr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;

        let result: i64 = conn
            .decr(key, 1)
            .await
            .context("Failed to decrement Redis key")?;

        Ok(result)
    }

    /// 哈希设置
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: bool = conn
            .hset(key, field, value)
            .await
            .context("Failed to set Redis hash field")?;

        Ok(result)
    }

    /// 哈希获取
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;

        let result: Option<String> = conn
            .hget(key, field)
            .await
            .context("Failed to get Redis hash field")?;

        Ok(result)
    }

    /// 哈希获取所有字段
    pub async fn hgetall(&self, key: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut conn = self.get_connection().await?;

        let result: std::collections::HashMap<String, String> = conn
            .hgetall(key)
            .await
            .context("Failed to get all Redis hash fields")?;

        Ok(result)
    }

    /// 添加到集合
    pub async fn sadd(&self, key: &str, member: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: bool = conn
            .sadd(key, member)
            .await
            .context("Failed to add to Redis set")?;

        Ok(result)
    }

    /// 检查集合成员
    pub async fn sismember(&self, key: &str, member: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;

        let result: bool = conn
            .sismember(key, member)
            .await
            .context("Failed to check Redis set membership")?;

        Ok(result)
    }

    /// 执行 Lua 脚本
    pub async fn eval(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<redis::Value> {
        let mut conn = self.get_connection().await?;

        let result: redis::Value = redis::cmd("EVAL")
            .arg(script)
            .arg(keys.len())
            .arg(keys)
            .arg(args)
            .query_async(&mut conn)
            .await
            .context("Failed to execute Redis Lua script")?;

        Ok(result)
    }

    /// 批量设置
    pub async fn mset(&self, items: &[(String, String)]) -> Result<()> {
        let mut conn = self.get_connection().await?;

        conn.mset::<_, _, ()>(items)
            .await
            .context("Failed to batch set Redis keys")?;

        // 更新本地缓存
        if self.config.enable_local_cache {
            let mut cache = self.local_cache.write().await;
            for (key, value) in items {
                let entry = CacheEntry::new(value.clone(), self.config.local_cache_ttl);
                cache.put(key.clone(), entry);
            }
        }

        Ok(())
    }

    /// 批量获取
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        let mut conn = self.get_connection().await?;

        let result: Vec<Option<String>> = conn
            .mget(keys)
            .await
            .context("Failed to batch get Redis keys")?;

        Ok(result)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &RedisStats {
        &self.stats
    }

    /// 清除本地缓存
    pub async fn clear_local_cache(&self) {
        let mut cache = self.local_cache.write().await;
        cache.clear();
        info!("Local cache cleared");
    }

    /// 获取客户端
    pub fn client(&self) -> Arc<Client> {
        self.client.clone()
    }
}

/// Redis 初始化
pub async fn init_redis(config: &RedisConfig) -> Result<RedisPool> {
    let pool = RedisPool::new(config)?;

    // 初始化连接管理器
    pool.init_connection_manager().await?;

    // 运行健康检查
    if !pool.health_check().await? {
        anyhow::bail!("Redis health check failed");
    }

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();

        assert_eq!(config.pool_size, 10);
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert!(config.enable_local_cache);
        assert!(config.enable_stats);
    }

    #[test]
    fn test_redis_stats() {
        let stats = RedisStats::default();

        stats.record_request(10);
        stats.record_cache_hit();
        stats.record_cache_miss();
        stats.record_error();

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(stats.cache_hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.cache_misses.load(Ordering::Relaxed), 1);
        assert_eq!(stats.errors.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_cache_hit_rate() {
        let stats = RedisStats::default();

        stats.record_cache_hit();
        stats.record_cache_hit();
        stats.record_cache_miss();

        let rate = stats.cache_hit_rate();
        assert!((rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_cache_entry_expiry() {
        let entry = CacheEntry::new("value".to_string(), Duration::from_millis(100));

        assert!(!entry.is_expired());

        std::thread::sleep(Duration::from_millis(150));

        assert!(entry.is_expired());
    }

    #[test]
    fn test_redis_pool_creation() {
        let config = RedisConfig {
            url: "redis://127.0.0.1:6379".to_string(),
            ..Default::default()
        };

        let pool = RedisPool::new(&config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_stats_reset() {
        let stats = RedisStats::default();

        stats.record_request(10);
        stats.record_cache_hit();

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 1);

        stats.reset();

        assert_eq!(stats.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(stats.cache_hits.load(Ordering::Relaxed), 0);
    }
}
