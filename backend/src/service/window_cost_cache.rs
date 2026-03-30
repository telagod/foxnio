//! Window Cost Cache 服务
//!
//! 窗口期费用缓存，用于优化实时计费查询性能
//! 支持内存缓存、Redis 缓存和批量数据库查询

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::db::redis::RedisPool;
use crate::entity::quota_usage_history;

// ============================================================================
// Prometheus 监控指标
// ============================================================================

lazy_static::lazy_static! {
    /// 缓存命中次数
    static ref WINDOW_CACHE_HITS: prometheus::IntCounter = 
        prometheus::register_int_counter!(prometheus::opts!(
            "foxnio_window_cache_hits_total",
            "Window cost cache hits (memory + Redis)"
        )).unwrap();

    /// 缓存未命中次数
    static ref WINDOW_CACHE_MISSES: prometheus::IntCounter = 
        prometheus::register_int_counter!(prometheus::opts!(
            "foxnio_window_cache_misses_total",
            "Window cost cache misses"
        )).unwrap();

    /// 批量查询次数
    static ref WINDOW_BATCH_QUERIES: prometheus::IntCounter = 
        prometheus::register_int_counter!(prometheus::opts!(
            "foxnio_window_batch_queries_total",
            "Number of batch SQL queries for window cost"
        )).unwrap();

    /// Redis 缓存命中次数
    static ref WINDOW_REDIS_HITS: prometheus::IntCounter = 
        prometheus::register_int_counter!(prometheus::opts!(
            "foxnio_window_redis_hits_total",
            "Window cost Redis cache hits"
        )).unwrap();

    /// Redis 缓存未命中次数
    static ref WINDOW_REDIS_MISSES: prometheus::IntCounter = 
        prometheus::register_int_counter!(prometheus::opts!(
            "foxnio_window_redis_misses_total",
            "Window cost Redis cache misses"
        )).unwrap();

    /// 预取的账户数量
    static ref WINDOW_PREFETCHED_ACCOUNTS: prometheus::IntGauge = 
        prometheus::register_int_gauge!(prometheus::opts!(
            "foxnio_window_prefetched_accounts",
            "Number of accounts prefetched in current batch"
        )).unwrap();
}

/// 窗口期费用缓存
pub struct WindowCostCache {
    /// 本地内存缓存
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// 窗口期时长
    window_duration: Duration,
    /// Redis 连接池（可选）
    redis_pool: Option<Arc<RedisPool>>,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    cost: f64,
    tokens: i64,
    requests: i64,
    expires_at: DateTime<Utc>,
}

/// 批量查询结果
#[derive(Debug, Clone)]
pub struct WindowCostData {
    pub account_id: i64,
    pub cost: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub requests: i64,
}

impl WindowCostCache {
    /// 创建新的窗口期费用缓存
    pub fn new(window_duration: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            window_duration,
            redis_pool: None,
        }
    }

    /// 创建带 Redis 支持的缓存
    pub fn with_redis(window_duration: Duration, redis_pool: Arc<RedisPool>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            window_duration,
            redis_pool: Some(redis_pool),
        }
    }

    /// 获取窗口期费用（优先级：内存缓存 -> Redis -> 返回 None）
    pub async fn get_window_cost(&self, key: &str) -> Option<(f64, i64, i64)> {
        // 1. 先检查内存缓存
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(key) {
                if entry.expires_at > Utc::now() {
                    WINDOW_CACHE_HITS.inc();
                    return Some((entry.cost, entry.tokens, entry.requests));
                }
            }
        }

        // 2. 检查 Redis 缓存
        if let Some(redis) = &self.redis_pool {
            if let Ok(Some(value)) = redis.get(key).await {
                // 尝试解析格式：cost|tokens|requests
                let parts: Vec<&str> = value.split('|').collect();
                if parts.len() == 3 {
                    if let (Ok(cost), Ok(tokens), Ok(requests)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<i64>(),
                        parts[2].parse::<i64>(),
                    ) {
                        WINDOW_REDIS_HITS.inc();
                        WINDOW_CACHE_HITS.inc();

                        // 回填到内存缓存
                        let mut cache = self.cache.write().await;
                        cache.insert(
                            key.to_string(),
                            CacheEntry {
                                cost,
                                tokens,
                                requests,
                                expires_at: Utc::now() + Duration::seconds(60),
                            },
                        );

                        return Some((cost, tokens, requests));
                    }
                }
            }
            WINDOW_REDIS_MISSES.inc();
        }

        WINDOW_CACHE_MISSES.inc();
        None
    }

    /// 设置窗口期费用（同时写入内存和 Redis）
    pub async fn set_window_cost(&self, key: String, cost: f64, tokens: i64, requests: i64) {
        let expires_at = Utc::now() + self.window_duration;

        // 1. 写入内存缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                key.clone(),
                CacheEntry {
                    cost,
                    tokens,
                    requests,
                    expires_at,
                },
            );
        }

        // 2. 写入 Redis 缓存
        if let Some(redis) = &self.redis_pool {
            let value = format!("{}|{}|{}", cost, tokens, requests);
            let _ = redis
                .set(&key, &value, Some(std::time::Duration::from_secs(60)))
                .await;
        }
    }

    /// 批量预取窗口费用（从数据库）
    ///
    /// 从数据库批量查询多个账户的窗口期费用，并写入 Redis 缓存
    pub async fn prefetch_window_costs(
        &self,
        db: &DatabaseConnection,
        account_ids: &[i64],
    ) -> Result<HashMap<i64, WindowCostData>> {
        if account_ids.is_empty() {
            return Ok(HashMap::new());
        }

        WINDOW_BATCH_QUERIES.inc();

        // 计算时间窗口
        let window_start = Utc::now() - Duration::hours(1);

        // 单次 SQL 查询多个账户
        let results: Vec<(i64, f64, i64, i64, i64)> = quota_usage_history::Entity::find()
            .select_only()
            .column(quota_usage_history::Column::AccountId)
            .column_as(
                sea_orm::sea_query::Expr::col(quota_usage_history::Column::Amount).sum(),
                "total_cost",
            )
            .column_as(
                sea_orm::sea_query::Expr::col(quota_usage_history::Column::TokensIn).sum(),
                "total_tokens_in",
            )
            .column_as(
                sea_orm::sea_query::Expr::col(quota_usage_history::Column::TokensOut).sum(),
                "total_tokens_out",
            )
            .column_as(
                sea_orm::sea_query::Expr::col(quota_usage_history::Column::Id).count(),
                "total_requests",
            )
            .filter(
                Condition::all()
                    .add(quota_usage_history::Column::AccountId.is_in(account_ids.to_vec()))
                    .add(quota_usage_history::Column::CreatedAt.gt(window_start)),
            )
            .group_by(quota_usage_history::Column::AccountId)
            .into_tuple::<(i64, f64, i64, i64, i64)>()
            .all(db)
            .await?;

        // 构建结果映射
        let mut result_map = HashMap::new();

        for (account_id, cost, tokens_in, tokens_out, requests) in results {
            result_map.insert(
                account_id,
                WindowCostData {
                    account_id,
                    cost,
                    tokens_in,
                    tokens_out,
                    requests,
                },
            );
        }

        // 写入 Redis 缓存
        if let Some(redis) = &self.redis_pool {
            for (account_id, data) in &result_map {
                let key = format!("window_cost:{}", account_id);
                let value = format!("{}|{}|{}", data.cost, data.tokens_in + data.tokens_out, data.requests);
                let _ = redis
                    .set(&key, &value, Some(std::time::Duration::from_secs(60)))
                    .await;
            }
        }

        // 更新监控指标
        WINDOW_PREFETCHED_ACCOUNTS.set(result_map.len() as i64);

        Ok(result_map)
    }

    /// 从 Redis 获取缓存的窗口费用
    pub async fn get_cached(
        &self,
        redis: &RedisPool,
        account_id: i64,
    ) -> Result<Option<WindowCostData>> {
        let key = format!("window_cost:{}", account_id);

        if let Some(value) = redis.get(&key).await? {
            // 解析格式：cost|tokens|requests
            let parts: Vec<&str> = value.split('|').collect();
            if parts.len() == 3 {
                if let (Ok(cost), Ok(tokens), Ok(requests)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<i64>(),
                    parts[2].parse::<i64>(),
                ) {
                    WINDOW_REDIS_HITS.inc();
                    return Ok(Some(WindowCostData {
                        account_id,
                        cost,
                        tokens_in: tokens / 2, // 近似值
                        tokens_out: tokens / 2,
                        requests,
                    }));
                }
            }
        }

        WINDOW_REDIS_MISSES.inc();
        Ok(None)
    }

    /// 批量获取窗口费用（带缓存）
    ///
    /// 优先从缓存获取，未命中则批量查询数据库
    pub async fn get_or_fetch_window_costs(
        &self,
        db: &DatabaseConnection,
        account_ids: &[i64],
    ) -> Result<HashMap<i64, WindowCostData>> {
        let mut result = HashMap::new();
        let mut uncached_ids = Vec::new();

        // 1. 尝试从缓存获取
        if let Some(redis) = &self.redis_pool {
            for &account_id in account_ids {
                let key = format!("window_cost:{}", account_id);
                if let Ok(Some(value)) = redis.get(&key).await {
                    if let Some(data) = parse_window_cost(account_id, &value) {
                        WINDOW_REDIS_HITS.inc();
                        WINDOW_CACHE_HITS.inc();
                        result.insert(account_id, data);
                        continue;
                    }
                }
                uncached_ids.push(account_id);
                WINDOW_REDIS_MISSES.inc();
            }
        } else {
            uncached_ids = account_ids.to_vec();
        }

        // 2. 批量查询未缓存的账户
        if !uncached_ids.is_empty() {
            let fetched = self.prefetch_window_costs(db, &uncached_ids).await?;
            result.extend(fetched);
        }

        Ok(result)
    }

    /// 清理过期缓存
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// 清空缓存
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// 获取缓存统计
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total = cache.len();
        let active = cache.values().filter(|e| e.expires_at > Utc::now()).count();

        CacheStats {
            total_entries: total,
            active_entries: active,
        }
    }
}

/// 解析窗口费用缓存值
fn parse_window_cost(account_id: i64, value: &str) -> Option<WindowCostData> {
    let parts: Vec<&str> = value.split('|').collect();
    if parts.len() == 3 {
        if let (Ok(cost), Ok(tokens), Ok(requests)) = (
            parts[0].parse::<f64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
        ) {
            return Some(WindowCostData {
                account_id,
                cost,
                tokens_in: tokens / 2,
                tokens_out: tokens / 2,
                requests,
            });
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
}

impl Default for WindowCostCache {
    fn default() -> Self {
        Self::new(Duration::minutes(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_cost_cache_creation() {
        let cache = WindowCostCache::new(Duration::minutes(5));
        assert!(cache.redis_pool.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_window_cost() {
        let cache = WindowCostCache::new(Duration::minutes(5));

        cache
            .set_window_cost("test_key".to_string(), 10.5, 1000, 5)
            .await;

        let result = cache.get_window_cost("test_key").await;
        assert!(result.is_some());

        let (cost, tokens, requests) = result.unwrap();
        assert_eq!(cost, 10.5);
        assert_eq!(tokens, 1000);
        assert_eq!(requests, 5);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let cache = WindowCostCache::new(Duration::seconds(1));

        cache
            .set_window_cost("test_key".to_string(), 10.5, 1000, 5)
            .await;

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

        cache.cleanup_expired().await;

        let result = cache.get_window_cost("test_key").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = WindowCostCache::new(Duration::minutes(5));

        cache
            .set_window_cost("key1".to_string(), 10.0, 100, 1)
            .await;
        cache
            .set_window_cost("key2".to_string(), 20.0, 200, 2)
            .await;

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.active_entries, 2);
    }

    #[test]
    fn test_parse_window_cost() {
        let result = parse_window_cost(123, "10.5|1000|5");
        assert!(result.is_some());

        let data = result.unwrap();
        assert_eq!(data.account_id, 123);
        assert_eq!(data.cost, 10.5);
        assert_eq!(data.tokens_in, 500);
        assert_eq!(data.tokens_out, 500);
        assert_eq!(data.requests, 5);

        // 测试无效格式
        let result = parse_window_cost(123, "invalid");
        assert!(result.is_none());
    }
}
