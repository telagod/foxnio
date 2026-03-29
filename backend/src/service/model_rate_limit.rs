//! Model rate limit service

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Rate limit configuration for a model
#[derive(Debug, Clone)]
pub struct ModelRateLimitConfig {
    /// Model name
    pub model: String,
    /// Requests per minute
    pub rpm: u32,
    /// Tokens per minute
    pub tpm: u64,
    /// Requests per day
    pub rpd: u32,
    /// Burst allowance
    pub burst: u32,
}

/// Rate limit usage tracking
#[derive(Debug, Clone)]
pub struct RateLimitUsage {
    /// Current RPM count
    pub rpm_count: u32,
    /// Current TPM count
    pub tpm_count: u64,
    /// Current RPD count
    pub rpd_count: u32,
    /// Last reset time
    pub last_reset: DateTime<Utc>,
}

/// Model rate limit service
pub struct ModelRateLimitService {
    /// Configurations per model
    configs: Arc<RwLock<HashMap<String, ModelRateLimitConfig>>>,
    /// Usage tracking per model
    usage: Arc<RwLock<HashMap<String, RateLimitUsage>>>,
}

impl Default for ModelRateLimitService {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRateLimitService {
    /// Create a new model rate limit service
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            usage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set rate limit config for a model
    pub async fn set_config(&self, config: ModelRateLimitConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(config.model.clone(), config);
    }

    /// Check if request is allowed
    pub async fn check_rate_limit(&self, model: &str) -> Result<(), String> {
        let configs = self.configs.read().await;
        let config = configs
            .get(model)
            .ok_or_else(|| format!("No rate limit config for model: {}", model))?;

        let mut usage = self.usage.write().await;
        let model_usage = usage
            .entry(model.to_string())
            .or_insert_with(|| RateLimitUsage {
                rpm_count: 0,
                tpm_count: 0,
                rpd_count: 0,
                last_reset: Utc::now(),
            });

        // Reset counters if needed
        let now = Utc::now();
        let elapsed = (now - model_usage.last_reset).num_seconds();

        if elapsed >= 60 {
            model_usage.rpm_count = 0;
            model_usage.tpm_count = 0;
            model_usage.last_reset = now;
        }

        if elapsed >= 86400 {
            model_usage.rpd_count = 0;
        }

        // Check limits
        if model_usage.rpm_count >= config.rpm {
            return Err("RPM limit exceeded".to_string());
        }

        if model_usage.rpd_count >= config.rpd {
            return Err("RPD limit exceeded".to_string());
        }

        Ok(())
    }

    /// Record a request
    pub async fn record_request(&self, model: &str, tokens: u64) {
        let mut usage = self.usage.write().await;
        if let Some(model_usage) = usage.get_mut(model) {
            model_usage.rpm_count += 1;
            model_usage.tpm_count += tokens;
            model_usage.rpd_count += 1;
        }
    }

    /// Get current usage for a model
    pub async fn get_usage(&self, model: &str) -> Option<RateLimitUsage> {
        let usage = self.usage.read().await;
        usage.get(model).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit() {
        let service = ModelRateLimitService::new();

        service
            .set_config(ModelRateLimitConfig {
                model: "gpt-4".to_string(),
                rpm: 60,
                tpm: 100000,
                rpd: 1000,
                burst: 10,
            })
            .await;

        let result = service.check_rate_limit("gpt-4").await;
        assert!(result.is_ok());
    }
}
