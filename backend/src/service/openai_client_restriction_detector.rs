use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// Detector for OpenAI client restrictions and violations
pub struct OpenAIClientRestrictionDetector {
    violations: Arc<RwLock<HashMap<String, ViolationRecord>>>,
    config: RestrictionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictionConfig {
    pub max_violations_per_hour: u32,
    pub ban_duration_minutes: u64,
    pub enable_auto_ban: bool,
    pub restricted_models: Vec<String>,
    pub restricted_regions: Vec<String>,
    pub max_request_size: usize,
    pub max_tokens_per_request: u32,
}

impl Default for RestrictionConfig {
    fn default() -> Self {
        Self {
            max_violations_per_hour: 10,
            ban_duration_minutes: 60,
            enable_auto_ban: true,
            restricted_models: vec![],
            restricted_regions: vec![],
            max_request_size: 10 * 1024 * 1024, // 10MB
            max_tokens_per_request: 100000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationRecord {
    pub client_id: String,
    pub violation_count: u32,
    pub last_violation_at: DateTime<Utc>,
    pub is_banned: bool,
    pub ban_until: Option<DateTime<Utc>>,
    pub violation_types: Vec<ViolationType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    RateLimitExceeded,
    InvalidRequest,
    RestrictedModel,
    RestrictedRegion,
    OversizedRequest,
    TokenLimitExceeded,
    SuspiciousPattern,
    AbusiveContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub client_id: String,
    pub model: String,
    pub region: Option<String>,
    pub request_size: usize,
    pub tokens: Option<u32>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictionResult {
    pub is_allowed: bool,
    pub violation: Option<ViolationType>,
    pub reason: Option<String>,
    pub remaining_quota: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum RestrictionError {
    #[error("Client banned: {0}")]
    ClientBanned(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

impl OpenAIClientRestrictionDetector {
    pub fn new(config: RestrictionConfig) -> Self {
        Self {
            violations: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if request is allowed
    pub async fn check_request(
        &self,
        ctx: &RequestContext,
    ) -> Result<RestrictionResult, RestrictionError> {
        // Check if client is banned
        if self.is_client_banned(&ctx.client_id).await {
            return Err(RestrictionError::ClientBanned(ctx.client_id.clone()));
        }

        // Check restricted models
        if self.config.restricted_models.contains(&ctx.model) {
            self.record_violation(&ctx.client_id, ViolationType::RestrictedModel)
                .await;

            return Ok(RestrictionResult {
                is_allowed: false,
                violation: Some(ViolationType::RestrictedModel),
                reason: Some(format!("Model {} is restricted", ctx.model)),
                remaining_quota: None,
            });
        }

        // Check restricted regions
        if let Some(region) = &ctx.region {
            if self.config.restricted_regions.contains(region) {
                self.record_violation(&ctx.client_id, ViolationType::RestrictedRegion)
                    .await;

                return Ok(RestrictionResult {
                    is_allowed: false,
                    violation: Some(ViolationType::RestrictedRegion),
                    reason: Some(format!("Region {} is restricted", region)),
                    remaining_quota: None,
                });
            }
        }

        // Check request size
        if ctx.request_size > self.config.max_request_size {
            self.record_violation(&ctx.client_id, ViolationType::OversizedRequest)
                .await;

            return Ok(RestrictionResult {
                is_allowed: false,
                violation: Some(ViolationType::OversizedRequest),
                reason: Some(format!(
                    "Request size {} exceeds limit {}",
                    ctx.request_size, self.config.max_request_size
                )),
                remaining_quota: None,
            });
        }

        // Check token limit
        if let Some(tokens) = ctx.tokens {
            if tokens > self.config.max_tokens_per_request {
                self.record_violation(&ctx.client_id, ViolationType::TokenLimitExceeded)
                    .await;

                return Ok(RestrictionResult {
                    is_allowed: false,
                    violation: Some(ViolationType::TokenLimitExceeded),
                    reason: Some(format!(
                        "Token count {} exceeds limit {}",
                        tokens, self.config.max_tokens_per_request
                    )),
                    remaining_quota: None,
                });
            }
        }

        Ok(RestrictionResult {
            is_allowed: true,
            violation: None,
            reason: None,
            remaining_quota: None,
        })
    }

    /// Check if client is banned
    async fn is_client_banned(&self, client_id: &str) -> bool {
        let violations = self.violations.read().await;
        if let Some(record) = violations.get(client_id) {
            if record.is_banned {
                if let Some(ban_until) = record.ban_until {
                    return Utc::now() < ban_until;
                }
            }
        }
        false
    }

    /// Record violation
    async fn record_violation(&self, client_id: &str, violation_type: ViolationType) {
        let mut violations = self.violations.write().await;
        let record = violations
            .entry(client_id.to_string())
            .or_insert(ViolationRecord {
                client_id: client_id.to_string(),
                violation_count: 0,
                last_violation_at: Utc::now(),
                is_banned: false,
                ban_until: None,
                violation_types: vec![],
            });

        record.violation_count += 1;
        record.last_violation_at = Utc::now();
        record.violation_types.push(violation_type.clone());

        // Auto-ban if threshold exceeded
        if self.config.enable_auto_ban
            && record.violation_count >= self.config.max_violations_per_hour
        {
            record.is_banned = true;
            record.ban_until = Some(
                Utc::now() + chrono::Duration::minutes(self.config.ban_duration_minutes as i64),
            );
            warn!(
                "Client {} auto-banned for {} minutes",
                client_id, self.config.ban_duration_minutes
            );
        }
    }

    /// Report suspicious activity
    pub async fn report_suspicious_activity(
        &self,
        client_id: &str,
        description: &str,
    ) -> Result<(), RestrictionError> {
        warn!(
            "Suspicious activity from client {}: {}",
            client_id, description
        );
        self.record_violation(client_id, ViolationType::SuspiciousPattern)
            .await;
        Ok(())
    }

    /// Get violation record for client
    pub async fn get_violation_record(&self, client_id: &str) -> Option<ViolationRecord> {
        let violations = self.violations.read().await;
        violations.get(client_id).cloned()
    }

    /// Clear violations for client
    pub async fn clear_violations(&self, client_id: &str) {
        let mut violations = self.violations.write().await;
        violations.remove(client_id);
    }

    /// Unban client
    pub async fn unban_client(&self, client_id: &str) {
        let mut violations = self.violations.write().await;
        if let Some(record) = violations.get_mut(client_id) {
            record.is_banned = false;
            record.ban_until = None;
        }
    }

    /// Clean up expired bans
    pub async fn cleanup_expired_bans(&self) {
        let mut violations = self.violations.write().await;
        violations.retain(|_, record| {
            if record.is_banned {
                if let Some(ban_until) = record.ban_until {
                    return Utc::now() < ban_until;
                }
            }
            true
        });
    }

    /// Get all banned clients
    pub async fn get_banned_clients(&self) -> Vec<String> {
        let violations = self.violations.read().await;
        violations
            .iter()
            .filter(|(_, record)| record.is_banned)
            .map(|(client_id, _)| client_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_restriction_detector() {
        let detector = OpenAIClientRestrictionDetector::new(RestrictionConfig::default());

        let ctx = RequestContext {
            client_id: "test_client".to_string(),
            model: "gpt-4".to_string(),
            region: Some("us".to_string()),
            request_size: 1024,
            tokens: Some(1000),
            user_agent: None,
            ip_address: None,
        };

        let result = detector.check_request(&ctx).await.unwrap();
        assert!(result.is_allowed);
    }

    #[test]
    fn test_restriction_config_default() {
        let config = RestrictionConfig::default();
        assert_eq!(config.max_violations_per_hour, 10);
        assert!(config.enable_auto_ban);
    }
}
