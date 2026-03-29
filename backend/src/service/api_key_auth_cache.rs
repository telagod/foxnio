use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Auth cache for API keys
pub struct ApiKeyAuthCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    config: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key_hash: String,
    pub user_id: i64,
    pub permissions: Vec<String>,
    pub cached_at: i64,
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
            default_ttl_seconds: 300,
        }
    }
}

impl ApiKeyAuthCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get from cache
    pub async fn get(&self, key_hash: &str) -> Option<CacheEntry> {
        let cache = self.cache.read().await;
        cache.get(key_hash).and_then(|entry| {
            let now = chrono::Utc::now().timestamp();
            if now - entry.cached_at > entry.ttl_seconds as i64 {
                None
            } else {
                Some(entry.clone())
            }
        })
    }

    /// Set in cache
    pub async fn set(&self, key_hash: String, entry: CacheEntry) {
        let mut cache = self.cache.write().await;

        // Evict old entries if at capacity
        if cache.len() >= self.config.max_entries {
            self.evict_oldest(&mut cache);
        }

        cache.insert(key_hash, entry);
    }

    /// Remove from cache
    pub async fn remove(&self, key_hash: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key_hash);
    }

    /// Clear cache
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Evict oldest entries
    fn evict_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
        let mut entries: Vec<_> = cache.iter().collect();
        entries.sort_by_key(|(_, entry)| entry.cached_at);

        // Remove oldest 10%
        let to_remove: Vec<_> = entries
            .into_iter()
            .take(self.config.max_entries / 10)
            .map(|(k, _)| k.clone())
            .collect();
        for key in to_remove {
            cache.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_operations() {
        let cache = ApiKeyAuthCache::new(CacheConfig::default());

        let entry = CacheEntry {
            key_hash: "test".to_string(),
            user_id: 1,
            permissions: vec!["read".to_string()],
            cached_at: chrono::Utc::now().timestamp(),
            ttl_seconds: 300,
        };

        cache.set("test".to_string(), entry.clone()).await;

        let cached = cache.get("test").await;
        assert!(cached.is_some());

        cache.remove("test").await;
        let cached = cache.get("test").await;
        assert!(cached.is_none());
    }
}
