//! Session limit cache service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Session limit entry
#[derive(Debug, Clone)]
pub struct SessionLimitEntry {
    /// Session ID
    pub session_id: String,
    /// User ID
    pub user_id: i64,
    /// Request count
    pub request_count: u32,
    /// Token count
    pub token_count: u64,
    /// Created at
    pub created_at: Instant,
    /// Last accessed
    pub last_accessed: Instant,
    /// TTL in seconds
    pub ttl_seconds: u64,
}

/// Session limit cache
pub struct SessionLimitCache {
    /// Cache entries
    entries: Arc<RwLock<HashMap<String, SessionLimitEntry>>>,
    /// Default TTL
    default_ttl: Duration,
    /// Max entries
    max_entries: usize,
}

impl Default for SessionLimitCache {
    fn default() -> Self {
        Self::new(10000, Duration::from_secs(3600))
    }
}

impl SessionLimitCache {
    /// Create a new session limit cache
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
            max_entries,
        }
    }

    /// Get or create session entry
    pub async fn get_or_create(&self, session_id: &str, user_id: i64) -> SessionLimitEntry {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get(session_id) {
            if entry.last_accessed.elapsed() < Duration::from_secs(entry.ttl_seconds) {
                let mut entry = entry.clone();
                entry.last_accessed = Instant::now();
                entries.insert(session_id.to_string(), entry.clone());
                return entry;
            }
        }

        // Create new entry
        let entry = SessionLimitEntry {
            session_id: session_id.to_string(),
            user_id,
            request_count: 0,
            token_count: 0,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            ttl_seconds: self.default_ttl.as_secs(),
        };

        // Evict oldest if at capacity
        if entries.len() >= self.max_entries {
            self.evict_oldest(&mut entries);
        }

        entries.insert(session_id.to_string(), entry.clone());
        entry
    }

    /// Update session counts
    pub async fn update_counts(&self, session_id: &str, requests: u32, tokens: u64) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(session_id) {
            entry.request_count += requests;
            entry.token_count += tokens;
            entry.last_accessed = Instant::now();
        }
    }

    /// Remove a session
    pub async fn remove(&self, session_id: &str) {
        let mut entries = self.entries.write().await;
        entries.remove(session_id);
    }

    /// Clear expired entries
    pub async fn clear_expired(&self) {
        let mut entries = self.entries.write().await;
        let now = Instant::now();

        entries.retain(|_, entry| {
            now.duration_since(entry.last_accessed) < Duration::from_secs(entry.ttl_seconds)
        });
    }

    /// Evict oldest entry
    fn evict_oldest(&self, entries: &mut HashMap<String, SessionLimitEntry>) {
        if let Some((oldest_key, _)) = entries.iter().min_by_key(|(_, e)| e.last_accessed) {
            let key = oldest_key.clone();
            entries.remove(&key);
        }
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        self.entries.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache() {
        let cache = SessionLimitCache::new(100, Duration::from_secs(60));
        let entry = cache.get_or_create("session-1", 123).await;
        assert_eq!(entry.user_id, 123);
        assert_eq!(cache.size().await, 1);
    }

    #[tokio::test]
    async fn test_update_counts() {
        let cache = SessionLimitCache::new(100, Duration::from_secs(60));
        cache.get_or_create("session-1", 123).await;
        cache.update_counts("session-1", 1, 100).await;

        let entry = cache.get_or_create("session-1", 123).await;
        assert_eq!(entry.request_count, 1);
        assert_eq!(entry.token_count, 100);
    }
}
