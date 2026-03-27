//! Redis 连接池实现

use anyhow::{Result, Context};
use redis::{Client, AsyncCommands};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error};

/// Redis 配置
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 10,
            timeout: Duration::from_secs(5),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

/// Redis 连接池
pub struct RedisPool {
    client: Arc<Client>,
    config: RedisConfig,
}

impl RedisPool {
    /// 创建新的 Redis 连接池
    pub fn new(config: &RedisConfig) -> Result<Self> {
        info!("Connecting to Redis: {}", config.url);
        
        let client = Client::open(config.url.as_str())
            .context("Failed to create Redis client")?;
        
        info!("Redis client created successfully");
        
        Ok(Self {
            client: Arc::new(client),
            config: config.clone(),
        })
    }
    
    /// 获取异步连接
    pub async fn get_connection(&self) -> Result<redis::aio::Connection> {
        self.client
            .get_async_connection()
            .await
            .context("Failed to get Redis connection")
    }
    
    /// 健康检查
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .context("Redis health check failed")?;
        
        let is_healthy = result == "PONG";
        info!("Redis health check: {}", if is_healthy { "passed" } else { "failed" });
        Ok(is_healthy)
    }
    
    /// 设置键值
    pub async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.get_connection().await?;
        
        if let Some(ttl) = ttl {
            conn.set_ex(key, value, ttl.as_secs())
                .await
                .context("Failed to set Redis key with TTL")?;
        } else {
            conn.set(key, value)
                .await
                .context("Failed to set Redis key")?;
        }
        
        Ok(())
    }
    
    /// 获取键值
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<String> = conn
            .get(key)
            .await
            .context("Failed to get Redis key")?;
        
        Ok(result)
    }
    
    /// 删除键
    pub async fn del(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        
        let result: i32 = conn
            .del(key)
            .await
            .context("Failed to delete Redis key")?;
        
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
        
        let result: i64 = conn
            .ttl(key)
            .await
            .context("Failed to get Redis key TTL")?;
        
        Ok(result)
    }
    
    /// 自增
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        
        let result: i64 = conn
            .incr(key)
            .await
            .context("Failed to increment Redis key")?;
        
        Ok(result)
    }
    
    /// 自减
    pub async fn decr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        
        let result: i64 = conn
            .decr(key)
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
    
    /// 获取客户端
    pub fn client(&self) -> Arc<Client> {
        self.client.clone()
    }
}

/// Redis 初始化
pub async fn init_redis(config: &RedisConfig) -> Result<RedisPool> {
    let pool = RedisPool::new(config)?;
    
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
        assert_eq!(config.retry_attempts, 3);
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
    fn test_redis_config_custom() {
        let config = RedisConfig {
            url: "redis://localhost:6379/0".to_string(),
            pool_size: 20,
            timeout: Duration::from_secs(10),
            retry_attempts: 5,
            retry_delay: Duration::from_millis(200),
        };
        
        assert_eq!(config.pool_size, 20);
        assert_eq!(config.timeout, Duration::from_secs(10));
    }
}
