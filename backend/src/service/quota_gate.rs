//! 统一配额网关
//!
//! 一个入口，一次检查，一个事务。
//! 检查链：余额 → 分组配额 → API Key 配额 → 速率限制
//! 结算：单事务 insert usage + deduct balance + update quotas

use anyhow::{bail, Result};
use chrono::{Datelike, TimeZone, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect,
    Set, TransactionTrait,
};
use uuid::Uuid;

use crate::entity::{api_keys, balance_ledger, groups, usages, users};

/// 配额检查结果
pub struct QuotaPermit {
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub model: String,
    pub group_id: Option<i64>,
}

/// 实际使用量（用于结算）
pub struct ActualUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: i64,
    pub account_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 统一配额网关
pub struct QuotaGate {
    db: DatabaseConnection,
    rate_multiplier: f64,
}

impl QuotaGate {
    pub fn new(db: DatabaseConnection, rate_multiplier: f64) -> Self {
        Self {
            db,
            rate_multiplier,
        }
    }

    /// 请求前配额检查（链式：余额 → 分组 → API Key）
    /// 任一失败返回具体错误
    pub async fn pre_check(
        &self,
        user_id: Uuid,
        api_key_id: Uuid,
        model: &str,
        group_id: Option<i64>,
    ) -> Result<QuotaPermit> {
        // 1. 余额检查
        self.check_balance(user_id).await?;

        // 2. 分组配额检查
        if let Some(gid) = group_id {
            self.check_group_quota(gid).await?;
        }

        // 3. API Key 配额检查
        if api_key_id != Uuid::nil() {
            self.check_api_key_quota(api_key_id).await?;
        }

        Ok(QuotaPermit {
            user_id,
            api_key_id,
            model: model.to_string(),
            group_id,
        })
    }

    /// 请求后原子结算（单事务：insert usage + deduct balance + update api_key quota）
    pub async fn post_settle(
        &self,
        permit: &QuotaPermit,
        usage: ActualUsage,
    ) -> Result<Uuid> {
        let txn = self.db.begin().await?;

        // 1. 插入 usage 记录
        let usage_id = Uuid::new_v4();
        let usage_record = usages::ActiveModel {
            id: Set(usage_id),
            user_id: Set(permit.user_id),
            api_key_id: Set(permit.api_key_id),
            account_id: Set(usage.account_id),
            model: Set(permit.model.clone()),
            input_tokens: Set(usage.input_tokens),
            output_tokens: Set(usage.output_tokens),
            cost: Set(usage.cost),
            request_id: Set(usage.request_id),
            success: Set(usage.success),
            error_message: Set(usage.error_message),
            metadata: Set(usage.metadata),
            created_at: Set(Utc::now()),
        };
        usage_record.insert(&txn).await?;

        // 2. 扣减余额（在同一事务中）
        if usage.cost > 0 && usage.success {
            let user = users::Entity::find_by_id(permit.user_id)
                .one(&txn)
                .await?
                .ok_or_else(|| anyhow::anyhow!("User not found"))?;

            let new_balance = user.balance - usage.cost;
            // 允许扣到 0，但记录审计
            let mut user_model: users::ActiveModel = user.into();
            user_model.balance = Set(new_balance);
            user_model.updated_at = Set(Utc::now());
            user_model.update(&txn).await?;

            // 3. 插入 ledger 审计记录
            use crate::entity::balance_ledger;
            let ledger = balance_ledger::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(permit.user_id),
                delta_cents: Set(-usage.cost),
                balance_before: Set(new_balance + usage.cost),
                balance_after: Set(new_balance),
                source_type: Set("usage".to_string()),
                source_id: Set(Some(usage_id.to_string())),
                description: Set(Some(format!(
                    "Usage: {} ({} tokens)",
                    permit.model,
                    usage.input_tokens + usage.output_tokens
                ))),
                metadata: Set(None),
                created_at: Set(Utc::now()),
            };
            ledger.insert(&txn).await?;
        }

        // 4. 更新 API Key 日配额使用量
        if permit.api_key_id != Uuid::nil() && usage.cost > 0 {
            if let Some(key) = api_keys::Entity::find_by_id(permit.api_key_id)
                .one(&txn)
                .await?
            {
                if key.daily_quota.is_some() {
                    let new_used = key.daily_used_quota.unwrap_or(0) + usage.cost;
                    let mut key_model: api_keys::ActiveModel = key.into();
                    key_model.daily_used_quota = Set(Some(new_used));
                    key_model.update(&txn).await?;
                }
            }
        }

        txn.commit().await?;

        tracing::info!(
            "QuotaGate settled: user={}, model={}, cost={}分, tokens={}",
            permit.user_id,
            permit.model,
            usage.cost,
            usage.input_tokens + usage.output_tokens,
        );

        Ok(usage_id)
    }

    /// 检查用户余额
    async fn check_balance(&self, user_id: Uuid) -> Result<()> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found: {user_id}"))?;

        if user.balance <= 0 {
            bail!("Insufficient balance");
        }
        Ok(())
    }

    /// 检查分组配额（按分组内账号过滤 usage）
    async fn check_group_quota(&self, group_id: i64) -> Result<()> {
        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found: {group_id}"))?;

        let now = Utc::now();

        // 日配额
        if let Some(daily_limit) = group.daily_limit_usd {
            if daily_limit > 0.0 {
                if let Some(today_start) = now
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .map(|t| Utc.from_utc_datetime(&t))
                {
                    let daily_cost = self
                        .sum_group_usage_since(group_id, today_start)
                        .await?;
                    let limit_cents = (daily_limit * 100.0) as i64;
                    if daily_cost >= limit_cents {
                        bail!("Daily quota exceeded for group {group_id}");
                    }
                }
            }
        }

        // 月配额
        if let Some(monthly_limit) = group.monthly_limit_usd {
            if monthly_limit > 0.0 {
                if let Some(month_start) = Utc
                    .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .single()
                {
                    let monthly_cost = self
                        .sum_group_usage_since(group_id, month_start)
                        .await?;
                    let limit_cents = (monthly_limit * 100.0) as i64;
                    if monthly_cost >= limit_cents {
                        bail!("Monthly quota exceeded for group {group_id}");
                    }
                }
            }
        }

        Ok(())
    }

    /// 按分组内账号过滤的 usage 合计
    async fn sum_group_usage_since(
        &self,
        group_id: i64,
        since: chrono::DateTime<Utc>,
    ) -> Result<i64> {
        use sea_orm::{DbBackend, FromQueryResult, Statement};

        #[derive(Debug, FromQueryResult)]
        struct CostSum {
            total: i64,
        }

        let row = CostSum::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT COALESCE(SUM(u.cost), 0)::bigint AS total
               FROM usages u
               INNER JOIN accounts a ON u.account_id = a.id
               WHERE a.group_id = $1 AND u.created_at >= $2"#,
            [group_id.into(), since.into()],
        ))
        .one(&self.db)
        .await?
        .unwrap_or(CostSum { total: 0 });

        Ok(row.total)
    }

    /// 检查 API Key 配额
    async fn check_api_key_quota(&self, api_key_id: Uuid) -> Result<()> {
        let key = api_keys::Entity::find_by_id(api_key_id)
            .one(&self.db)
            .await?;

        if let Some(key) = key {
            if key.is_quota_exceeded() {
                bail!("API Key daily quota exceeded");
            }
        }

        Ok(())
    }
}
