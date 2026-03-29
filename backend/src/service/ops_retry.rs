use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Retry policy for operations
pub struct OpsRetry {
    config: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f32,
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            multiplier: 2.0,
            retryable_errors: vec![
                "timeout".to_string(),
                "connection_refused".to_string(),
                "rate_limited".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryState {
    pub attempt: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub total_delay_ms: u64,
}

impl OpsRetry {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self, error: &str) -> bool {
        self.config
            .retryable_errors
            .iter()
            .any(|e| error.contains(e))
    }

    /// Calculate delay for next retry
    pub fn calculate_delay(&self, attempt: u32) -> u64 {
        let delay =
            self.config.initial_delay_ms as f32 * self.config.multiplier.powi(attempt as i32);
        delay.min(self.config.max_delay_ms as f32) as u64
    }

    /// Should retry
    pub fn should_retry(&self, state: &RetryState) -> bool {
        state.attempt < self.config.max_attempts
            && state
                .last_error
                .as_ref()
                .map(|e| self.is_retryable(e))
                .unwrap_or(false)
    }

    /// Create new retry state
    pub fn new_state(&self) -> RetryState {
        RetryState {
            attempt: 0,
            last_error: None,
            next_retry_at: None,
            total_delay_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
    }

    #[test]
    fn test_calculate_delay() {
        let retry = OpsRetry::new(RetryConfig::default());

        let delay0 = retry.calculate_delay(0);
        let delay1 = retry.calculate_delay(1);
        let delay2 = retry.calculate_delay(2);

        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
    }
}
