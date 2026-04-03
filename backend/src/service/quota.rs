//! API Key 配额管理服务
//!
//! 提供配额管理、多窗口限流、IP 白名单/黑名单功能

#![allow(dead_code)]

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{DatabaseConnection, PaginatorTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 时间窗口类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimeWindow {
    FiveHours,
    OneDay,
    SevenDays,
}

impl TimeWindow {
    pub fn duration(&self) -> Duration {
        match self {
            TimeWindow::FiveHours => Duration::hours(5),
            TimeWindow::OneDay => Duration::days(1),
            TimeWindow::SevenDays => Duration::days(7),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TimeWindow::FiveHours => "5h",
            TimeWindow::OneDay => "1d",
            TimeWindow::SevenDays => "7d",
        }
    }
}

/// 配额配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    pub api_key_id: Uuid,
    pub quota_limit: f64,
    pub quota_used: f64,
    pub rate_limits: HashMap<String, f64>,
    pub ip_whitelist: Vec<String>,
    pub ip_blacklist: Vec<String>,
    pub window_usage: HashMap<String, WindowUsage>,
}

/// 窗口使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowUsage {
    pub window_type: TimeWindow,
    pub usage: f64,
    pub limit: f64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

/// 更新配额请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateQuotaRequest {
    pub quota_limit: Option<f64>,
    pub rate_limit_5h: Option<f64>,
    pub rate_limit_1d: Option<f64>,
    pub rate_limit_7d: Option<f64>,
    pub ip_whitelist: Option<Vec<String>>,
    pub ip_blacklist: Option<Vec<String>>,
}

/// 配额检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckResult {
    pub allowed: bool,
    pub reason: Option<String>,
    pub current_usage: f64,
    pub limit: f64,
    pub remaining: f64,
}

/// 配额使用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUsageRecord {
    pub id: Uuid,
    pub api_key_id: Uuid,
    pub amount: f64,
    pub model: String,
    pub tokens_used: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStats {
    pub total_users: u64,
    pub active_subscription_users: u64,
    pub total_quota: f64,
    pub total_used: f64,
    pub total_remaining: f64,
    pub average_usage: f64,
    pub utilization_rate: f64,
}

/// 配额服务
pub struct QuotaService {
    db: DatabaseConnection,
    usage_cache: Arc<RwLock<HashMap<Uuid, QuotaConfig>>>,
}

impl QuotaService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            usage_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置配额
    pub async fn set_quota(&self, api_key_id: Uuid, limit: f64) -> Result<()> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        let mut active: api_keys::ActiveModel = key.into();
        active.daily_quota = Set(Some(limit as i64));
        active.daily_used_quota = Set(Some(0));
        active.quota_reset_at = Set(Some(Utc::now() + Duration::days(1)));
        active.update(&self.db).await?;

        // 清除缓存
        self.usage_cache.write().await.remove(&api_key_id);

        Ok(())
    }

    /// 获取配额配置
    pub async fn get_quota_config(&self, api_key_id: Uuid) -> Result<Option<QuotaConfig>> {
        use crate::entity::api_keys;
        use sea_orm::EntityTrait;

        // 先检查缓存
        if let Some(config) = self.usage_cache.read().await.get(&api_key_id) {
            return Ok(Some(config.clone()));
        }

        // 从数据库查询
        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?;

        if let Some(key) = key {
            let config = QuotaConfig {
                api_key_id,
                quota_limit: key.daily_quota.unwrap_or(0) as f64,
                quota_used: key.daily_used_quota.unwrap_or(0) as f64,
                rate_limits: HashMap::new(),
                ip_whitelist: key
                    .ip_whitelist
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default(),
                ip_blacklist: vec![],
                window_usage: HashMap::new(),
            };

            // 更新缓存
            self.usage_cache
                .write()
                .await
                .insert(api_key_id, config.clone());

            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    /// 更新配额配置
    pub async fn update_quota(
        &self,
        api_key_id: Uuid,
        req: UpdateQuotaRequest,
    ) -> Result<QuotaConfig> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        let mut active: api_keys::ActiveModel = key.into();

        if let Some(limit) = req.quota_limit {
            active.daily_quota = Set(Some(limit as i64));
        }

        if let Some(ip_whitelist) = req.ip_whitelist {
            active.ip_whitelist = Set(Some(serde_json::to_value(ip_whitelist)?));
        }

        active.update(&self.db).await?;

        // 清除缓存
        self.usage_cache.write().await.remove(&api_key_id);

        self.get_quota_config(api_key_id)
            .await?
            .ok_or_else(|| anyhow!("Failed to get updated config"))
    }

    /// 检查配额
    pub async fn check_quota(
        &self,
        api_key_id: Uuid,
        estimated_cost: f64,
    ) -> Result<QuotaCheckResult> {
        let config = self.get_quota_config(api_key_id).await?;

        if let Some(config) = config {
            let remaining = config.quota_limit - config.quota_used;

            if remaining < estimated_cost {
                return Ok(QuotaCheckResult {
                    allowed: false,
                    reason: Some("Insufficient quota".to_string()),
                    current_usage: config.quota_used,
                    limit: config.quota_limit,
                    remaining,
                });
            }

            Ok(QuotaCheckResult {
                allowed: true,
                reason: None,
                current_usage: config.quota_used,
                limit: config.quota_limit,
                remaining,
            })
        } else {
            Ok(QuotaCheckResult {
                allowed: true,
                reason: None,
                current_usage: 0.0,
                limit: f64::MAX,
                remaining: f64::MAX,
            })
        }
    }

    /// 消费配额
    pub async fn consume_quota(
        &self,
        api_key_id: Uuid,
        amount: f64,
        model: &str,
        tokens: i64,
    ) -> Result<()> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        // 1. 更新 API Key 的配额使用量
        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        let current_used = key.daily_used_quota.unwrap_or(0) as f64;
        let new_used = current_used + amount;

        let mut active: api_keys::ActiveModel = key.into();
        active.daily_used_quota = Set(Some(new_used as i64));
        active.last_used_at = Set(Some(Utc::now()));
        active.update(&self.db).await?;

        // 2. 更新缓存
        if let Some(config) = self.usage_cache.write().await.get_mut(&api_key_id) {
            config.quota_used = new_used;
        }

        tracing::debug!(
            "Consumed quota: api_key={}, amount={}, tokens={}, model={}",
            api_key_id,
            amount,
            tokens,
            model
        );

        Ok(())
    }

    /// 设置速率限制
    pub async fn set_rate_limit(
        &self,
        api_key_id: Uuid,
        window: TimeWindow,
        limit: f64,
    ) -> Result<()> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        // 更新 RPM 限制（简化实现，只支持每分钟限制）
        if window == TimeWindow::FiveHours {
            // 实际存储到 rate_limit_rpm
            let mut active: api_keys::ActiveModel = key.into();
            active.rate_limit_rpm = Set(Some(limit as i32));
            active.update(&self.db).await?;
        }

        Ok(())
    }

    /// 检查速率限制
    pub async fn check_rate_limit(
        &self,
        api_key_id: Uuid,
        window: TimeWindow,
    ) -> Result<QuotaCheckResult> {
        // 获取窗口使用量
        let usage = self.get_window_usage(api_key_id, window.clone()).await?;

        Ok(QuotaCheckResult {
            allowed: usage.usage < usage.limit,
            reason: if usage.usage >= usage.limit {
                Some(format!(
                    "Rate limit exceeded for {} window",
                    window.as_str()
                ))
            } else {
                None
            },
            current_usage: usage.usage,
            limit: usage.limit,
            remaining: (usage.limit - usage.usage).max(0.0),
        })
    }

    /// 获取窗口使用量
    async fn get_window_usage(&self, api_key_id: Uuid, window: TimeWindow) -> Result<WindowUsage> {
        use crate::entity::usages;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let now = Utc::now();
        let window_start = now - window.duration();

        // 从数据库查询该时间窗口内的使用量
        let usages = usages::Entity::find()
            .filter(usages::Column::ApiKeyId.eq(api_key_id))
            .filter(usages::Column::CreatedAt.gte(window_start))
            .filter(usages::Column::CreatedAt.lte(now))
            .all(&self.db)
            .await?;

        let total_tokens: i64 = usages
            .iter()
            .map(|u| u.input_tokens + u.output_tokens)
            .sum();
        let _total_cost: i64 = usages.iter().map(|u| u.cost).sum();

        // 根据窗口类型设置限制
        let limit = match window {
            TimeWindow::FiveHours => 100000.0,  // 10万 tokens
            TimeWindow::OneDay => 500000.0,     // 50万 tokens
            TimeWindow::SevenDays => 3000000.0, // 300万 tokens
        };

        Ok(WindowUsage {
            window_type: window,
            usage: total_tokens as f64,
            limit,
            window_start,
            window_end: now,
        })
    }

    /// 设置 IP 白名单
    pub async fn set_ip_whitelist(&self, api_key_id: Uuid, ips: Vec<String>) -> Result<()> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        self.validate_ip_list(&ips)?;

        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        let mut active: api_keys::ActiveModel = key.into();
        active.ip_whitelist = Set(Some(serde_json::to_value(ips)?));
        active.update(&self.db).await?;

        // 清除缓存
        self.usage_cache.write().await.remove(&api_key_id);

        Ok(())
    }

    /// 设置 IP 黑名单
    pub async fn set_ip_blacklist(&self, api_key_id: Uuid, ips: Vec<String>) -> Result<()> {
        self.validate_ip_list(&ips)?;

        // 注意：当前 api_keys 表没有 ip_blacklist 字段
        // 这里简化实现，实际需要扩展数据库或使用其他存储
        tracing::warn!("IP blacklist set for api_key {}: {:?}", api_key_id, ips);

        Ok(())
    }

    /// 检查 IP 是否允许
    pub async fn check_ip_allowed(&self, api_key_id: Uuid, ip: &str) -> Result<bool> {
        let config = self.get_quota_config(api_key_id).await?;

        if let Some(config) = config {
            // 检查黑名单
            if config.ip_blacklist.contains(&ip.to_string()) {
                return Ok(false);
            }

            // 检查白名单
            if !config.ip_whitelist.is_empty() {
                return Ok(config.ip_whitelist.contains(&ip.to_string()));
            }
        }

        Ok(true)
    }

    /// 验证 IP 列表格式
    fn validate_ip_list(&self, ips: &[String]) -> Result<()> {
        for ip in ips {
            if ip.parse::<std::net::IpAddr>().is_err() {
                bail!("Invalid IP address: {}", ip);
            }
        }
        Ok(())
    }

    /// 获取使用历史
    pub async fn get_usage_history(
        &self,
        api_key_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<QuotaUsageRecord>> {
        use crate::entity::usages;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let records = usages::Entity::find()
            .filter(usages::Column::ApiKeyId.eq(api_key_id))
            .filter(usages::Column::CreatedAt.gte(start_time))
            .filter(usages::Column::CreatedAt.lte(end_time))
            .order_by_desc(usages::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(records
            .into_iter()
            .map(|r| QuotaUsageRecord {
                id: r.id,
                api_key_id: r.api_key_id,
                amount: r.cost as f64 / 100.0, // 转换为元
                model: r.model,
                tokens_used: r.input_tokens + r.output_tokens,
                timestamp: r.created_at,
            })
            .collect())
    }

    /// 获取管理员视图的配额统计
    pub async fn get_stats(&self) -> Result<QuotaStats> {
        use crate::entity::{subscriptions, users};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let total_users = users::Entity::find().count(&self.db).await?;
        let active_subscriptions = subscriptions::Entity::find()
            .filter(subscriptions::Column::Status.eq("active"))
            .all(&self.db)
            .await?;

        let active_subscription_users = active_subscriptions
            .iter()
            .filter(|subscription| subscription.is_active())
            .count() as u64;

        let total_quota: f64 = active_subscriptions
            .iter()
            .filter(|subscription| subscription.is_active())
            .map(|subscription| decimal_to_f64(subscription.quota_limit))
            .sum();

        let total_used: f64 = active_subscriptions
            .iter()
            .filter(|subscription| subscription.is_active())
            .map(|subscription| decimal_to_f64(subscription.quota_used))
            .sum();

        let total_remaining = (total_quota - total_used).max(0.0);
        let average_usage = if active_subscription_users == 0 {
            0.0
        } else {
            total_used / active_subscription_users as f64
        };
        let utilization_rate = if total_quota <= 0.0 {
            0.0
        } else {
            total_used / total_quota
        };

        Ok(QuotaStats {
            total_users,
            active_subscription_users,
            total_quota,
            total_used,
            total_remaining,
            average_usage,
            utilization_rate,
        })
    }

    /// 重置配额
    pub async fn reset_quota(&self, api_key_id: Uuid) -> Result<()> {
        use crate::entity::api_keys;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};

        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("API key not found"))?;

        let mut active: api_keys::ActiveModel = key.into();
        active.daily_used_quota = Set(Some(0));
        active.quota_reset_at = Set(Some(Utc::now() + Duration::days(1)));
        active.update(&self.db).await?;

        // 清除缓存
        self.usage_cache.write().await.remove(&api_key_id);

        Ok(())
    }

    /// 清理过期窗口数据
    pub async fn cleanup_expired_windows(&self) -> Result<i64> {
        use crate::entity::usages;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        // 删除 30 天前的使用记录
        let cutoff = Utc::now() - Duration::days(30);

        let result = usages::Entity::delete_many()
            .filter(usages::Column::CreatedAt.lt(cutoff))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected as i64)
    }
}

fn decimal_to_f64(value: rust_decimal::Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_window_duration() {
        assert_eq!(TimeWindow::FiveHours.duration(), Duration::hours(5));
        assert_eq!(TimeWindow::OneDay.duration(), Duration::days(1));
        assert_eq!(TimeWindow::SevenDays.duration(), Duration::days(7));
    }
}
