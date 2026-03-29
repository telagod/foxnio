//! Gemini token cache service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Gemini token cache entry
#[derive(Debug, Clone)]
pub struct GeminiTokenEntry {
    /// Access token
    pub access_token: String,
    /// Refresh token
    pub refresh_token: Option<String>,
    /// Expires at
    pub expires_at: Instant,
    /// Scope
    pub scope: String,
}

impl GeminiTokenEntry {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Check if token expires within duration
    pub fn expires_within(&self, duration: Duration) -> bool {
        Instant::now().duration_since(Instant::now())
            >= self.expires_at.saturating_duration_since(Instant::now()) - duration
    }
}

/// Gemini token cache
pub struct GeminiTokenCache {
    /// Token entries by user ID
    tokens: Arc<RwLock<HashMap<i64, GeminiTokenEntry>>>,
    /// Default token TTL
    default_ttl: Duration,
}

impl Default for GeminiTokenCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

impl GeminiTokenCache {
    /// Create a new token cache
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    /// Store a token
    pub async fn store(
        &self,
        user_id: i64,
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<Duration>,
    ) {
        let mut tokens = self.tokens.write().await;

        let ttl = expires_in.unwrap_or(self.default_ttl);
        let entry = GeminiTokenEntry {
            access_token,
            refresh_token,
            expires_at: Instant::now() + ttl,
            scope: "gemini".to_string(),
        };

        tokens.insert(user_id, entry);
    }

    /// Get a token
    pub async fn get(&self, user_id: i64) -> Option<GeminiTokenEntry> {
        let tokens = self.tokens.read().await;
        tokens.get(&user_id).cloned()
    }

    /// Get valid token (not expired)
    pub async fn get_valid(&self, user_id: i64) -> Option<GeminiTokenEntry> {
        let tokens = self.tokens.read().await;
        tokens.get(&user_id).filter(|t| !t.is_expired()).cloned()
    }

    /// Remove a token
    pub async fn remove(&self, user_id: i64) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(&user_id);
    }

    /// Clear expired tokens
    pub async fn clear_expired(&self) {
        let mut tokens = self.tokens.write().await;
        tokens.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        self.tokens.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_get() {
        let cache = GeminiTokenCache::new(Duration::from_secs(60));

        cache.store(123, "token-123".to_string(), None, None).await;
        let entry = cache.get(123).await;

        assert!(entry.is_some());
        assert_eq!(entry.unwrap().access_token, "token-123");
    }

    #[tokio::test]
    async fn test_expired_token() {
        let cache = GeminiTokenCache::new(Duration::from_secs(60));

        cache
            .store(
                123,
                "token-123".to_string(),
                None,
                Some(Duration::from_millis(1)),
            )
            .await;
        tokio::time::sleep(Duration::from_millis(10)).await;

        let entry = cache.get_valid(123).await;
        assert!(entry.is_none());
    }
}
