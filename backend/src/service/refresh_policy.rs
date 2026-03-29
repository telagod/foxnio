use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Refresh policy for tokens
pub struct RefreshPolicy {
    config: RefreshConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshConfig {
    pub refresh_before_expiry_seconds: u64,
    pub max_refresh_attempts: u32,
    pub refresh_attempt_delay_ms: u64,
    pub refresh_on_error: bool,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            refresh_before_expiry_seconds: 300,
            max_refresh_attempts: 3,
            refresh_attempt_delay_ms: 1000,
            refresh_on_error: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshDecision {
    pub should_refresh: bool,
    pub reason: RefreshReason,
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefreshReason {
    TokenExpiringSoon,
    TokenExpired,
    ErrorOccurred,
    ManualRefresh,
    NotNeeded,
}

impl RefreshPolicy {
    pub fn new(config: RefreshConfig) -> Self {
        Self { config }
    }

    /// Should refresh token
    pub fn should_refresh(&self, expires_at: DateTime<Utc>) -> RefreshDecision {
        let now = Utc::now();
        let time_until_expiry = (expires_at - now).num_seconds();

        if time_until_expiry <= 0 {
            return RefreshDecision {
                should_refresh: true,
                reason: RefreshReason::TokenExpired,
                delay_ms: None,
            };
        }

        if time_until_expiry <= self.config.refresh_before_expiry_seconds as i64 {
            return RefreshDecision {
                should_refresh: true,
                reason: RefreshReason::TokenExpiringSoon,
                delay_ms: None,
            };
        }

        RefreshDecision {
            should_refresh: false,
            reason: RefreshReason::NotNeeded,
            delay_ms: None,
        }
    }

    /// Get delay between attempts
    pub fn get_attempt_delay(&self, attempt: u32) -> Option<u64> {
        if attempt >= self.config.max_refresh_attempts {
            return None;
        }

        Some(self.config.refresh_attempt_delay_ms * 2u64.pow(attempt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_policy() {
        let policy = RefreshPolicy::new(RefreshConfig::default());

        // Test expired token
        let expired = Utc::now() - chrono::Duration::seconds(10);
        let decision = policy.should_refresh(expired);
        assert_eq!(decision.reason, RefreshReason::TokenExpired);

        // Test valid token
        let valid = Utc::now() + chrono::Duration::seconds(600);
        let decision = policy.should_refresh(valid);
        assert_eq!(decision.reason, RefreshReason::NotNeeded);
    }
}
