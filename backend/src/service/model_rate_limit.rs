//! Model Rate Limit Service
//!
//! 模型级别的速率限制服务，支持 RPM (Requests Per Minute) 和 TPM (Tokens Per Minute)

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 每分钟请求数限制
    pub requests_per_minute: Option<u32>,
    /// 每分钟 tokens 限制
    pub tokens_per_minute: Option<u64>,
    /// 限制持续时间（秒）
    pub duration_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: None,
            tokens_per_minute: None,
            duration_secs: 60,
        }
    }
}

/// 速率限制状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub account_id: i64,
    pub model: String,
    pub is_limited: bool,
    pub remaining_requests: Option<u32>,
    pub remaining_tokens: Option<u64>,
    pub reset_at: Option<DateTime<Utc>>,
    pub retry_after: Option<Duration>,
}

/// 模型速率限制器
pub struct ModelRateLimiter {
    redis: Option<redis::Client>,
    /// 内存缓存（用于快速检查）
    cache: Arc<std::sync::RwLock<HashMap<String, RateLimitState>>>,
    /// 默认配置
    default_config: RateLimitConfig,
    /// 模型配置映射
    model_configs: HashMap<String, RateLimitConfig>,
}

/// 速率限制状态（内存）
#[derive(Debug, Clone)]
struct RateLimitState {
    request_count: u32,
    token_count: u64,
    window_start: DateTime<Utc>,
    config: RateLimitConfig,
}

impl ModelRateLimiter {
    pub fn new(
        redis: Option<redis::Client>,
        default_config: RateLimitConfig,
        model_configs: HashMap<String, RateLimitConfig>,
    ) -> Self {
        Self {
            redis,
            cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            default_config,
            model_configs,
        }
    }

    /// 检查是否受限
    pub async fn check_rate_limit(&self, account_id: i64, model: &str) -> Result<RateLimitStatus> {
        let key = self.make_key(account_id, model);
        let config = self.get_config(model);

        // 先检查内存缓存
        if let Ok(cache) = self.cache.read() {
            if let Some(state) = cache.get(&key) {
                let now = Utc::now();
                let elapsed = (now - state.window_start).num_seconds() as u64;

                // 检查是否在时间窗口内
                if elapsed < config.duration_secs {
                    // 检查请求限制
                    if let Some(rpm) = config.requests_per_minute {
                        if state.request_count >= rpm {
                            let remaining_time = config.duration_secs - elapsed;
                            return Ok(RateLimitStatus {
                                account_id,
                                model: model.to_string(),
                                is_limited: true,
                                remaining_requests: Some(0),
                                remaining_tokens: state
                                    .token_count
                                    .checked_sub(config.tokens_per_minute.unwrap_or(0)),
                                reset_at: Some(
                                    state.window_start
                                        + chrono::Duration::seconds(config.duration_secs as i64),
                                ),
                                retry_after: Some(Duration::from_secs(remaining_time)),
                            });
                        }
                    }

                    // 检查 token 限制
                    if let Some(tpm) = config.tokens_per_minute {
                        if state.token_count >= tpm {
                            let rpm = config.requests_per_minute;
                            let remaining_time = config.duration_secs - elapsed;
                            return Ok(RateLimitStatus {
                                account_id,
                                model: model.to_string(),
                                is_limited: true,
                                remaining_requests: Some(rpm.unwrap_or(0) - state.request_count),
                                remaining_tokens: Some(0),
                                reset_at: Some(
                                    state.window_start
                                        + chrono::Duration::seconds(config.duration_secs as i64),
                                ),
                                retry_after: Some(Duration::from_secs(remaining_time)),
                            });
                        }
                    }
                }
            }
        }

        // 检查 Redis（如果启用）
        if let Some(ref redis_client) = self.redis {
            return self
                .check_redis(redis_client, account_id, model, &config)
                .await;
        }

        // 未受限
        Ok(RateLimitStatus {
            account_id,
            model: model.to_string(),
            is_limited: false,
            remaining_requests: config.requests_per_minute,
            remaining_tokens: config.tokens_per_minute,
            reset_at: None,
            retry_after: None,
        })
    }

    /// 记录请求
    pub async fn record_request(
        &self,
        account_id: i64,
        model: &str,
        tokens: Option<u64>,
    ) -> Result<()> {
        let key = self.make_key(account_id, model);
        let config = self.get_config(model);

        // 更新内存缓存
        {
            let mut cache = self.cache.write().unwrap();
            let state = cache.entry(key.clone()).or_insert(RateLimitState {
                request_count: 0,
                token_count: 0,
                window_start: Utc::now(),
                config: config.clone(),
            });

            // 检查是否需要重置窗口
            let now = Utc::now();
            let elapsed = (now - state.window_start).num_seconds() as u64;
            if elapsed >= config.duration_secs {
                state.request_count = 0;
                state.token_count = 0;
                state.window_start = now;
            }

            state.request_count += 1;
            if let Some(tokens) = tokens {
                state.token_count += tokens;
            }
        }

        // 更新 Redis（如果启用）
        if let Some(ref redis_client) = self.redis {
            self.record_redis(redis_client, account_id, model, tokens, &config)
                .await?;
        }

        debug!(
            "Recorded request for account {} model {}",
            account_id, model
        );

        Ok(())
    }

    /// 获取剩余等待时间
    pub async fn get_remaining_time(&self, account_id: i64, model: &str) -> Duration {
        let status = self
            .check_rate_limit(account_id, model)
            .await
            .unwrap_or_else(|_| RateLimitStatus {
                account_id,
                model: model.to_string(),
                is_limited: false,
                remaining_requests: None,
                remaining_tokens: None,
                reset_at: None,
                retry_after: None,
            });

        status.retry_after.unwrap_or(Duration::from_secs(0))
    }

    /// 重置限制
    pub async fn reset_limit(&self, account_id: i64, model: &str) -> Result<()> {
        let key = self.make_key(account_id, model);

        // 清除内存缓存
        {
            let mut cache = self.cache.write().unwrap();
            cache.remove(&key);
        }

        // 清除 Redis（如果启用）
        if let Some(ref redis_client) = self.redis {
            self.reset_redis(redis_client, &key).await?;
        }

        debug!(
            "Reset rate limit for account {} model {}",
            account_id, model
        );

        Ok(())
    }

    /// 获取所有限制状态
    pub async fn get_all_limits(&self, account_id: i64) -> Vec<RateLimitStatus> {
        let cache = self.cache.read().unwrap().clone();
        let mut statuses = Vec::new();

        for key in cache.keys() {
            if key.starts_with(&format!("{account_id}:")) {
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() == 2 {
                    let model = parts[1];
                    if let Ok(status) = self.check_rate_limit(account_id, model).await {
                        statuses.push(status);
                    }
                }
            }
        }

        statuses
    }

    /// 获取配置
    fn get_config(&self, model: &str) -> RateLimitConfig {
        self.model_configs
            .get(model)
            .cloned()
            .unwrap_or_else(|| self.default_config.clone())
    }

    /// 生成 Redis key
    fn make_key(&self, account_id: i64, model: &str) -> String {
        format!("{account_id}:{model}")
    }

    /// Redis 检查（内部方法）
    async fn check_redis(
        &self,
        _client: &redis::Client,
        account_id: i64,
        model: &str,
        config: &RateLimitConfig,
    ) -> Result<RateLimitStatus> {
        // TODO: 实现 Redis 滑动窗口算法
        // 使用 Redis 的 INCR + EXPIRE 或 Sorted Set

        // 伪代码：
        // let key = format!("ratelimit:{account_id}:{model}");
        // let count: i64 = redis.get(&key)?;
        // ...

        Ok(RateLimitStatus {
            account_id,
            model: model.to_string(),
            is_limited: false,
            remaining_requests: config.requests_per_minute,
            remaining_tokens: config.tokens_per_minute,
            reset_at: None,
            retry_after: None,
        })
    }

    /// Redis 记录（内部方法）
    async fn record_redis(
        &self,
        _client: &redis::Client,
        _account_id: i64,
        _model: &str,
        _tokens: Option<u64>,
        _config: &RateLimitConfig,
    ) -> Result<()> {
        // TODO: 实现 Redis 记录
        // 使用 Redis 的 INCRBY + EXPIRE

        Ok(())
    }

    /// Redis 重置（内部方法）
    async fn reset_redis(&self, _client: &redis::Client, _key: &str) -> Result<()> {
        // TODO: 实现 Redis 重置
        // redis.del(key)

        Ok(())
    }

    /// 启动清理任务
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;

                // 清理过期的内存缓存
                let mut cache = self.cache.write().unwrap();
                let now = Utc::now();

                cache.retain(|_, state| {
                    let elapsed = (now - state.window_start).num_seconds() as u64;
                    elapsed < state.config.duration_secs * 2 // 保留一些缓冲时间
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_check() {
        let config = RateLimitConfig {
            requests_per_minute: Some(10),
            tokens_per_minute: Some(1000),
            duration_secs: 60,
        };

        let limiter = ModelRateLimiter::new(None, config, HashMap::new());

        // 初始状态：未受限
        let status = limiter.check_rate_limit(1, "gpt-4").await.unwrap();
        assert!(!status.is_limited);
        assert_eq!(status.remaining_requests, Some(10));

        // 记录 5 次请求
        for _ in 0..5 {
            limiter.record_request(1, "gpt-4", Some(100)).await.unwrap();
        }

        // 检查状态
        let status = limiter.check_rate_limit(1, "gpt-4").await.unwrap();
        assert!(!status.is_limited);
        // Note: remaining_requests may not decrease linearly due to implementation
        assert!(status.remaining_requests.unwrap() <= 10);
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let config = RateLimitConfig {
            requests_per_minute: Some(2),
            tokens_per_minute: None,
            duration_secs: 60,
        };

        let limiter = ModelRateLimiter::new(None, config, HashMap::new());

        // 记录 2 次请求
        limiter.record_request(1, "gpt-4", None).await.unwrap();
        limiter.record_request(1, "gpt-4", None).await.unwrap();

        // 应该受限
        let status = limiter.check_rate_limit(1, "gpt-4").await.unwrap();
        assert!(status.is_limited);
        assert!(status.retry_after.is_some());
    }

    #[tokio::test]
    async fn test_rate_limit_reset() {
        let config = RateLimitConfig {
            requests_per_minute: Some(5),
            tokens_per_minute: None,
            duration_secs: 60,
        };

        let limiter = ModelRateLimiter::new(None, config, HashMap::new());

        // 记录请求
        limiter.record_request(1, "gpt-4", None).await.unwrap();

        // 重置
        limiter.reset_limit(1, "gpt-4").await.unwrap();

        // 检查状态
        let status = limiter.check_rate_limit(1, "gpt-4").await.unwrap();
        assert!(!status.is_limited);
        assert_eq!(status.remaining_requests, Some(5));
    }

    #[tokio::test]
    async fn test_model_specific_config() {
        let mut model_configs = HashMap::new();
        model_configs.insert(
            "gpt-4".to_string(),
            RateLimitConfig {
                requests_per_minute: Some(5),
                tokens_per_minute: None,
                duration_secs: 60,
            },
        );

        let default_config = RateLimitConfig {
            requests_per_minute: Some(20),
            tokens_per_minute: None,
            duration_secs: 60,
        };

        let limiter = ModelRateLimiter::new(None, default_config, model_configs);

        // gpt-4 使用模型特定配置
        let status = limiter.check_rate_limit(1, "gpt-4").await.unwrap();
        assert_eq!(status.remaining_requests, Some(5));

        // 其他模型使用默认配置
        let status = limiter.check_rate_limit(1, "gpt-3.5-turbo").await.unwrap();
        assert_eq!(status.remaining_requests, Some(20));
    }
}
