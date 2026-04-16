//! 计费服务 - 完整实现

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::entity::usages;
use crate::service::balance_ledger::BalanceLedgerService;

/// 使用记录参数
#[derive(Debug, Clone)]
pub struct RecordUsageParams {
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: i64,
    pub success: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UserStats {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DailyUsageStats {
    pub date: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cost: i64,
    pub cost_yuan: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UserUsageReport {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: i64,
    pub total_cost_yuan: f64,
    pub daily_usage: Vec<DailyUsageStats>,
}

pub struct BillingService {
    db: DatabaseConnection,
    rate_multiplier: f64,
    pricing_service: Arc<crate::service::pricing::PricingService>,
}

impl BillingService {
    pub fn new(db: DatabaseConnection, rate_multiplier: f64) -> Self {
        let pricing_service = Arc::new(crate::service::pricing::PricingService::new(db.clone()));
        Self {
            db,
            rate_multiplier,
            pricing_service,
        }
    }

    /// 记录用量
    pub async fn record_usage(&self, params: RecordUsageParams) -> Result<UsageRecord> {
        let cost = self.calculate_cost(&params.model, params.input_tokens, params.output_tokens);

        let usage = usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(params.user_id),
            api_key_id: Set(params.api_key_id),
            account_id: Set(None),
            model: Set(params.model),
            input_tokens: Set(params.input_tokens),
            output_tokens: Set(params.output_tokens),
            cost: Set(cost),
            request_id: Set(None),
            success: Set(params.success),
            error_message: Set(params.error_message),
            metadata: Set(None),
            created_at: Set(Utc::now()),
        };

        let usage = usage.insert(&self.db).await?;

        // 扣减余额 + record ledger entry atomically
        if cost > 0 {
            let ledger_service = BalanceLedgerService::new(self.db.clone());
            let _ = ledger_service
                .record(
                    params.user_id,
                    "usage",
                    Some(usage.id.to_string()),
                    -cost,
                    Some(format!(
                        "Usage: {} ({} tokens)",
                        usage.model,
                        usage.input_tokens + usage.output_tokens
                    )),
                    None,
                )
                .await;
        }

        Ok(UsageRecord {
            id: usage.id,
            user_id: usage.user_id,
            model: usage.model,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cost: usage.cost,
            success: usage.success,
            created_at: usage.created_at,
        })
    }

    /// 余额预检：检查用户余额是否足够处理请求
    /// 返回 Ok(()) 表示余额充足，Err 表示余额不足
    pub async fn check_balance(&self, user_id: Uuid) -> Result<()> {
        use crate::entity::users;

        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found: {user_id}"))?;

        if user.balance <= 0 {
            return Err(anyhow::anyhow!("Insufficient balance"));
        }

        Ok(())
    }

    /// 分组配额预检：检查分组的日/月配额是否超限
    /// 返回 Ok(()) 表示配额充足，Err 表示配额超限
    pub async fn check_group_quota(&self, group_id: i64) -> Result<()> {
        use crate::entity::{groups, usages};

        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found: {group_id}"))?;

        let now = chrono::Utc::now();

        // 检查日配额
        if let Some(daily_limit) = group.daily_limit_usd {
            if daily_limit > 0.0 {
                let today_start = now.date_naive().and_hms_opt(0, 0, 0)
                    .map(|t| chrono::TimeZone::from_utc_datetime(&chrono::Utc, &t));
                if let Some(today_start) = today_start {
                    let daily_cost: i64 = usages::Entity::find()
                        .filter(usages::Column::CreatedAt.gte(today_start))
                        .select_only()
                        .column_as(usages::Column::Cost.sum(), "total")
                        .into_tuple::<Option<i64>>()
                        .one(&self.db)
                        .await?
                        .flatten()
                        .unwrap_or(0);

                    // daily_limit_usd → 分: * 100
                    let limit_cents = (daily_limit * 100.0) as i64;
                    if daily_cost >= limit_cents {
                        return Err(anyhow::anyhow!("Daily quota exceeded for group {group_id}"));
                    }
                }
            }
        }

        // 检查月配额
        if let Some(monthly_limit) = group.monthly_limit_usd {
            if monthly_limit > 0.0 {
                let month_start = chrono::Utc
                    .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .single();
                if let Some(month_start) = month_start {
                    let monthly_cost: i64 = usages::Entity::find()
                        .filter(usages::Column::CreatedAt.gte(month_start))
                        .select_only()
                        .column_as(usages::Column::Cost.sum(), "total")
                        .into_tuple::<Option<i64>>()
                        .one(&self.db)
                        .await?
                        .flatten()
                        .unwrap_or(0);

                    let limit_cents = (monthly_limit * 100.0) as i64;
                    if monthly_cost >= limit_cents {
                        return Err(anyhow::anyhow!("Monthly quota exceeded for group {group_id}"));
                    }
                }
            }
        }

        Ok(())
    }

    /// 计算费用（单位：分）— 委托给 PricingService
    pub fn calculate_cost(&self, model: &str, input_tokens: i64, output_tokens: i64) -> i64 {
        // PricingService 是 async 的，这里用 blocking fallback 保持兼容
        // 热路径应直接调用 pricing_service.calculate_cost_simple()
        Self::calculate_cost_static(model, input_tokens, output_tokens, self.rate_multiplier)
    }

    /// 异步计算费用 — 走 DB 定价表
    pub async fn calculate_cost_async(
        &self,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> i64 {
        self.pricing_service
            .calculate_cost_simple(model, input_tokens, output_tokens, self.rate_multiplier)
            .await
    }

    /// 获取 PricingService 引用
    pub fn pricing_service(&self) -> &Arc<crate::service::pricing::PricingService> {
        &self.pricing_service
    }

    /// 计算费用（静态方法，用于测试和 fallback）
    pub fn calculate_cost_static(
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        rate_multiplier: f64,
    ) -> i64 {
        // 模型定价（每 1K tokens，单位：分）
        let (input_rate, output_rate) = match model {
            // Claude 3
            "claude-3-opus-20240229" => (1500, 7500),
            "claude-3-sonnet-20240229" => (300, 1500),
            "claude-3-haiku-20240307" => (25, 125),
            "claude-3-5-sonnet-20241022" => (300, 1500),

            // GPT-4
            "gpt-4-turbo" | "gpt-4-turbo-preview" | "gpt-4-0125-preview" => (1000, 3000),
            "gpt-4" | "gpt-4-0613" => (3000, 6000),
            "gpt-4o" | "gpt-4o-2024-11-20" => (250, 1000),
            "gpt-4o-mini" => (15, 60),
            "gpt-3.5-turbo" | "gpt-3.5-turbo-0125" => (50, 150),

            // Gemini
            "gemini-1.5-pro" | "gemini-1.5-pro-latest" => (350, 1050),
            "gemini-1.5-flash" | "gemini-1.5-flash-latest" => (35, 105),
            "gemini-2.0-flash-exp" => (0, 0), // 免费

            // DeepSeek
            "deepseek-chat" => (10, 30),
            "deepseek-reasoner" => (55, 220),

            // 默认
            _ => (100, 300),
        };

        let input_cost =
            (input_tokens as f64 / 1000.0 * input_rate as f64 * rate_multiplier) as i64;
        let output_cost =
            (output_tokens as f64 / 1000.0 * output_rate as f64 * rate_multiplier) as i64;

        input_cost + output_cost
    }

    /// 获取用户用量统计
    pub async fn get_user_stats(&self, user_id: Uuid, days: i32) -> Result<UserStats> {
        let usages = self
            .load_user_usage_since(user_id, Self::rolling_start_time(days))
            .await?;
        Ok(Self::summarize_usage(&usages))
    }

    /// 获取用户用量报表（含日维度聚合）
    pub async fn get_user_usage_report(&self, user_id: Uuid, days: i32) -> Result<UserUsageReport> {
        let days = days.max(1);
        let start_time = Self::report_start_time(days);
        let usages = self.load_user_usage_since(user_id, start_time).await?;
        let summary = Self::summarize_usage(&usages);
        let daily_usage = Self::build_daily_usage(&usages, start_time, days);

        Ok(UserUsageReport {
            total_requests: summary.total_requests,
            total_input_tokens: summary.total_input_tokens,
            total_output_tokens: summary.total_output_tokens,
            total_tokens: summary.total_input_tokens + summary.total_output_tokens,
            total_cost: summary.total_cost,
            total_cost_yuan: summary.total_cost as f64 / 100.0,
            daily_usage,
        })
    }

    /// 获取全局统计（管理后台）
    pub async fn get_global_stats(&self, days: i32) -> Result<UserStats> {
        let usages = self
            .load_global_usage_since(Self::rolling_start_time(days))
            .await?;
        Ok(Self::summarize_usage(&usages))
    }

    async fn load_user_usage_since(
        &self,
        user_id: Uuid,
        start_time: DateTime<Utc>,
    ) -> Result<Vec<usages::Model>> {
        Ok(usages::Entity::find()
            .filter(usages::Column::UserId.eq(user_id))
            .filter(usages::Column::CreatedAt.gte(start_time))
            .all(&self.db)
            .await?)
    }

    async fn load_global_usage_since(
        &self,
        start_time: DateTime<Utc>,
    ) -> Result<Vec<usages::Model>> {
        Ok(usages::Entity::find()
            .filter(usages::Column::CreatedAt.gte(start_time))
            .all(&self.db)
            .await?)
    }

    fn summarize_usage(usages: &[usages::Model]) -> UserStats {
        UserStats {
            total_requests: usages.len() as i64,
            total_input_tokens: usages.iter().map(|usage| usage.input_tokens).sum(),
            total_output_tokens: usages.iter().map(|usage| usage.output_tokens).sum(),
            total_cost: usages.iter().map(|usage| usage.cost).sum(),
        }
    }

    fn build_daily_usage(
        usages: &[usages::Model],
        start_time: DateTime<Utc>,
        days: i32,
    ) -> Vec<DailyUsageStats> {
        let mut grouped = BTreeMap::<String, DailyUsageStats>::new();

        for offset in 0..days {
            let date = (start_time + Duration::days(offset as i64))
                .format("%Y-%m-%d")
                .to_string();
            grouped.insert(
                date.clone(),
                DailyUsageStats {
                    date,
                    requests: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                    cost: 0,
                    cost_yuan: 0.0,
                },
            );
        }

        for usage in usages {
            let date = usage.created_at.format("%Y-%m-%d").to_string();
            if let Some(entry) = grouped.get_mut(&date) {
                entry.requests += 1;
                entry.input_tokens += usage.input_tokens;
                entry.output_tokens += usage.output_tokens;
                entry.total_tokens += usage.input_tokens + usage.output_tokens;
                entry.cost += usage.cost;
                entry.cost_yuan = entry.cost as f64 / 100.0;
            }
        }

        grouped.into_values().collect()
    }

    fn rolling_start_time(days: i32) -> DateTime<Utc> {
        Utc::now() - Duration::days(days.max(1) as i64)
    }

    fn report_start_time(days: i32) -> DateTime<Utc> {
        let start_time = Utc::now() - Duration::days(days.max(1) as i64 - 1);
        start_time
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
            .expect("valid start time")
    }
}
