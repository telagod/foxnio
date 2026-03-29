//! User group rate service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// User group rate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGroupRate {
    /// Group ID
    pub group_id: i64,
    /// Group name
    pub name: String,
    /// Rate per 1K tokens
    pub rate_per_1k: f64,
    /// Monthly quota
    pub monthly_quota: Option<u64>,
    /// RPM limit
    pub rpm_limit: Option<u32>,
    /// TPM limit
    pub tpm_limit: Option<u64>,
    /// Model multipliers
    pub model_multipliers: HashMap<String, f64>,
}

/// User group rate service
pub struct UserGroupRateService {
    /// Group rates
    rates: Arc<RwLock<HashMap<i64, UserGroupRate>>>,
}

impl Default for UserGroupRateService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserGroupRateService {
    /// Create a new service
    pub fn new() -> Self {
        Self {
            rates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set group rate
    pub async fn set_rate(&self, rate: UserGroupRate) {
        let mut rates = self.rates.write().await;
        rates.insert(rate.group_id, rate);
    }

    /// Get group rate
    pub async fn get_rate(&self, group_id: i64) -> Option<UserGroupRate> {
        let rates = self.rates.read().await;
        rates.get(&group_id).cloned()
    }

    /// Calculate cost for usage
    pub async fn calculate_cost(&self, group_id: i64, tokens: u64, model: &str) -> Option<f64> {
        let rates = self.rates.read().await;
        let group_rate = rates.get(&group_id)?;

        let base_cost = (tokens as f64 / 1000.0) * group_rate.rate_per_1k;

        // Apply model multiplier if configured
        let multiplier = group_rate
            .model_multipliers
            .get(model)
            .copied()
            .unwrap_or(1.0);

        Some(base_cost * multiplier)
    }

    /// Check if within quota
    pub async fn check_quota(&self, group_id: i64, current_usage: u64) -> bool {
        let rates = self.rates.read().await;

        if let Some(rate) = rates.get(&group_id) {
            if let Some(quota) = rate.monthly_quota {
                return current_usage < quota;
            }
        }

        true
    }

    /// Get RPM limit for group
    pub async fn get_rpm_limit(&self, group_id: i64) -> Option<u32> {
        let rates = self.rates.read().await;
        rates.get(&group_id).and_then(|r| r.rpm_limit)
    }

    /// List all group rates
    pub async fn list_rates(&self) -> Vec<UserGroupRate> {
        let rates = self.rates.read().await;
        rates.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_rate() {
        let service = UserGroupRateService::new();

        let rate = UserGroupRate {
            group_id: 1,
            name: "Standard".to_string(),
            rate_per_1k: 0.01,
            monthly_quota: Some(1000000),
            rpm_limit: Some(60),
            tpm_limit: Some(100000),
            model_multipliers: HashMap::new(),
        };

        service.set_rate(rate.clone()).await;
        let retrieved = service.get_rate(1).await.unwrap();

        assert_eq!(retrieved.name, "Standard");
        assert_eq!(retrieved.rate_per_1k, 0.01);
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let service = UserGroupRateService::new();

        let mut multipliers = HashMap::new();
        multipliers.insert("gpt-4".to_string(), 2.0);

        let rate = UserGroupRate {
            group_id: 1,
            name: "Standard".to_string(),
            rate_per_1k: 0.01,
            monthly_quota: None,
            rpm_limit: None,
            tpm_limit: None,
            model_multipliers: multipliers,
        };

        service.set_rate(rate).await;

        let cost = service.calculate_cost(1, 1000, "gpt-4").await.unwrap();
        assert_eq!(cost, 0.02); // 0.01 * 2.0
    }
}
