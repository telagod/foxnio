//! 分组服务 - 账号分组管理
//!
//! 提供分组创建、管理、模型路由和配额分发功能

#![allow(dead_code)]
use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::entity::{account_groups, accounts, groups};

// ============ 请求/响应结构体 ============

/// 创建分组请求
#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub platform: String,
    pub daily_limit_usd: Option<f64>,
    pub weekly_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub rate_multiplier: Option<f64>,
    pub fallback_group_id: Option<i64>,
    pub model_routing: Option<HashMap<String, Vec<i64>>>,
    pub model_routing_enabled: Option<bool>,
    pub claude_code_only: Option<bool>,
    pub is_exclusive: Option<bool>,
    pub sort_order: Option<i32>,
}

/// 更新分组请求
#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub daily_limit_usd: Option<f64>,
    pub weekly_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub rate_multiplier: Option<f64>,
    pub fallback_group_id: Option<i64>,
    pub model_routing: Option<HashMap<String, Vec<i64>>>,
    pub model_routing_enabled: Option<bool>,
    pub claude_code_only: Option<bool>,
    pub fallback_group_id_on_invalid_request: Option<i64>,
    pub is_exclusive: Option<bool>,
    pub sort_order: Option<i32>,
}

/// 分组信息响应
#[derive(Debug, Serialize)]
pub struct GroupInfo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub platform: String,
    pub status: String,
    pub daily_limit_usd: Option<f64>,
    pub weekly_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub rate_multiplier: f64,
    pub model_routing: Option<HashMap<String, Vec<i64>>>,
    pub model_routing_enabled: bool,
    pub fallback_group_id: Option<i64>,
    pub claude_code_only: bool,
    pub is_exclusive: bool,
    pub sort_order: i32,
    pub account_count: i64,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

/// 添加账号到分组请求
#[derive(Debug, Deserialize)]
pub struct AddAccountToGroupRequest {
    pub account_id: Uuid,
    pub group_id: i64,
}

/// 从分组移除账号请求
#[derive(Debug, Deserialize)]
pub struct RemoveAccountFromGroupRequest {
    pub account_id: Uuid,
    pub group_id: i64,
}

// ============ GroupService ============

pub struct GroupService {
    db: DatabaseConnection,
}

impl GroupService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 创建分组
    pub async fn create_group(&self, req: CreateGroupRequest) -> Result<GroupInfo> {
        let now = Utc::now();
        let model_routing_json = req.model_routing.map(|m| serde_json::to_value(m).unwrap());

        let group = groups::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            name: Set(req.name),
            description: Set(req.description),
            platform: Set(req.platform),
            status: Set("active".to_string()),
            daily_limit_usd: Set(req.daily_limit_usd),
            weekly_limit_usd: Set(req.weekly_limit_usd),
            monthly_limit_usd: Set(req.monthly_limit_usd),
            rate_multiplier: Set(req.rate_multiplier.unwrap_or(1.0)),
            model_routing: Set(model_routing_json),
            model_routing_enabled: Set(req.model_routing_enabled.unwrap_or(false)),
            fallback_group_id: Set(req.fallback_group_id),
            claude_code_only: Set(req.claude_code_only.unwrap_or(false)),
            fallback_group_id_on_invalid_request: Set(None),
            supported_model_scopes: Set(None),
            sort_order: Set(req.sort_order.unwrap_or(0)),
            is_exclusive: Set(req.is_exclusive.unwrap_or(false)),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
        };

        let group = group.insert(&self.db).await?;

        Ok(GroupInfo {
            id: group.id,
            name: group.name,
            description: group.description,
            platform: group.platform,
            status: group.status,
            daily_limit_usd: group.daily_limit_usd,
            weekly_limit_usd: group.weekly_limit_usd,
            monthly_limit_usd: group.monthly_limit_usd,
            rate_multiplier: group.rate_multiplier,
            model_routing: group
                .model_routing
                .and_then(|v| serde_json::from_value(v).ok()),
            model_routing_enabled: group.model_routing_enabled,
            fallback_group_id: group.fallback_group_id,
            claude_code_only: group.claude_code_only,
            is_exclusive: group.is_exclusive,
            sort_order: group.sort_order,
            account_count: 0,
            created_at: group.created_at,
            updated_at: group.updated_at,
        })
    }

    /// 获取分组详情
    pub async fn get_group(&self, group_id: i64) -> Result<Option<GroupInfo>> {
        let group = groups::Entity::find_by_id(group_id).one(&self.db).await?;

        if let Some(g) = group {
            // 统计分组内的账号数量
            let account_count = account_groups::Entity::find()
                .filter(account_groups::Column::GroupId.eq(g.id))
                .count(&self.db)
                .await? as i64;

            Ok(Some(GroupInfo {
                id: g.id,
                name: g.name,
                description: g.description,
                platform: g.platform,
                status: g.status,
                daily_limit_usd: g.daily_limit_usd,
                weekly_limit_usd: g.weekly_limit_usd,
                monthly_limit_usd: g.monthly_limit_usd,
                rate_multiplier: g.rate_multiplier,
                model_routing: g.model_routing.and_then(|v| serde_json::from_value(v).ok()),
                model_routing_enabled: g.model_routing_enabled,
                fallback_group_id: g.fallback_group_id,
                claude_code_only: g.claude_code_only,
                is_exclusive: g.is_exclusive,
                sort_order: g.sort_order,
                account_count,
                created_at: g.created_at,
                updated_at: g.updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// 列出所有分组
    pub async fn list_groups(&self, platform: Option<&str>) -> Result<Vec<GroupInfo>> {
        let mut query = groups::Entity::find()
            .filter(groups::Column::DeletedAt.is_null())
            .order_by_asc(groups::Column::SortOrder)
            .order_by_asc(groups::Column::Id);

        if let Some(p) = platform {
            query = query.filter(groups::Column::Platform.eq(p));
        }

        let groups = query.all(&self.db).await?;

        let mut result = Vec::new();
        for g in groups {
            let account_count = account_groups::Entity::find()
                .filter(account_groups::Column::GroupId.eq(g.id))
                .count(&self.db)
                .await? as i64;

            result.push(GroupInfo {
                id: g.id,
                name: g.name,
                description: g.description,
                platform: g.platform,
                status: g.status,
                daily_limit_usd: g.daily_limit_usd,
                weekly_limit_usd: g.weekly_limit_usd,
                monthly_limit_usd: g.monthly_limit_usd,
                rate_multiplier: g.rate_multiplier,
                model_routing: g
                    .model_routing
                    .clone()
                    .and_then(|v| serde_json::from_value(v).ok()),
                model_routing_enabled: g.model_routing_enabled,
                fallback_group_id: g.fallback_group_id,
                claude_code_only: g.claude_code_only,
                is_exclusive: g.is_exclusive,
                sort_order: g.sort_order,
                account_count,
                created_at: g.created_at,
                updated_at: g.updated_at,
            });
        }

        Ok(result)
    }

    /// 更新分组
    pub async fn update_group(
        &self,
        group_id: i64,
        req: UpdateGroupRequest,
    ) -> Result<Option<GroupInfo>> {
        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

        let mut group: groups::ActiveModel = group.into();

        if let Some(name) = req.name {
            group.name = Set(name);
        }
        if let Some(description) = req.description {
            group.description = Set(Some(description));
        }
        if let Some(status) = req.status {
            group.status = Set(status);
        }
        if let Some(daily_limit_usd) = req.daily_limit_usd {
            group.daily_limit_usd = Set(Some(daily_limit_usd));
        }
        if let Some(weekly_limit_usd) = req.weekly_limit_usd {
            group.weekly_limit_usd = Set(Some(weekly_limit_usd));
        }
        if let Some(monthly_limit_usd) = req.monthly_limit_usd {
            group.monthly_limit_usd = Set(Some(monthly_limit_usd));
        }
        if let Some(rate_multiplier) = req.rate_multiplier {
            group.rate_multiplier = Set(rate_multiplier);
        }
        if let Some(fallback_group_id) = req.fallback_group_id {
            group.fallback_group_id = Set(Some(fallback_group_id));
        }
        if let Some(model_routing) = req.model_routing {
            group.model_routing = Set(Some(serde_json::to_value(model_routing)?));
        }
        if let Some(model_routing_enabled) = req.model_routing_enabled {
            group.model_routing_enabled = Set(model_routing_enabled);
        }
        if let Some(claude_code_only) = req.claude_code_only {
            group.claude_code_only = Set(claude_code_only);
        }
        if let Some(fallback_group_id_on_invalid_request) = req.fallback_group_id_on_invalid_request
        {
            group.fallback_group_id_on_invalid_request =
                Set(Some(fallback_group_id_on_invalid_request));
        }
        if let Some(is_exclusive) = req.is_exclusive {
            group.is_exclusive = Set(is_exclusive);
        }
        if let Some(sort_order) = req.sort_order {
            group.sort_order = Set(sort_order);
        }

        group.updated_at = Set(Utc::now());
        let group = group.update(&self.db).await?;

        self.get_group(group.id).await
    }

    /// 删除分组（软删除）
    pub async fn delete_group(&self, group_id: i64) -> Result<()> {
        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

        let mut group: groups::ActiveModel = group.into();
        group.deleted_at = Set(Some(Utc::now()));
        group.updated_at = Set(Utc::now());
        group.update(&self.db).await?;

        Ok(())
    }

    /// 添加账号到分组
    pub async fn add_account_to_group(&self, account_id: Uuid, group_id: i64) -> Result<()> {
        // 检查账号是否存在
        let _account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        // 检查分组是否存在
        let _group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

        // 检查是否已在分组中
        let existing = account_groups::Entity::find()
            .filter(account_groups::Column::AccountId.eq(account_id))
            .filter(account_groups::Column::GroupId.eq(group_id))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Ok(()); // 已存在，忽略
        }

        // 创建关联
        let association = account_groups::ActiveModel {
            account_id: Set(account_id),
            group_id: Set(group_id),
            created_at: Set(Utc::now()),
        };

        association.insert(&self.db).await?;

        Ok(())
    }

    /// 从分组移除账号
    pub async fn remove_account_from_group(&self, account_id: Uuid, group_id: i64) -> Result<()> {
        let association = account_groups::Entity::find()
            .filter(account_groups::Column::AccountId.eq(account_id))
            .filter(account_groups::Column::GroupId.eq(group_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not in group"))?;

        association.delete(&self.db).await?;

        Ok(())
    }

    /// 获取分组内的账号列表
    pub async fn get_group_accounts(&self, group_id: i64) -> Result<Vec<accounts::Model>> {
        let account_ids: Vec<Uuid> = account_groups::Entity::find()
            .filter(account_groups::Column::GroupId.eq(group_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|ag| ag.account_id)
            .collect();

        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids))
            .order_by_desc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts)
    }

    /// 根据模型选择分组内的账号（支持模型路由）
    pub fn select_account_for_model<'a>(
        &'a self,
        group_id: i64,
        model: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<accounts::Model>>> + 'a>>
    {
        Box::pin(async move {
            let group = groups::Entity::find_by_id(group_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

            // 如果启用了模型路由，优先使用路由配置
            if group.model_routing_enabled {
                if let Some(ref routing_json) = group.model_routing {
                    let routing: HashMap<String, Vec<i64>> =
                        serde_json::from_value(routing_json.clone())?;

                    // 查找匹配的路由规则
                    for (pattern, account_ids) in routing.iter() {
                        if self.match_model_pattern(model, pattern) {
                            // 按顺序查找可用账号
                            for &account_id in account_ids {
                                if let Some(account) =
                                    self.get_available_account_by_id(account_id).await?
                                {
                                    return Ok(Some(account));
                                }
                            }
                        }
                    }
                }
            }

            // 未找到路由或路由未启用，按优先级选择账号
            let accounts = self.get_group_accounts(group_id).await?;
            for account in accounts {
                if account.is_active() {
                    return Ok(Some(account));
                }
            }

            // 如果有降级分组，尝试降级
            if let Some(fallback_id) = group.fallback_group_id {
                return self.select_account_for_model(fallback_id, model).await;
            }

            Ok(None)
        })
    }

    /// 匹配模型模式
    fn match_model_pattern(&self, model: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // 支持前缀匹配
        if let Some(prefix) = pattern.strip_suffix('*') {
            return model.starts_with(prefix);
        }

        // 精确匹配
        model == pattern
    }

    /// 根据 ID 获取可用账号
    async fn get_available_account_by_id(
        &self,
        account_id: i64,
    ) -> Result<Option<accounts::Model>> {
        // 注意：accounts 表的 ID 是 UUID，这里需要转换
        let account = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .filter(accounts::Column::Id.eq(Uuid::from_u128(account_id as u128)))
            .one(&self.db)
            .await?;

        Ok(account)
    }

    /// 获取平台的默认分组
    pub async fn get_default_group_for_platform(
        &self,
        platform: &str,
    ) -> Result<Option<groups::Model>> {
        let group = groups::Entity::find()
            .filter(groups::Column::Platform.eq(platform))
            .filter(groups::Column::Status.eq("active"))
            .filter(groups::Column::DeletedAt.is_null())
            .order_by_asc(groups::Column::SortOrder)
            .one(&self.db)
            .await?;

        Ok(group)
    }

    /// 检查分组配额
    pub async fn check_group_quota(&self, group_id: i64) -> Result<GroupQuotaStatus> {
        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

        // Query daily / weekly / monthly usage for accounts in this group.
        // usages.cost is stored in cents; group limits are in USD (f64).
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN u.created_at >= CURRENT_DATE THEN u.cost ELSE 0 END), 0) AS daily_cost,
                    COALESCE(SUM(CASE WHEN u.created_at >= date_trunc('week', CURRENT_DATE) THEN u.cost ELSE 0 END), 0) AS weekly_cost,
                    COALESCE(SUM(CASE WHEN u.created_at >= date_trunc('month', CURRENT_DATE) THEN u.cost ELSE 0 END), 0) AS monthly_cost
                FROM usages u
                INNER JOIN account_groups ag ON ag.account_id = u.account_id
                WHERE ag.group_id = $1
                "#,
                [group_id.into()],
            ))
            .await?;

        let (daily_used, weekly_used, monthly_used) = match row {
            Some(ref r) => {
                let d: i64 = r.try_get_by_index(0).unwrap_or(0);
                let w: i64 = r.try_get_by_index(1).unwrap_or(0);
                let m: i64 = r.try_get_by_index(2).unwrap_or(0);
                (d as f64 / 100.0, w as f64 / 100.0, m as f64 / 100.0)
            }
            None => (0.0, 0.0, 0.0),
        };

        let is_over_limit = group.daily_limit_usd.map_or(false, |lim| daily_used >= lim)
            || group
                .weekly_limit_usd
                .map_or(false, |lim| weekly_used >= lim)
            || group
                .monthly_limit_usd
                .map_or(false, |lim| monthly_used >= lim);

        Ok(GroupQuotaStatus {
            group_id: group.id,
            group_name: group.name,
            daily_limit: group.daily_limit_usd,
            daily_used,
            weekly_limit: group.weekly_limit_usd,
            monthly_limit: group.monthly_limit_usd,
            monthly_used,
            is_over_limit,
        })
    }

    // ========================================================================
    // 用户端方法
    // ========================================================================

    /// 列出用户可用分组
    pub async fn list_available_groups(&self, platform: Option<&str>) -> Result<Vec<GroupInfo>> {
        self.list_groups(platform).await
    }

    /// 获取分组费率信息
    pub async fn get_group_rates(&self, platform: Option<&str>) -> Result<Vec<GroupRateInfo>> {
        let groups = self.list_groups(platform).await?;

        Ok(groups
            .into_iter()
            .map(|g| {
                // Extract model names from model_routing keys
                let models = g
                    .model_routing
                    .as_ref()
                    .map(|routing| routing.keys().cloned().collect::<Vec<String>>())
                    .unwrap_or_default();

                GroupRateInfo {
                    group_id: g.id,
                    group_name: g.name,
                    platform: g.platform,
                    rate_multiplier: g.rate_multiplier,
                    models,
                }
            })
            .collect())
    }

    // ========================================================================
    // 管理端扩展方法
    // ========================================================================

    /// 获取使用摘要
    pub async fn get_usage_summary(&self) -> Result<Vec<GroupUsageSummary>> {
        // Single query: per-group daily + monthly usage, account counts
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    g.id,
                    g.name,
                    g.platform,
                    g.daily_limit_usd,
                    g.monthly_limit_usd,
                    COALESCE(SUM(CASE WHEN u.created_at >= CURRENT_DATE THEN u.cost ELSE 0 END), 0) AS daily_cost,
                    COALESCE(SUM(CASE WHEN u.created_at >= date_trunc('month', CURRENT_DATE) THEN u.cost ELSE 0 END), 0) AS monthly_cost,
                    COUNT(DISTINCT ag.account_id) AS account_count,
                    COUNT(DISTINCT CASE WHEN a.status = 'active' THEN a.id END) AS active_account_count
                FROM groups g
                LEFT JOIN account_groups ag ON ag.group_id = g.id
                LEFT JOIN accounts a ON a.id = ag.account_id
                LEFT JOIN usages u ON u.account_id = ag.account_id
                WHERE g.deleted_at IS NULL
                GROUP BY g.id, g.name, g.platform, g.daily_limit_usd, g.monthly_limit_usd
                ORDER BY g.sort_order, g.id
                "#,
                [],
            ))
            .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in &rows {
            let group_id: i64 = row.try_get_by_index(0)?;
            let group_name: String = row.try_get_by_index(1)?;
            let platform: String = row.try_get_by_index(2)?;
            let daily_limit: Option<f64> = row.try_get_by_index(3)?;
            let monthly_limit: Option<f64> = row.try_get_by_index(4)?;
            let daily_cost: i64 = row.try_get_by_index(5).unwrap_or(0);
            let monthly_cost: i64 = row.try_get_by_index(6).unwrap_or(0);
            let acct_count: i64 = row.try_get_by_index(7).unwrap_or(0);
            let active_count: i64 = row.try_get_by_index(8).unwrap_or(0);

            result.push(GroupUsageSummary {
                group_id,
                group_name,
                platform,
                daily_used_usd: daily_cost as f64 / 100.0,
                daily_limit_usd: daily_limit.unwrap_or(0.0),
                monthly_used_usd: monthly_cost as f64 / 100.0,
                monthly_limit_usd: monthly_limit.unwrap_or(0.0),
                account_count: acct_count,
                active_account_count: active_count,
            });
        }
        Ok(result)
    }

    /// 获取容量摘要
    pub async fn get_capacity_summary(&self) -> Result<Vec<GroupCapacitySummary>> {
        // Per-group: total accounts, active accounts, daily usage vs daily limit
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    g.id,
                    g.name,
                    g.platform,
                    g.daily_limit_usd,
                    COUNT(DISTINCT ag.account_id) AS total_accounts,
                    COUNT(DISTINCT CASE WHEN a.status = 'active' THEN a.id END) AS active_accounts,
                    COALESCE(SUM(CASE WHEN u.created_at >= CURRENT_DATE THEN u.cost ELSE 0 END), 0) AS daily_cost
                FROM groups g
                LEFT JOIN account_groups ag ON ag.group_id = g.id
                LEFT JOIN accounts a ON a.id = ag.account_id
                LEFT JOIN usages u ON u.account_id = ag.account_id
                WHERE g.deleted_at IS NULL
                GROUP BY g.id, g.name, g.platform, g.daily_limit_usd
                ORDER BY g.sort_order, g.id
                "#,
                [],
            ))
            .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in &rows {
            let group_id: i64 = row.try_get_by_index(0)?;
            let group_name: String = row.try_get_by_index(1)?;
            let platform: String = row.try_get_by_index(2)?;
            let daily_limit: Option<f64> = row.try_get_by_index(3)?;
            let total_accounts: i64 = row.try_get_by_index(4).unwrap_or(0);
            let _active_accounts: i64 = row.try_get_by_index(5).unwrap_or(0);
            let daily_cost: i64 = row.try_get_by_index(6).unwrap_or(0);

            let total_capacity = daily_limit.unwrap_or(0.0);
            let used_capacity = daily_cost as f64 / 100.0;

            result.push(GroupCapacitySummary {
                group_id,
                group_name,
                platform,
                total_capacity,
                used_capacity,
                account_count: total_accounts,
            });
        }
        Ok(result)
    }

    /// 更新排序顺序
    pub async fn update_sort_order(&self, orders: &[SortOrderItem]) -> Result<()> {
        for item in orders {
            if let Some(group) = groups::Entity::find_by_id(item.id).one(&self.db).await? {
                let mut group: groups::ActiveModel = group.into();
                group.sort_order = Set(item.sort_order);
                group.updated_at = Set(Utc::now());
                group.update(&self.db).await?;
            }
        }
        Ok(())
    }

    /// 获取分组统计
    pub async fn get_group_stats(&self, group_id: i64) -> Result<Option<GroupStats>> {
        let group = self.get_group(group_id).await?;

        let g = match group {
            Some(g) => g,
            None => return Ok(None),
        };

        // Aggregate usage stats for all accounts in this group.
        // latency is not a dedicated column — we derive success_rate from the bool.
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    COUNT(*)                                          AS total_requests,
                    COALESCE(SUM(u.input_tokens + u.output_tokens), 0) AS total_tokens,
                    COALESCE(SUM(u.cost), 0)                          AS total_cost,
                    CASE WHEN COUNT(*) > 0
                         THEN COUNT(*) FILTER (WHERE u.success)::float / COUNT(*)::float * 100.0
                         ELSE 100.0 END                               AS success_rate
                FROM usages u
                INNER JOIN account_groups ag ON ag.account_id = u.account_id
                WHERE ag.group_id = $1
                "#,
                [group_id.into()],
            ))
            .await?;

        let (total_requests, total_tokens, total_cost, success_rate) = match row {
            Some(ref r) => {
                let reqs: i64 = r.try_get_by_index(0).unwrap_or(0);
                let toks: i64 = r.try_get_by_index(1).unwrap_or(0);
                let cost: i64 = r.try_get_by_index(2).unwrap_or(0);
                let sr: f64 = r.try_get_by_index(3).unwrap_or(100.0);
                (reqs, toks, cost as f64 / 100.0, sr)
            }
            None => (0, 0, 0.0, 100.0),
        };

        Ok(Some(GroupStats {
            group_id: g.id,
            group_name: g.name,
            total_requests,
            total_tokens,
            total_cost,
            average_latency_ms: 0.0, // no latency column in usages table
            success_rate,
        }))
    }

    /// 获取费率倍数
    pub async fn get_rate_multipliers(&self, group_id: i64) -> Result<Vec<RateMultiplierInfo>> {
        let group = groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

        // 返回默认费率倍数
        Ok(vec![RateMultiplierInfo {
            model: "*".to_string(),
            multiplier: group.rate_multiplier,
        }])
    }

    /// 获取分组 API Keys
    /// API keys belong to users, not groups directly. We find API keys that have
    /// been used with accounts in this group (via the usages table).
    pub async fn get_group_api_keys(&self, group_id: i64) -> Result<Vec<ApiKeyInfo>> {
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT DISTINCT
                    ak.id,
                    ak.key,
                    ak.name,
                    ak.status,
                    ak.created_at
                FROM api_keys ak
                INNER JOIN usages u ON u.api_key_id = ak.id
                INNER JOIN account_groups ag ON ag.account_id = u.account_id
                WHERE ag.group_id = $1
                ORDER BY ak.created_at DESC
                "#,
                [group_id.into()],
            ))
            .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: Uuid = row.try_get_by_index(0)?;
            let key: String = row.try_get_by_index(1)?;
            let name: Option<String> = row.try_get_by_index(2)?;
            let status: String = row.try_get_by_index(3)?;
            let created_at: chrono::DateTime<Utc> = row.try_get_by_index(4)?;

            // Mask the key: show first 7 and last 4 chars
            let key_masked = if key.len() >= 12 {
                format!("{}...{}", &key[..7], &key[key.len() - 4..])
            } else {
                key
            };

            result.push(ApiKeyInfo {
                id,
                key_masked,
                name,
                status,
                created_at,
            });
        }
        Ok(result)
    }

    /// 列出所有分组（简化版）
    pub async fn list_all_groups(&self) -> Result<Vec<SimpleGroupInfo>> {
        let groups = groups::Entity::find()
            .filter(groups::Column::DeletedAt.is_null())
            .order_by_asc(groups::Column::SortOrder)
            .all(&self.db)
            .await?;

        Ok(groups
            .into_iter()
            .map(|g| SimpleGroupInfo {
                id: g.id,
                name: g.name,
                platform: g.platform,
                status: g.status,
                sort_order: g.sort_order,
            })
            .collect())
    }
}

/// 分组配额状态
#[derive(Debug, Serialize)]
pub struct GroupQuotaStatus {
    pub group_id: i64,
    pub group_name: String,
    pub daily_limit: Option<f64>,
    pub daily_used: f64,
    pub weekly_limit: Option<f64>,
    pub monthly_limit: Option<f64>,
    pub monthly_used: f64,
    pub is_over_limit: bool,
}

/// 分组费率信息
#[derive(Debug, Serialize)]
pub struct GroupRateInfo {
    pub group_id: i64,
    pub group_name: String,
    pub platform: String,
    pub rate_multiplier: f64,
    pub models: Vec<String>,
}

/// 分组使用摘要
#[derive(Debug, Serialize)]
pub struct GroupUsageSummary {
    pub group_id: i64,
    pub group_name: String,
    pub platform: String,
    pub daily_used_usd: f64,
    pub daily_limit_usd: f64,
    pub monthly_used_usd: f64,
    pub monthly_limit_usd: f64,
    pub account_count: i64,
    pub active_account_count: i64,
}

/// 分组容量摘要
#[derive(Debug, Serialize)]
pub struct GroupCapacitySummary {
    pub group_id: i64,
    pub group_name: String,
    pub platform: String,
    pub total_capacity: f64,
    pub used_capacity: f64,
    pub account_count: i64,
}

/// 排序顺序项
#[derive(Debug, Deserialize)]
pub struct SortOrderItem {
    pub id: i64,
    pub sort_order: i32,
}

/// 分组统计
#[derive(Debug, Serialize)]
pub struct GroupStats {
    pub group_id: i64,
    pub group_name: String,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub average_latency_ms: f64,
    pub success_rate: f64,
}

/// 费率倍数信息
#[derive(Debug, Serialize)]
pub struct RateMultiplierInfo {
    pub model: String,
    pub multiplier: f64,
}

/// API Key 信息
#[derive(Debug, Serialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub key_masked: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
}

/// 简化分组信息
#[derive(Debug, Serialize)]
pub struct SimpleGroupInfo {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub status: String,
    pub sort_order: i32,
}
