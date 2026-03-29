//! 分组服务 - 账号分组管理
//!
//! 提供分组创建、管理、模型路由和配额分发功能

#![allow(dead_code)]
use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
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

        // TODO: 实现实际的使用量统计
        let daily_used = 0.0;
        let monthly_used = 0.0;

        Ok(GroupQuotaStatus {
            group_id: group.id,
            group_name: group.name,
            daily_limit: group.daily_limit_usd,
            daily_used,
            weekly_limit: group.weekly_limit_usd,
            monthly_limit: group.monthly_limit_usd,
            monthly_used,
            is_over_limit: false,
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
            .map(|g| GroupRateInfo {
                group_id: g.id,
                group_name: g.name,
                platform: g.platform,
                rate_multiplier: g.rate_multiplier,
                models: vec![], // TODO: 从 model_routing 解析
            })
            .collect())
    }

    // ========================================================================
    // 管理端扩展方法
    // ========================================================================

    /// 获取使用摘要
    pub async fn get_usage_summary(&self) -> Result<Vec<GroupUsageSummary>> {
        let groups = self.list_groups(None).await?;

        Ok(groups
            .into_iter()
            .map(|g| GroupUsageSummary {
                group_id: g.id,
                group_name: g.name,
                platform: g.platform,
                daily_used_usd: 0.0, // TODO: 实现实际统计
                daily_limit_usd: g.daily_limit_usd.unwrap_or(0.0),
                monthly_used_usd: 0.0,
                monthly_limit_usd: g.monthly_limit_usd.unwrap_or(0.0),
                account_count: g.account_count,
                active_account_count: g.account_count,
            })
            .collect())
    }

    /// 获取容量摘要
    pub async fn get_capacity_summary(&self) -> Result<Vec<GroupCapacitySummary>> {
        let groups = self.list_groups(None).await?;

        Ok(groups
            .into_iter()
            .map(|g| GroupCapacitySummary {
                group_id: g.id,
                group_name: g.name,
                platform: g.platform,
                total_capacity: g.daily_limit_usd.unwrap_or(0.0),
                used_capacity: 0.0,
                account_count: g.account_count,
            })
            .collect())
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

        if let Some(g) = group {
            Ok(Some(GroupStats {
                group_id: g.id,
                group_name: g.name,
                total_requests: 0,
                total_tokens: 0,
                total_cost: 0.0,
                average_latency_ms: 0.0,
                success_rate: 100.0,
            }))
        } else {
            Ok(None)
        }
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
    pub async fn get_group_api_keys(&self, group_id: i64) -> Result<Vec<ApiKeyInfo>> {
        // TODO: 实现根据分组查询 API Keys
        let _ = group_id;
        Ok(vec![])
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
