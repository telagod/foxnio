use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache service for billing data
pub struct BillingCacheService {
    cache: Arc<RwLock<HashMap<i64, CachedBalance>>>,
    config: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedBalance {
    pub user_id: i64,
    pub balance: f64,
    pub cached_at: DateTime<Utc>,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub default_ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            default_ttl_seconds: 60,
        }
    }
}

impl BillingCacheService {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get cached balance
    pub async fn get(&self, user_id: i64) -> Option<f64> {
        let cache = self.cache.read().await;
        cache.get(&user_id).and_then(|entry| {
            let now = Utc::now();
            if (now - entry.cached_at).num_seconds() < entry.ttl_seconds as i64 {
                Some(entry.balance)
            } else {
                None
            }
        })
    }

    /// Set cached balance
    pub async fn set(&self, user_id: i64, balance: f64, ttl_seconds: Option<u64>) {
        let entry = CachedBalance {
            user_id,
            balance,
            cached_at: Utc::now(),
            ttl_seconds: ttl_seconds.unwrap_or(self.config.default_ttl_seconds),
        };

        let mut cache = self.cache.write().await;
        cache.insert(user_id, entry);
    }

    /// Invalidate cache
    pub async fn invalidate(&self, user_id: i64) {
        let mut cache = self.cache.write().await;
        cache.remove(&user_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_billing_cache() {
        let cache = BillingCacheService::new(CacheConfig::default());

        cache.set(1, 100.0, None).await;

        let balance = cache.get(1).await;
        assert_eq!(balance, Some(100.0));

        cache.invalidate(1).await;
        let balance = cache.get(1).await;
        assert!(balance.is_none());
    }
}
