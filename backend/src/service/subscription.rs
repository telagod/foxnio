//! 订阅系统
//!
//! 提供订阅计划管理、用户订阅、配额管理功能
//!
//! 预留功能：订阅系统（P2 功能）

#![allow(dead_code)]

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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

/// 用户配额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuota {
    pub user_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub plan_name: Option<String>,
    pub daily_requests_used: i32,
    pub daily_requests_limit: Option<i32>,
    pub monthly_tokens_used: i64,
    pub monthly_tokens_limit: Option<i64>,
    pub allowed_models: Vec<String>,
    pub priority: i32,
    pub rate_limit: i32,
    pub reset_daily_at: Option<DateTime<Utc>>,
    pub reset_monthly_at: Option<DateTime<Utc>>,
}

impl UserQuota {
    /// 检查是否有日限额
    pub fn has_daily_limit(&self) -> bool {
        self.daily_requests_limit.is_some()
    }

    /// 检查是否有月限额
    pub fn has_monthly_limit(&self) -> bool {
        self.monthly_tokens_limit.is_some()
    }

    /// 检查日限额是否超限
    pub fn is_daily_quota_exceeded(&self) -> bool {
        if let Some(limit) = self.daily_requests_limit {
            return self.daily_requests_used >= limit;
        }
        false
    }

    /// 检查月限额是否超限
    pub fn is_monthly_quota_exceeded(&self) -> bool {
        if let Some(limit) = self.monthly_tokens_limit {
            return self.monthly_tokens_used >= limit;
        }
        false
    }

    /// 获取日限额剩余
    pub fn daily_remaining(&self) -> Option<i32> {
        self.daily_requests_limit
            .map(|limit| (limit - self.daily_requests_used).max(0))
    }

    /// 获取月限额剩余
    pub fn monthly_remaining(&self) -> Option<i64> {
        self.monthly_tokens_limit
            .map(|limit| (limit - self.monthly_tokens_used).max(0))
    }

    /// 获取使用百分比
    pub fn daily_usage_percent(&self) -> Option<f64> {
        self.daily_requests_limit
            .map(|limit| (self.daily_requests_used as f64 / limit as f64) * 100.0)
    }

    pub fn monthly_usage_percent(&self) -> Option<f64> {
        self.monthly_tokens_limit
            .map(|limit| (self.monthly_tokens_used as f64 / limit as f64) * 100.0)
    }
}

/// 配额使用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUsageRecord {
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub requests: i32,
    pub tokens: i64,
    pub model: String,
}

/// 订阅服务配置
#[derive(Debug, Clone)]
pub struct SubscriptionConfig {
    /// 默认日限额
    pub default_daily_limit: i32,
    /// 默认月限额
    pub default_monthly_limit: i64,
    /// 允许超限
    pub allow_over_quota: bool,
    /// 超限警告阈值（百分比）
    pub quota_warning_threshold: f64,
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            default_daily_limit: 1000,
            default_monthly_limit: 10_000_000,
            allow_over_quota: false,
            quota_warning_threshold: 80.0,
        }
    }
}

/// 订阅服务
pub struct SubscriptionService {
    config: SubscriptionConfig,
    /// 用户配额缓存
    user_quotas: Arc<RwLock<HashMap<Uuid, UserQuota>>>,
    /// 使用记录
    usage_records: Arc<RwLock<Vec<QuotaUsageRecord>>>,
    /// 订阅计划
    plans: Arc<RwLock<HashMap<Uuid, SubscriptionPlan>>>,
    /// 用户订阅
    subscriptions: Arc<RwLock<HashMap<Uuid, UserSubscription>>>,
}

impl SubscriptionService {
    pub fn new(config: SubscriptionConfig) -> Self {
        Self {
            config,
            user_quotas: Arc::new(RwLock::new(HashMap::new())),
            usage_records: Arc::new(RwLock::new(Vec::new())),
            plans: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
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

        let mut plans = self.plans.write().await;
        plans.insert(plan.id, plan.clone());

        Ok(plan)
    }

    /// 获取所有活跃计划
    pub async fn list_active_plans(&self) -> Result<Vec<SubscriptionPlan>> {
        let plans = self.plans.read().await;
        Ok(plans.values().filter(|p| p.is_active).cloned().collect())
    }

    /// 用户订阅计划
    pub async fn subscribe(
        &self,
        user_id: Uuid,
        plan_id: Uuid,
        duration_days: Option<i32>,
    ) -> Result<UserSubscription> {
        let plans = self.plans.read().await;
        let plan = plans
            .get(&plan_id)
            .ok_or_else(|| anyhow::anyhow!("Plan not found"))?;

        let now = Utc::now();
        let days = duration_days.unwrap_or(plan.duration_days);

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

        // 初始化用户配额
        let quota = UserQuota {
            user_id,
            subscription_id: Some(subscription.id),
            plan_name: Some(plan.name.clone()),
            daily_requests_used: 0,
            daily_requests_limit: plan.features.max_requests_per_day,
            monthly_tokens_used: 0,
            monthly_tokens_limit: plan.features.max_tokens_per_month,
            allowed_models: plan.features.allowed_models.clone(),
            priority: plan.features.priority,
            rate_limit: plan.features.rate_limit,
            reset_daily_at: Some(now + Duration::days(1)),
            reset_monthly_at: Some(now + Duration::days(days as i64)),
        };

        let mut quotas = self.user_quotas.write().await;
        quotas.insert(user_id, quota);

        let mut subs = self.subscriptions.write().await;
        subs.insert(subscription.id, subscription.clone());

        Ok(subscription)
    }

    /// 取消订阅
    pub async fn cancel_subscription(&self, subscription_id: Uuid) -> Result<()> {
        let mut subs = self.subscriptions.write().await;
        if let Some(sub) = subs.get_mut(&subscription_id) {
            sub.status = SubscriptionStatus::Cancelled;
        }
        Ok(())
    }

    /// 检查用户订阅状态
    pub async fn check_subscription(&self, user_id: Uuid) -> Result<Option<UserSubscription>> {
        let subs = self.subscriptions.read().await;

        // 找到用户最新的活跃订阅
        Ok(subs
            .values()
            .filter(|s| s.user_id == user_id && s.status == SubscriptionStatus::Active)
            .max_by_key(|s| s.end_time)
            .cloned())
    }

    /// 续费订阅
    pub async fn renew_subscription(&self, subscription_id: Uuid) -> Result<UserSubscription> {
        let mut subs = self.subscriptions.write().await;

        if let Some(sub) = subs.get_mut(&subscription_id) {
            let plans = self.plans.read().await;
            let plan = plans
                .get(&sub.plan_id)
                .ok_or_else(|| anyhow::anyhow!("Plan not found"))?;

            sub.end_time += Duration::days(plan.duration_days as i64);
            sub.status = SubscriptionStatus::Active;

            return Ok(sub.clone());
        }

        bail!("Subscription not found")
    }

    /// 获取用户配额
    pub async fn get_user_quota(&self, user_id: Uuid) -> Result<UserQuota> {
        let quotas = self.user_quotas.read().await;

        if let Some(quota) = quotas.get(&user_id) {
            return Ok(quota.clone());
        }

        // 返回默认配额
        Ok(UserQuota {
            user_id,
            subscription_id: None,
            plan_name: None,
            daily_requests_used: 0,
            daily_requests_limit: Some(self.config.default_daily_limit),
            monthly_tokens_used: 0,
            monthly_tokens_limit: Some(self.config.default_monthly_limit),
            allowed_models: vec!["*".to_string()],
            priority: 0,
            rate_limit: 60,
            reset_daily_at: None,
            reset_monthly_at: None,
        })
    }

    /// 检查用户是否可以使用指定模型
    pub async fn can_use_model(&self, user_id: Uuid, model: &str) -> Result<bool> {
        let quota = self.get_user_quota(user_id).await?;

        // 检查模型是否在允许列表中
        if quota.allowed_models.contains(&"*".to_string()) {
            return Ok(true);
        }

        Ok(quota
            .allowed_models
            .iter()
            .any(|m| model.starts_with(m) || model == m))
    }

    /// 检查用户是否可以使用配额
    pub async fn check_quota(&self, user_id: Uuid, _tokens: i64) -> Result<QuotaCheckResult> {
        let quota = self.get_user_quota(user_id).await?;

        // 检查日限额
        if quota.is_daily_quota_exceeded() && !self.config.allow_over_quota {
            return Ok(QuotaCheckResult::DailyLimitExceeded {
                used: quota.daily_requests_used,
                limit: quota.daily_requests_limit.unwrap_or(0),
            });
        }

        // 检查月限额
        if quota.is_monthly_quota_exceeded() && !self.config.allow_over_quota {
            return Ok(QuotaCheckResult::MonthlyLimitExceeded {
                used: quota.monthly_tokens_used,
                limit: quota.monthly_tokens_limit.unwrap_or(0),
            });
        }

        // 检查警告阈值
        let warning = if let Some(percent) = quota.monthly_usage_percent() {
            percent >= self.config.quota_warning_threshold
        } else {
            false
        };

        Ok(QuotaCheckResult::Allowed {
            warning,
            daily_remaining: quota.daily_remaining(),
            monthly_remaining: quota.monthly_remaining(),
        })
    }

    /// 记录使用量
    pub async fn record_usage(
        &self,
        user_id: Uuid,
        requests: i32,
        tokens: i64,
        model: String,
    ) -> Result<()> {
        // 更新配额
        {
            let mut quotas = self.user_quotas.write().await;
            // 如果用户没有配额记录，创建一个默认的
            quotas.entry(user_id).or_insert_with(|| UserQuota {
                user_id,
                subscription_id: None,
                plan_name: None,
                daily_requests_used: 0,
                daily_requests_limit: Some(1000), // 默认每日限制
                monthly_tokens_used: 0,
                monthly_tokens_limit: Some(1_000_000), // 默认每月限制
                allowed_models: vec!["*".to_string()],
                priority: 0,
                rate_limit: 60,
                reset_daily_at: None,
                reset_monthly_at: None,
            });
            if let Some(quota) = quotas.get_mut(&user_id) {
                quota.daily_requests_used += requests;
                quota.monthly_tokens_used += tokens;
            }
        }

        // 记录使用历史
        {
            let record = QuotaUsageRecord {
                user_id,
                timestamp: Utc::now(),
                requests,
                tokens,
                model,
            };
            let mut records = self.usage_records.write().await;
            records.push(record);

            // 限制历史记录数量
            if records.len() > 10000 {
                records.remove(0);
            }
        }

        Ok(())
    }

    /// 重置日配额
    pub async fn reset_daily_quotas(&self) -> Result<usize> {
        let mut quotas = self.user_quotas.write().await;
        let now = Utc::now();
        let mut reset_count = 0;

        for quota in quotas.values_mut() {
            if let Some(reset_at) = quota.reset_daily_at {
                if now >= reset_at {
                    quota.daily_requests_used = 0;
                    quota.reset_daily_at = Some(now + Duration::days(1));
                    reset_count += 1;
                }
            }
        }

        Ok(reset_count)
    }

    /// 重置月配额
    pub async fn reset_monthly_quotas(&self) -> Result<usize> {
        let mut quotas = self.user_quotas.write().await;
        let now = Utc::now();
        let mut reset_count = 0;

        for quota in quotas.values_mut() {
            if let Some(reset_at) = quota.reset_monthly_at {
                if now >= reset_at {
                    quota.monthly_tokens_used = 0;
                    quota.reset_monthly_at = Some(now + Duration::days(30));
                    reset_count += 1;
                }
            }
        }

        Ok(reset_count)
    }

    /// 自动续费检查
    pub async fn check_auto_renewals(&self) -> Result<Vec<UserSubscription>> {
        let subs = self.subscriptions.read().await;
        let now = Utc::now();

        Ok(subs
            .values()
            .filter(|s| {
                s.auto_renew
                    && s.status == SubscriptionStatus::Active
                    && (s.end_time - now).num_days() <= 3
            })
            .cloned()
            .collect())
    }

    /// 处理过期订阅
    pub async fn handle_expired_subscriptions(&self) -> Result<Vec<Uuid>> {
        let mut subs = self.subscriptions.write().await;
        let now = Utc::now();
        let mut expired = Vec::new();

        for sub in subs.values_mut() {
            if sub.status == SubscriptionStatus::Active && sub.end_time <= now {
                sub.status = SubscriptionStatus::Expired;
                expired.push(sub.id);
            }
        }

        Ok(expired)
    }

    /// 获取使用统计
    pub async fn get_usage_stats(&self, user_id: Uuid) -> Result<UsageStats> {
        let records = self.usage_records.read().await;
        let user_records: Vec<_> = records.iter().filter(|r| r.user_id == user_id).collect();

        let total_requests: i32 = user_records.iter().map(|r| r.requests).sum();
        let total_tokens: i64 = user_records.iter().map(|r| r.tokens).sum();

        let mut model_usage: HashMap<String, i64> = HashMap::new();
        for record in user_records {
            *model_usage.entry(record.model.clone()).or_insert(0) += record.tokens;
        }

        Ok(UsageStats {
            total_requests,
            total_tokens,
            model_usage,
        })
    }
}

/// 配额检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuotaCheckResult {
    Allowed {
        warning: bool,
        daily_remaining: Option<i32>,
        monthly_remaining: Option<i64>,
    },
    DailyLimitExceeded {
        used: i32,
        limit: i32,
    },
    MonthlyLimitExceeded {
        used: i64,
        limit: i64,
    },
}

/// 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub total_requests: i32,
    pub total_tokens: i64,
    pub model_usage: HashMap<String, i64>,
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

    #[tokio::test]
    async fn test_subscription_service() {
        let service = SubscriptionService::new(SubscriptionConfig::default());

        // 创建计划
        let plan = service
            .create_plan(
                "Pro".to_string(),
                9900,
                30,
                PlanFeatures {
                    max_requests_per_day: Some(1000),
                    max_tokens_per_month: Some(10_000_000),
                    allowed_models: vec!["*".to_string()],
                    priority: 1,
                    rate_limit: 120,
                },
            )
            .await
            .unwrap();

        // 订阅
        let user_id = Uuid::new_v4();
        let sub = service.subscribe(user_id, plan.id, None).await.unwrap();

        assert_eq!(sub.status, SubscriptionStatus::Active);

        // 检查配额
        let quota = service.get_user_quota(user_id).await.unwrap();
        assert_eq!(quota.daily_requests_limit, Some(1000));
    }

    #[tokio::test]
    async fn test_quota_tracking() {
        let service = SubscriptionService::new(SubscriptionConfig::default());
        let user_id = Uuid::new_v4();

        // 记录使用
        service
            .record_usage(user_id, 10, 1000, "gpt-4".to_string())
            .await
            .unwrap();

        let quota = service.get_user_quota(user_id).await.unwrap();
        assert_eq!(quota.daily_requests_used, 10);
        assert_eq!(quota.monthly_tokens_used, 1000);
    }

    #[test]
    fn test_user_quota_methods() {
        let quota = UserQuota {
            user_id: Uuid::nil(),
            subscription_id: None,
            plan_name: None,
            daily_requests_used: 80,
            daily_requests_limit: Some(100),
            monthly_tokens_used: 50000,
            monthly_tokens_limit: Some(100000),
            allowed_models: vec!["*".to_string()],
            priority: 0,
            rate_limit: 60,
            reset_daily_at: None,
            reset_monthly_at: None,
        };

        assert!(!quota.is_daily_quota_exceeded());
        assert!(!quota.is_monthly_quota_exceeded());
        assert_eq!(quota.daily_remaining(), Some(20));
        assert_eq!(quota.monthly_remaining(), Some(50000));
        assert!(quota.daily_usage_percent().unwrap() > 0.0);
    }
}
