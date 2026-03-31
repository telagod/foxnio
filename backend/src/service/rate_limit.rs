//! Redis 速率限制实现

#![allow(dead_code)]
use anyhow::Result;
use redis::{AsyncCommands, Client};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// 速率限制配置
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// 时间窗口（秒）
    pub window_seconds: u64,
    /// 最大请求数
    pub max_requests: u64,
    /// 是否启用
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            window_seconds: 60,
            max_requests: 60,
            enabled: true,
        }
    }
}

/// 速率限制结果
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub current_count: u64,
    pub limit: u64,
    pub reset_after: u64,
    pub retry_after: Option<u64>,
}

/// Redis 速率限制器
#[derive(Clone)]
pub struct RedisRateLimiter {
    client: Client,
    config: RateLimitConfig,
    // 本地计数器缓存（用于降级）
    local_counters: Arc<RwLock<Vec<(String, u64, std::time::Instant)>>>,
}

impl RedisRateLimiter {
    pub fn new(redis_url: &str, config: RateLimitConfig) -> Result<Self> {
        let client = Client::open(redis_url)?;

        Ok(Self {
            client,
            config,
            local_counters: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 检查速率限制
    pub async fn check_rate_limit(&self, key: &str) -> Result<RateLimitResult> {
        if !self.config.enabled {
            return Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit: self.config.max_requests,
                reset_after: self.config.window_seconds,
                retry_after: None,
            });
        }

        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // 使用 Redis INCR + EXPIRE 实现滑动窗口
        let redis_key = format!("ratelimit:{key}");

        // 获取当前计数
        let current: u64 = conn.get(&redis_key).await.unwrap_or(0);

        if current >= self.config.max_requests {
            // 获取剩余时间
            let ttl: i64 = conn
                .ttl(&redis_key)
                .await
                .unwrap_or(self.config.window_seconds as i64);

            return Ok(RateLimitResult {
                allowed: false,
                current_count: current,
                limit: self.config.max_requests,
                reset_after: ttl as u64,
                retry_after: Some(ttl as u64),
            });
        }

        // 增加计数
        let new_count: u64 = conn.incr(&redis_key, 1).await?;

        // 如果是第一次访问，设置过期时间
        if new_count == 1 {
            let _: () = conn
                .expire(&redis_key, self.config.window_seconds as i64)
                .await?;
        }

        Ok(RateLimitResult {
            allowed: true,
            current_count: new_count,
            limit: self.config.max_requests,
            reset_after: self.config.window_seconds,
            retry_after: None,
        })
    }

    /// 使用 Lua 脚本的原子性速率限制检查
    pub async fn check_rate_limit_atomic(&self, key: &str) -> Result<RateLimitResult> {
        if !self.config.enabled {
            return Ok(RateLimitResult {
                allowed: true,
                current_count: 0,
                limit: self.config.max_requests,
                reset_after: self.config.window_seconds,
                retry_after: None,
            });
        }

        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Lua 脚本：原子性地检查和增加计数
        let lua_script = r#"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window = tonumber(ARGV[2])
            
            local current = redis.call('GET', key)
            if current == false then
                current = 0
            else
                current = tonumber(current)
            end
            
            if current >= limit then
                local ttl = redis.call('TTL', key)
                return {0, current, limit, ttl}
            end
            
            local new_count = redis.call('INCR', key)
            if new_count == 1 then
                redis.call('EXPIRE', key, window)
            end
            
            local ttl = redis.call('TTL', key)
            return {1, new_count, limit, ttl}
        "#;

        let redis_key = format!("ratelimit:{key}");

        let result: (i64, u64, u64, i64) = redis::cmd("EVAL")
            .arg(lua_script)
            .arg(1)
            .arg(&redis_key)
            .arg(self.config.max_requests)
            .arg(self.config.window_seconds)
            .query_async(&mut conn)
            .await?;

        let (allowed, current_count, limit, ttl) = result;

        Ok(RateLimitResult {
            allowed: allowed == 1,
            current_count,
            limit,
            reset_after: if ttl > 0 {
                ttl as u64
            } else {
                self.config.window_seconds
            },
            retry_after: if allowed == 0 { Some(ttl as u64) } else { None },
        })
    }

    /// 重置速率限制计数器
    pub async fn reset(&self, key: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let redis_key = format!("ratelimit:{key}");
        let _: () = conn.del(&redis_key).await?;
        Ok(())
    }

    /// 获取当前计数
    pub async fn get_current_count(&self, key: &str) -> Result<u64> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let redis_key = format!("ratelimit:{key}");
        let count: u64 = conn.get(&redis_key).await.unwrap_or(0);
        Ok(count)
    }

    /// 本地降级速率限制（当 Redis 不可用时）
    pub async fn check_rate_limit_local(&self, key: &str) -> RateLimitResult {
        let mut counters = self.local_counters.write().await;
        let now = std::time::Instant::now();

        // 清理过期的计数器
        counters.retain(|(_, _, timestamp)| {
            now.duration_since(*timestamp) < Duration::from_secs(self.config.window_seconds)
        });

        // 统计当前 key 的请求数
        let current_count = counters.iter().filter(|(k, _, _)| k == key).count() as u64;

        if current_count >= self.config.max_requests {
            return RateLimitResult {
                allowed: false,
                current_count,
                limit: self.config.max_requests,
                reset_after: self.config.window_seconds,
                retry_after: Some(self.config.window_seconds),
            };
        }

        // 添加新的请求记录
        counters.push((key.to_string(), 1, now));

        RateLimitResult {
            allowed: true,
            current_count: current_count + 1,
            limit: self.config.max_requests,
            reset_after: self.config.window_seconds,
            retry_after: None,
        }
    }
}

/// 分布式速率限制器（支持多层级）
pub struct DistributedRateLimiter {
    redis_limiter: Option<RedisRateLimiter>,
    user_limits: HashMap<String, RateLimitConfig>,
    api_key_limits: HashMap<String, RateLimitConfig>,
}

impl DistributedRateLimiter {
    pub fn new(redis_limiter: Option<RedisRateLimiter>) -> Self {
        Self {
            redis_limiter,
            user_limits: HashMap::new(),
            api_key_limits: HashMap::new(),
        }
    }

    pub fn with_user_limit(mut self, user_id: &str, config: RateLimitConfig) -> Self {
        self.user_limits.insert(user_id.to_string(), config);
        self
    }

    pub fn with_api_key_limit(mut self, api_key: &str, config: RateLimitConfig) -> Self {
        self.api_key_limits.insert(api_key.to_string(), config);
        self
    }

    /// 检查用户级别的速率限制
    pub async fn check_user_limit(&self, user_id: &str) -> Result<RateLimitResult> {
        let config = self.user_limits.get(user_id).cloned().unwrap_or_default();

        if let Some(ref limiter) = self.redis_limiter {
            let mut limiter = limiter.clone();
            limiter.config = config;
            limiter.check_rate_limit(&format!("user:{user_id}")).await
        } else {
            Ok(self
                .check_local_limit(&format!("user:{user_id}"), config)
                .await)
        }
    }

    /// 检查 API Key 级别的速率限制
    pub async fn check_api_key_limit(&self, api_key_id: &str) -> Result<RateLimitResult> {
        let config = self
            .api_key_limits
            .get(api_key_id)
            .cloned()
            .unwrap_or_default();

        if let Some(ref limiter) = self.redis_limiter {
            let mut limiter = limiter.clone();
            limiter.config = config;
            limiter
                .check_rate_limit(&format!("apikey:{api_key_id}"))
                .await
        } else {
            Ok(self
                .check_local_limit(&format!("apikey:{api_key_id}"), config)
                .await)
        }
    }

    async fn check_local_limit(&self, _key: &str, config: RateLimitConfig) -> RateLimitResult {
        // 简化的本地限制
        RateLimitResult {
            allowed: true,
            current_count: 0,
            limit: config.max_requests,
            reset_after: config.window_seconds,
            retry_after: None,
        }
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();

        assert_eq!(config.window_seconds, 60);
        assert_eq!(config.max_requests, 60);
        assert!(config.enabled);
    }

    #[test]
    fn test_rate_limit_result_allowed() {
        let result = RateLimitResult {
            allowed: true,
            current_count: 10,
            limit: 60,
            reset_after: 60,
            retry_after: None,
        };

        assert!(result.allowed);
        assert!(result.retry_after.is_none());
    }

    #[test]
    fn test_rate_limit_result_blocked() {
        let result = RateLimitResult {
            allowed: false,
            current_count: 60,
            limit: 60,
            reset_after: 30,
            retry_after: Some(30),
        };

        assert!(!result.allowed);
        assert_eq!(result.retry_after, Some(30));
    }

    #[test]
    fn test_rate_limit_config_custom() {
        let config = RateLimitConfig {
            window_seconds: 120,
            max_requests: 100,
            enabled: false,
        };

        assert_eq!(config.window_seconds, 120);
        assert_eq!(config.max_requests, 100);
        assert!(!config.enabled);
    }
}
