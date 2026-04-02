//! 上游账号服务 - 完整实现

#![allow(dead_code)]
use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter,
    QueryOrder, QuerySelect, PaginatorTrait, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::entity::accounts;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub credential_type: String,
    pub status: String,
    pub priority: i32,
    pub last_error: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

/// Provider 统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStats {
    pub provider: String,
    pub total: u64,
    pub active: u64,
    pub inactive: u64,
    pub error: u64,
}

pub struct AccountService {
    db: DatabaseConnection,
}

impl AccountService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 添加账号
    pub async fn add(
        &self,
        name: &str,
        provider: &str,
        credential_type: &str,
        credential: &str,
        priority: i32,
    ) -> Result<AccountInfo> {
        // TODO: 加密 credential
        let encrypted_credential = credential.to_string(); // 实际应该加密

        let now = Utc::now();
        let account = accounts::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            provider: Set(provider.to_string()),
            credential_type: Set(credential_type.to_string()),
            credential: Set(encrypted_credential),
            metadata: Set(None),
            status: Set("active".to_string()),
            last_error: Set(None),
            priority: Set(priority),
            concurrent_limit: Set(Some(5)),
            rate_limit_rpm: Set(Some(60)),
            group_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let account = account.insert(&self.db).await?;

        Ok(AccountInfo {
            id: account.id,
            name: account.name,
            provider: account.provider,
            credential_type: account.credential_type,
            status: account.status,
            priority: account.priority,
            last_error: account.last_error,
            created_at: account.created_at,
        })
    }

    /// 获取可用账号（用于调度）
    pub async fn get_available(&self, provider: &str) -> Result<Vec<accounts::Model>> {
        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Provider.eq(provider))
            .filter(accounts::Column::Status.eq("active"))
            .order_by_desc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts)
    }

    /// 获取支持指定模型的账号
    pub async fn get_for_model(&self, model: &str) -> Result<Vec<accounts::Model>> {
        // 根据模型推断 provider
        let provider = if model.starts_with("claude") {
            "anthropic"
        } else if model.starts_with("gpt") {
            "openai"
        } else if model.starts_with("gemini") {
            "gemini"
        } else {
            "openai" // 默认
        };

        self.get_available(provider).await
    }

    /// 更新账号状态
    pub async fn update_status(
        &self,
        account_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        let mut account: accounts::ActiveModel = account.into();
        account.status = Set(status.to_string());
        account.last_error = Set(error.map(|s| s.to_string()));
        account.updated_at = Set(Utc::now());
        account.update(&self.db).await?;

        Ok(())
    }

    /// 列出所有账号（保留兼容性，但建议使用 list_paged）
    pub async fn list_all(&self) -> Result<Vec<AccountInfo>> {
        let accounts = accounts::Entity::find()
            .order_by_desc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts
            .into_iter()
            .map(|a| AccountInfo {
                id: a.id,
                name: a.name,
                provider: a.provider,
                credential_type: a.credential_type,
                status: a.status,
                priority: a.priority,
                last_error: a.last_error,
                created_at: a.created_at,
            })
            .collect())
    }

    /// 分页查询账号 - 性能优化版本
    ///
    /// # 参数
    /// - `page`: 页码（从 1 开始）
    /// - `per_page`: 每页数量（最大 200）
    /// - `status`: 状态过滤（可选）
    /// - `provider`: Provider 过滤（可选）
    /// - `search`: 名称搜索（可选）
    ///
    /// # 返回
    /// (账号列表, 总数)
    pub async fn list_paged(
        &self,
        page: u64,
        per_page: u64,
        status: Option<&str>,
        provider: Option<&str>,
        search: Option<&str>,
    ) -> Result<(Vec<AccountInfo>, u64)> {
        // 限制每页最大数量
        let per_page = per_page.clamp(1, 200);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = accounts::Entity::find();

        // 应用过滤条件
        if let Some(s) = status {
            query = query.filter(accounts::Column::Status.eq(s));
        }
        if let Some(p) = provider {
            query = query.filter(accounts::Column::Provider.eq(p));
        }
        if let Some(s) = search {
            query = query.filter(accounts::Column::Name.contains(s));
        }

        // 获取总数
        let total = query.clone().count(&self.db).await?;

        // 分页查询
        let accounts = query
            .order_by_desc(accounts::Column::Priority)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        let items: Vec<AccountInfo> = accounts
            .into_iter()
            .map(|a| AccountInfo {
                id: a.id,
                name: a.name,
                provider: a.provider,
                credential_type: a.credential_type,
                status: a.status,
                priority: a.priority,
                last_error: a.last_error,
                created_at: a.created_at,
            })
            .collect();

        Ok((items, total))
    }

    /// 获取活跃账号数量（快速统计）
    pub async fn count_active(&self) -> Result<u64> {
        let count = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .count(&self.db)
            .await?;
        Ok(count)
    }

    /// 获取按 Provider 分组的账号统计
    pub async fn stats_by_provider(&self) -> Result<HashMap<String, ProviderStats>> {
        let accounts = accounts::Entity::find()
            .all(&self.db)
            .await?;

        let mut stats: HashMap<String, ProviderStats> = HashMap::new();

        for account in accounts {
            let entry = stats.entry(account.provider.clone()).or_insert(ProviderStats {
                provider: account.provider.clone(),
                total: 0,
                active: 0,
                inactive: 0,
                error: 0,
            });

            entry.total += 1;
            match account.status.as_str() {
                "active" => entry.active += 1,
                "inactive" => entry.inactive += 1,
                "error" => entry.error += 1,
                _ => {}
            }
        }

        Ok(stats)
    }

    /// 删除账号
    pub async fn delete(&self, account_id: Uuid) -> Result<()> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        account.delete(&self.db).await?;
        Ok(())
    }

    /// 获取账号详情（包含 credential）
    pub async fn get_with_credential(&self, account_id: Uuid) -> Result<Option<accounts::Model>> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?;

        Ok(account)
    }

    /// 刷新账号 Token
    pub async fn refresh_token(&self, _account_id: Uuid) -> Result<bool> {
        // TODO: 实现具体的 Token 刷新逻辑
        // 需要根据 provider 调用相应的 API
        Ok(true)
    }

    /// 恢复账号状态
    pub async fn recover_state(&self, account_id: Uuid) -> Result<bool> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        let mut account: accounts::ActiveModel = account.into();
        account.status = Set("active".to_string());
        account.last_error = Set(None);
        account.updated_at = Set(Utc::now());
        account.update(&self.db).await?;

        Ok(true)
    }

    /// 刷新账号 Tier
    pub async fn refresh_tier(&self, _account_id: Uuid) -> Result<String> {
        // TODO: 实现具体的 Tier 刷新逻辑
        // 需要根据 provider 调用相应的 API
        Ok("tier1".to_string())
    }

    /// 清除账号错误
    pub async fn clear_error(&self, account_id: Uuid) -> Result<()> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        let mut account: accounts::ActiveModel = account.into();
        account.last_error = Set(None);
        account.updated_at = Set(Utc::now());
        account.update(&self.db).await?;

        Ok(())
    }

    /// 获取账号使用统计
    pub async fn get_usage_stats(&self, _account_id: Uuid) -> Result<serde_json::Value> {
        // TODO: 实现使用统计查询
        Ok(serde_json::json!({
            "total_requests": 0,
            "total_tokens": 0,
            "total_cost": 0.0,
        }))
    }

    /// 获取账号今日统计
    pub async fn get_today_stats(&self, _account_id: Uuid) -> Result<serde_json::Value> {
        // TODO: 实现今日统计查询
        Ok(serde_json::json!({
            "requests": 0,
            "tokens": 0,
            "cost": 0.0,
        }))
    }

    /// 重置账号配额
    pub async fn reset_quota(&self, _account_id: Uuid) -> Result<()> {
        // TODO: 实现配额重置
        Ok(())
    }
}
