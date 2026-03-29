//! Rate limit service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limit config
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub key: String,
    pub max_requests: u32,
    pub window_seconds: u64,
}

/// Rate limit state
#[derive(Debug, Clone)]
pub struct RateLimitState {
    pub count: u32,
    pub window_start: Instant,
    pub config: RateLimitConfig,
}

/// Rate limit service
pub struct RateLimitService {
    states: Arc<RwLock<HashMap<String, RateLimitState>>>,
}

impl Default for RateLimitService {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitService {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check(&self, key: &str, config: RateLimitConfig) -> Result<(), String> {
        let mut states = self.states.write().await;
        let now = Instant::now();

        let state = states
            .entry(key.to_string())
            .or_insert_with(|| RateLimitState {
                count: 0,
                window_start: now,
                config: config.clone(),
            });

        // Reset if window expired
        if now.duration_since(state.window_start) > Duration::from_secs(state.config.window_seconds)
        {
            state.count = 0;
            state.window_start = now;
        }

        if state.count >= state.config.max_requests {
            return Err("Rate limit exceeded".to_string());
        }

        state.count += 1;
        Ok(())
    }

    pub async fn reset(&self, key: &str) {
        let mut states = self.states.write().await;
        states.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit() {
        let service = RateLimitService::new();
        let config = RateLimitConfig {
            key: "test".to_string(),
            max_requests: 2,
            window_seconds: 60,
        };

        assert!(service.check("test", config.clone()).await.is_ok());
        assert!(service.check("test", config.clone()).await.is_ok());
        assert!(service.check("test", config).await.is_err());
    }
}
