//! User group rate resolver service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::user_group_rate::{UserGroupRate, UserGroupRateService};

/// User group rate resolver
pub struct UserGroupRateResolver {
    /// User to group mapping
    user_groups: Arc<RwLock<HashMap<i64, i64>>>,
    /// Group rate service reference
    rate_service: Arc<UserGroupRateService>,
}

impl UserGroupRateResolver {
    /// Create a new resolver
    pub fn new(rate_service: Arc<UserGroupRateService>) -> Self {
        Self {
            user_groups: Arc::new(RwLock::new(HashMap::new())),
            rate_service,
        }
    }

    /// Set user's group
    pub async fn set_user_group(&self, user_id: i64, group_id: i64) {
        let mut user_groups = self.user_groups.write().await;
        user_groups.insert(user_id, group_id);
    }

    /// Get user's group
    pub async fn get_user_group(&self, user_id: i64) -> Option<i64> {
        let user_groups = self.user_groups.read().await;
        user_groups.get(&user_id).copied()
    }

    /// Resolve rate for user
    pub async fn resolve_rate(&self, user_id: i64) -> Option<UserGroupRate> {
        let group_id = self.get_user_group(user_id).await?;
        self.rate_service.get_rate(group_id).await
    }

    /// Calculate cost for user
    pub async fn calculate_user_cost(&self, user_id: i64, tokens: u64, model: &str) -> Option<f64> {
        let group_id = self.get_user_group(user_id).await?;
        self.rate_service
            .calculate_cost(group_id, tokens, model)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolver() {
        let rate_service = Arc::new(UserGroupRateService::new());
        let resolver = UserGroupRateResolver::new(rate_service);

        resolver.set_user_group(123, 1).await;
        let group = resolver.get_user_group(123).await;
        assert_eq!(group, Some(1));
    }
}
