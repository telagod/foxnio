use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache for RPM (Requests Per Minute) tracking
pub struct RpmCache {
    cache: Arc<RwLock<HashMap<String, RpmEntry>>>,
    config: RpmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpmEntry {
    pub key: String,
    pub count: u32,
    pub window_start: DateTime<Utc>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpmConfig {
    pub window_size_seconds: u64,
    pub default_limit: u32,
}

impl Default for RpmConfig {
    fn default() -> Self {
        Self {
            window_size_seconds: 60,
            default_limit: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpmCheck {
    pub allowed: bool,
    pub current: u32,
    pub limit: u32,
    pub reset_at: DateTime<Utc>,
}

impl RpmCache {
    pub fn new(config: RpmConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check and increment RPM
    pub async fn check_and_increment(&self, key: &str, limit: Option<u32>) -> RpmCheck {
        let mut cache = self.cache.write().await;
        let now = Utc::now();
        let limit = limit.unwrap_or(self.config.default_limit);

        let entry = cache.entry(key.to_string()).or_insert(RpmEntry {
            key: key.to_string(),
            count: 0,
            window_start: now,
            limit,
        });

        // Reset window if expired
        let window_end =
            entry.window_start + chrono::Duration::seconds(self.config.window_size_seconds as i64);
        if now >= window_end {
            entry.count = 0;
            entry.window_start = now;
        }

        let allowed = entry.count < entry.limit;
        if allowed {
            entry.count += 1;
        }

        RpmCheck {
            allowed,
            current: entry.count,
            limit: entry.limit,
            reset_at: entry.window_start
                + chrono::Duration::seconds(self.config.window_size_seconds as i64),
        }
    }

    /// Get current RPM
    pub async fn get_current(&self, key: &str) -> u32 {
        let cache = self.cache.read().await;
        cache.get(key).map(|e| e.count).unwrap_or(0)
    }

    /// Reset RPM for key
    pub async fn reset(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpm_cache() {
        let cache = RpmCache::new(RpmConfig::default());

        // Should allow first request
        let check = cache.check_and_increment("test", Some(10)).await;
        assert!(check.allowed);
        assert_eq!(check.current, 1);

        // Should allow up to limit
        for _ in 0..9 {
            cache.check_and_increment("test", Some(10)).await;
        }

        // Should deny after limit
        let check = cache.check_and_increment("test", Some(10)).await;
        assert!(!check.allowed);
    }
}
