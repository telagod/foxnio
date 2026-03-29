//! User subscription port (interface)

use async_trait::async_trait;

/// Subscription plan
#[derive(Debug, Clone)]
pub struct SubscriptionPlan {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub features: Vec<String>,
    pub monthly_quota: u64,
}

/// User subscription
#[derive(Debug, Clone)]
pub struct UserSubscription {
    pub user_id: i64,
    pub plan_id: String,
    pub start_time: i64,
    pub end_time: i64,
    pub is_active: bool,
}

/// User subscription port trait
#[async_trait]
pub trait UserSubscriptionPort: Send + Sync {
    /// Get user's subscription
    async fn get_subscription(&self, user_id: i64) -> Option<UserSubscription>;

    /// Subscribe user to plan
    async fn subscribe(&self, user_id: i64, plan_id: &str) -> Result<(), String>;

    /// Cancel subscription
    async fn cancel(&self, user_id: i64) -> Result<(), String>;

    /// List available plans
    async fn list_plans(&self) -> Vec<SubscriptionPlan>;
}

/// Default implementation
pub struct DefaultUserSubscriptionPort {
    subscriptions:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<i64, UserSubscription>>>,
}

impl Default for DefaultUserSubscriptionPort {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultUserSubscriptionPort {
    pub fn new() -> Self {
        Self {
            subscriptions: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }
}

#[async_trait]
impl UserSubscriptionPort for DefaultUserSubscriptionPort {
    async fn get_subscription(&self, user_id: i64) -> Option<UserSubscription> {
        let subs = self.subscriptions.read().await;
        subs.get(&user_id).cloned()
    }

    async fn subscribe(&self, user_id: i64, plan_id: &str) -> Result<(), String> {
        let mut subs = self.subscriptions.write().await;
        let now = chrono::Utc::now().timestamp();
        subs.insert(
            user_id,
            UserSubscription {
                user_id,
                plan_id: plan_id.to_string(),
                start_time: now,
                end_time: now + 30 * 24 * 3600, // 30 days
                is_active: true,
            },
        );
        Ok(())
    }

    async fn cancel(&self, user_id: i64) -> Result<(), String> {
        let mut subs = self.subscriptions.write().await;
        if let Some(sub) = subs.get_mut(&user_id) {
            sub.is_active = false;
            Ok(())
        } else {
            Err("Subscription not found".to_string())
        }
    }

    async fn list_plans(&self) -> Vec<SubscriptionPlan> {
        vec![
            SubscriptionPlan {
                id: "basic".to_string(),
                name: "Basic".to_string(),
                price: 9.99,
                features: vec!["100K tokens/month".to_string()],
                monthly_quota: 100_000,
            },
            SubscriptionPlan {
                id: "pro".to_string(),
                name: "Pro".to_string(),
                price: 29.99,
                features: vec!["1M tokens/month".to_string()],
                monthly_quota: 1_000_000,
            },
        ]
    }
}
