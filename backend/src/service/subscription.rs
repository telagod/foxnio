//! 订阅系统

use anyhow::{Result, bail};
use chrono::{DateTime, Utc, Duration};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 订阅计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    pub id: Uuid,
    pub name: String,
    pub price: i64, // 分
    pub duration_days: i32,
    pub features: PlanFeatures,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 计划特性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFeatures {
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_month: Option<i64>,
    pub allowed_models: Vec<String>,
    pub priority: i32,
    pub rate_limit: i32, // 每分钟请求数
}

/// 用户订阅
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub plan_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SubscriptionStatus,
    pub auto_renew: bool,
    pub created_at: DateTime<Utc>,
}

/// 订阅状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubscriptionStatus {
    Active,
    Expired,
    Cancelled,
    Paused,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionStatus::Active => write!(f, "active"),
            SubscriptionStatus::Expired => write!(f, "expired"),
            SubscriptionStatus::Cancelled => write!(f, "cancelled"),
            SubscriptionStatus::Paused => write!(f, "paused"),
        }
    }
}

/// 订阅服务
pub struct SubscriptionService {
    db: DatabaseConnection,
}

impl SubscriptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
    
    /// 创建订阅计划
    pub async fn create_plan(
        &self,
        name: String,
        price: i64,
        duration_days: i32,
        features: PlanFeatures,
    ) -> Result<SubscriptionPlan> {
        let plan = SubscriptionPlan {
            id: Uuid::new_v4(),
            name,
            price,
            duration_days,
            features,
            is_active: true,
            created_at: Utc::now(),
        };
        
        // TODO: 保存到数据库
        Ok(plan)
    }
    
    /// 获取所有活跃计划
    pub async fn list_active_plans(&self) -> Result<Vec<SubscriptionPlan>> {
        // TODO: 从数据库查询
        Ok(vec![])
    }
    
    /// 用户订阅计划
    pub async fn subscribe(
        &self,
        user_id: Uuid,
        plan_id: Uuid,
        duration_days: Option<i32>,
    ) -> Result<UserSubscription> {
        // 检查用户是否有余额
        // 检查计划是否存在
        // 扣除余额
        // 创建订阅
        
        let now = Utc::now();
        let days = duration_days.unwrap_or(30);
        
        let subscription = UserSubscription {
            id: Uuid::new_v4(),
            user_id,
            plan_id,
            start_time: now,
            end_time: now + Duration::days(days as i64),
            status: SubscriptionStatus::Active,
            auto_renew: false,
            created_at: now,
        };
        
        Ok(subscription)
    }
    
    /// 取消订阅
    pub async fn cancel_subscription(&self, subscription_id: Uuid) -> Result<()> {
        // TODO: 更新数据库
        Ok(())
    }
    
    /// 检查用户订阅状态
    pub async fn check_subscription(&self, user_id: Uuid) -> Result<Option<UserSubscription>> {
        // TODO: 从数据库查询
        Ok(None)
    }
    
    /// 续费订阅
    pub async fn renew_subscription(&self, subscription_id: Uuid) -> Result<UserSubscription> {
        // TODO: 实现续费逻辑
        bail!("Not implemented")
    }
    
    /// 获取用户剩余配额
    pub async fn get_user_quota(&self, user_id: Uuid) -> Result<UserQuota> {
        // TODO: 查询用户配额
        Ok(UserQuota {
            daily_requests: 0,
            daily_limit: None,
            monthly_tokens: 0,
            monthly_limit: None,
            allowed_models: vec!["*".to_string()],
        })
    }
    
    /// 检查用户是否可以使用指定模型
    pub async fn can_use_model(&self, user_id: Uuid, model: &str) -> Result<bool> {
        let quota = self.get_user_quota(user_id).await?;
        
        // 检查模型是否在允许列表中
        if quota.allowed_models.contains(&"*".to_string()) {
            return Ok(true);
        }
        
        Ok(quota.allowed_models.iter().any(|m| {
            model.starts_with(m) || model == m
        }))
    }
    
    /// 记录使用量
    pub async fn record_usage(
        &self,
        user_id: Uuid,
        requests: i32,
        tokens: i64,
    ) -> Result<()> {
        // TODO: 更新使用量统计
        Ok(())
    }
    
    /// 自动续费检查
    pub async fn check_auto_renewals(&self) -> Result<Vec<UserSubscription>> {
        // TODO: 查询需要续费的订阅
        Ok(vec![])
    }
}

/// 用户配额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuota {
    pub daily_requests: i32,
    pub daily_limit: Option<i32>,
    pub monthly_tokens: i64,
    pub monthly_limit: Option<i64>,
    pub allowed_models: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_subscription_status_display() {
        assert_eq!(SubscriptionStatus::Active.to_string(), "active");
        assert_eq!(SubscriptionStatus::Expired.to_string(), "expired");
        assert_eq!(SubscriptionStatus::Cancelled.to_string(), "cancelled");
        assert_eq!(SubscriptionStatus::Paused.to_string(), "paused");
    }
    
    #[test]
    fn test_plan_features() {
        let features = PlanFeatures {
            max_requests_per_day: Some(100),
            max_tokens_per_month: Some(1000000),
            allowed_models: vec!["gpt-4".to_string(), "claude-3".to_string()],
            priority: 1,
            rate_limit: 60,
        };
        
        assert_eq!(features.max_requests_per_day, Some(100));
        assert_eq!(features.allowed_models.len(), 2);
    }
    
    #[test]
    fn test_subscription_duration() {
        let now = Utc::now();
        let end = now + Duration::days(30);
        
        let duration = end - now;
        assert_eq!(duration.num_days(), 30);
    }
    
    #[test]
    fn test_user_quota() {
        let quota = UserQuota {
            daily_requests: 50,
            daily_limit: Some(100),
            monthly_tokens: 50000,
            monthly_limit: Some(100000),
            allowed_models: vec!["*".to_string()],
        };
        
        assert!(quota.daily_requests < quota.daily_limit.unwrap());
    }
}
