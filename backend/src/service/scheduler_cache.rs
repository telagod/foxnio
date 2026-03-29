use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache for scheduler
pub struct SchedulerCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    config: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
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
            default_ttl_seconds: 3600,
        }
    }
}

impl SchedulerCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get from cache
    pub async fn get(&self, key: &str) -> Option<CacheEntry> {
        let cache = self.cache.read().await;
        cache.get(key).and_then(|entry| {
            if entry.expires_at > Utc::now() {
                Some(entry.clone())
            } else {
                None
            }
        })
    }

    /// Set in cache
    pub async fn set(&self, key: String, value: serde_json::Value, ttl_seconds: Option<u64>) {
        let now = Utc::now();
        let ttl = ttl_seconds.unwrap_or(self.config.default_ttl_seconds);

        let entry = CacheEntry {
            key: key.clone(),
            value,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(ttl as i64),
        };

        let mut cache = self.cache.write().await;

        // Evict if at capacity
        if cache.len() >= self.config.max_entries {
            self.evict_expired(&mut cache);
        }

        cache.insert(key, entry);
    }

    /// Remove from cache
    pub async fn remove(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }

    /// Evict expired entries
    fn evict_expired(&self, cache: &mut HashMap<String, CacheEntry>) {
        let now = Utc::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }

    /// Clear cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_operations() {
        let cache = SchedulerCache::new(CacheConfig::default());

        cache
            .set("key".to_string(), serde_json::json!("value"), None)
            .await;

        let entry = cache.get("key").await;
        assert!(entry.is_some());

        cache.remove("key").await;
        let entry = cache.get("key").await;
        assert!(entry.is_none());
    }
}
