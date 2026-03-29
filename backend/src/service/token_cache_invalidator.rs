//! Token 缓存失效服务
//!
//! 管理 Token 缓存的失效和更新

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::token_cache_key::TokenCacheKey;

/// 失效原因
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationReason {
    TokenExpired,
    TokenRevoked,
    AccountUpdated,
    ManualInvalidation,
    ErrorDetected,
}

/// 失效事件
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub key: TokenCacheKey,
    pub reason: InvalidationReason,
    pub timestamp: DateTime<Utc>,
    pub details: Option<String>,
}

/// 失效策略
#[derive(Debug, Clone)]
pub struct InvalidationPolicy {
    pub max_age_seconds: u64,
    pub refresh_before_expiry_seconds: u64,
    pub invalidate_on_error: bool,
    pub invalidate_on_revocation: bool,
}

impl Default for InvalidationPolicy {
    fn default() -> Self {
        Self {
            max_age_seconds: 3600,              // 1 小时
            refresh_before_expiry_seconds: 300, // 5 分钟前刷新
            invalidate_on_error: true,
            invalidate_on_revocation: true,
        }
    }
}

/// 失效统计
#[derive(Debug, Clone, Default)]
pub struct InvalidationStats {
    pub total_invalidations: u64,
    pub expired_invalidations: u64,
    pub revoked_invalidations: u64,
    pub error_invalidations: u64,
    pub manual_invalidations: u64,
}

/// Token 缓存失效服务
pub struct TokenCacheInvalidator {
    policy: InvalidationPolicy,
    invalidation_log: Arc<RwLock<Vec<InvalidationEvent>>>,
    stats: Arc<RwLock<InvalidationStats>>,
    pending_invalidations: Arc<RwLock<HashSet<String>>>,
}

impl TokenCacheInvalidator {
    /// 创建新的失效服务
    pub fn new(policy: InvalidationPolicy) -> Self {
        Self {
            policy,
            invalidation_log: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(InvalidationStats::default())),
            pending_invalidations: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 使 Token 失效
    pub async fn invalidate(
        &self,
        key: &TokenCacheKey,
        reason: InvalidationReason,
        details: Option<String>,
    ) {
        // 记录失效事件
        let event = InvalidationEvent {
            key: key.clone(),
            reason: reason.clone(),
            timestamp: Utc::now(),
            details,
        };

        {
            let mut log = self.invalidation_log.write().await;
            log.push(event);

            // 限制日志大小
            if log.len() > 10000 {
                log.remove(0);
            }
        }

        // 添加到待失效集合
        {
            let mut pending = self.pending_invalidations.write().await;
            pending.insert(key.to_string());
        }

        // 更新统计
        {
            let mut stats = self.stats.write().await;
            stats.total_invalidations += 1;

            match reason {
                InvalidationReason::TokenExpired => stats.expired_invalidations += 1,
                InvalidationReason::TokenRevoked => stats.revoked_invalidations += 1,
                InvalidationReason::ErrorDetected => stats.error_invalidations += 1,
                InvalidationReason::ManualInvalidation => stats.manual_invalidations += 1,
                _ => {}
            }
        }
    }

    /// 批量失效
    pub async fn invalidate_batch(
        &self,
        keys: &[TokenCacheKey],
        reason: InvalidationReason,
        details: Option<String>,
    ) {
        for key in keys {
            self.invalidate(key, reason.clone(), details.clone()).await;
        }
    }

    /// 检查是否需要失效
    pub fn should_invalidate(&self, created_at: DateTime<Utc>) -> bool {
        let age = (Utc::now() - created_at).num_seconds() as u64;
        age >= self.policy.max_age_seconds
    }

    /// 检查是否需要刷新
    pub fn should_refresh(&self, expires_at: DateTime<Utc>) -> bool {
        let time_until_expiry = (expires_at - Utc::now()).num_seconds() as u64;
        time_until_expiry <= self.policy.refresh_before_expiry_seconds
    }

    /// 获取待失效的键
    pub async fn get_pending_invalidations(&self) -> Vec<String> {
        let pending = self.pending_invalidations.read().await;
        pending.iter().cloned().collect()
    }

    /// 清除待失效记录
    pub async fn clear_pending(&self, key: &str) {
        let mut pending = self.pending_invalidations.write().await;
        pending.remove(key);
    }

    /// 获取失效日志
    pub async fn get_invalidation_log(&self, limit: usize) -> Vec<InvalidationEvent> {
        let log = self.invalidation_log.write().await;
        log.iter().rev().take(limit).cloned().collect()
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> InvalidationStats {
        self.stats.read().await.clone()
    }

    /// 清空失效日志
    pub async fn clear_log(&self) {
        let mut log = self.invalidation_log.write().await;
        log.clear();
    }

    /// 重置统计
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = InvalidationStats::default();
    }

    /// 检查键是否已失效
    pub async fn is_invalidated(&self, key: &str) -> bool {
        let pending = self.pending_invalidations.read().await;
        pending.contains(key)
    }
}

impl Default for TokenCacheInvalidator {
    fn default() -> Self {
        Self::new(InvalidationPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalidate() {
        let invalidator = TokenCacheInvalidator::default();
        let key = TokenCacheKey::new("openai", "account1");

        invalidator
            .invalidate(
                &key,
                InvalidationReason::TokenExpired,
                Some("Token expired".to_string()),
            )
            .await;

        let stats = invalidator.get_stats().await;
        assert_eq!(stats.total_invalidations, 1);
        assert_eq!(stats.expired_invalidations, 1);
    }

    #[tokio::test]
    async fn test_should_invalidate() {
        let invalidator = TokenCacheInvalidator::default();

        let old_time = Utc::now() - chrono::Duration::hours(2);
        assert!(invalidator.should_invalidate(old_time));

        let recent_time = Utc::now() - chrono::Duration::minutes(30);
        assert!(!invalidator.should_invalidate(recent_time));
    }

    #[tokio::test]
    async fn test_should_refresh() {
        let invalidator = TokenCacheInvalidator::default();

        let soon_expiry = Utc::now() + chrono::Duration::minutes(2);
        assert!(invalidator.should_refresh(soon_expiry));

        let far_expiry = Utc::now() + chrono::Duration::hours(1);
        assert!(!invalidator.should_refresh(far_expiry));
    }

    #[tokio::test]
    async fn test_is_invalidated() {
        let invalidator = TokenCacheInvalidator::default();
        let key = TokenCacheKey::new("openai", "account1");

        assert!(!invalidator.is_invalidated(&key.to_string()).await);

        invalidator
            .invalidate(&key, InvalidationReason::ManualInvalidation, None)
            .await;

        assert!(invalidator.is_invalidated(&key.to_string()).await);
    }

    #[tokio::test]
    async fn test_batch_invalidation() {
        let invalidator = TokenCacheInvalidator::default();
        let keys = vec![
            TokenCacheKey::new("openai", "account1"),
            TokenCacheKey::new("openai", "account2"),
        ];

        invalidator
            .invalidate_batch(&keys, InvalidationReason::AccountUpdated, None)
            .await;

        let stats = invalidator.get_stats().await;
        assert_eq!(stats.total_invalidations, 2);
    }
}
