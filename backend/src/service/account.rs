//! 上游账号服务 - 完整实现
//!
//! 功能特性：
//! - CRUD 操作
//! - 分页查询
//! - 调度信息支持
//! - 内存缓存
//! - 凭证加密

#![allow(dead_code)]
use anyhow::Result;
use chrono::Utc;
use lru::LruCache;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::entity::accounts;
use crate::utils::encryption_global::GlobalEncryption;

/// 基础账号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub credential_type: String,
    pub status: String,
    pub priority: i32,
    pub last_error: Option<String>,
    pub group_id: Option<i64>,
    pub created_at: chrono::DateTime<Utc>,
}

/// 带调度信息的账号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWithScheduling {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub credential_type: String,
    pub priority: i32,
    pub concurrent_limit: i32,
    pub rate_limit_rpm: Option<i32>,
    pub group_id: Option<i64>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

/// 账号并发信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConcurrency {
    pub id: Uuid,
    pub max_concurrency: i32,
}

/// Provider 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub provider: String,
    pub total: u64,
    pub active: u64,
    pub inactive: u64,
    pub error: u64,
}

/// 创建账号请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub provider: String,
    pub credential_type: String,
    pub credential: String,
    pub priority: Option<i32>,
    pub concurrent_limit: Option<i32>,
    pub rate_limit_rpm: Option<i32>,
    pub group_id: Option<i64>,
}

/// 缓存键
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum CacheKey {
    ActiveAccounts(String), // provider
    Account(Uuid),          // account_id
    AccountList(String),    // filter hash
}

/// 账号服务
#[derive(Clone)]
pub struct AccountService {
    db: DatabaseConnection,
    cache: Arc<RwLock<LruCache<CacheKey, Vec<AccountInfo>>>>,
    single_cache: Arc<RwLock<LruCache<Uuid, accounts::Model>>>,
}

impl AccountService {
    /// 创建新的账号服务实例
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()))),
            single_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
        }
    }

    /// 清除缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        let mut single_cache = self.single_cache.write().await;
        single_cache.clear();
    }

    /// 使指定账号的缓存失效
    pub async fn invalidate_cache(&self, account_id: Uuid) {
        let mut single_cache = self.single_cache.write().await;
        single_cache.pop(&account_id);
    }

    /// 添加账号（自动加密凭证）
    pub async fn add(
        &self,
        name: &str,
        provider: &str,
        credential_type: &str,
        credential: &str,
        priority: i32,
    ) -> Result<AccountInfo> {
        // 加密凭证
        let encrypted_credential = GlobalEncryption::encrypt(credential)
            .map_err(|e| anyhow::anyhow!("Failed to encrypt credential: {}", e))?;

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
            group_id: account.group_id,
            created_at: account.created_at,
        })
    }

    /// 创建账号（带调度配置）
    pub async fn create_with_scheduling(
        &self,
        req: CreateAccountRequest,
    ) -> Result<AccountWithScheduling> {
        // 加密凭证
        let encrypted_credential = GlobalEncryption::encrypt(&req.credential)
            .map_err(|e| anyhow::anyhow!("Failed to encrypt credential: {}", e))?;

        let now = Utc::now();
        let id = Uuid::new_v4();

        let account = accounts::ActiveModel {
            id: Set(id),
            name: Set(req.name),
            provider: Set(req.provider),
            credential_type: Set(req.credential_type),
            credential: Set(encrypted_credential),
            metadata: Set(None),
            status: Set("active".to_string()),
            last_error: Set(None),
            priority: Set(req.priority.unwrap_or(50)),
            concurrent_limit: Set(req.concurrent_limit.or(Some(5))),
            rate_limit_rpm: Set(req.rate_limit_rpm),
            group_id: Set(req.group_id),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let account = account.insert(&self.db).await?;

        Ok(AccountWithScheduling {
            id: account.id,
            name: account.name,
            provider: account.provider,
            status: account.status,
            credential_type: account.credential_type,
            priority: account.priority,
            concurrent_limit: account.concurrent_limit.unwrap_or(5),
            rate_limit_rpm: account.rate_limit_rpm,
            group_id: account.group_id,
            created_at: account.created_at,
            updated_at: account.updated_at,
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

    /// 按多个 provider 获取可用账号（按优先级降序）
    pub async fn get_available_for_providers(
        &self,
        providers: &[&str],
    ) -> Result<Vec<accounts::Model>> {
        if providers.is_empty() {
            return Ok(Vec::new());
        }

        let providers = providers
            .iter()
            .map(|provider| provider.to_string())
            .collect::<Vec<_>>();

        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Provider.is_in(providers))
            .filter(accounts::Column::Status.eq("active"))
            .order_by_desc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts)
    }

    /// 获取带调度信息的活跃账号列表
    pub async fn list_active_with_scheduling(&self) -> Result<Vec<AccountWithScheduling>> {
        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .order_by_asc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts
            .into_iter()
            .map(|a| AccountWithScheduling {
                id: a.id,
                name: a.name,
                provider: a.provider,
                status: a.status,
                credential_type: a.credential_type,
                priority: a.priority,
                concurrent_limit: a.concurrent_limit.unwrap_or(10),
                rate_limit_rpm: a.rate_limit_rpm,
                group_id: a.group_id,
                created_at: a.created_at,
                updated_at: a.updated_at,
            })
            .collect())
    }

    /// 获取可调度的账号（支持分组过滤）
    pub async fn list_schedulable(
        &self,
        group_id: Option<i64>,
    ) -> Result<Vec<AccountWithScheduling>> {
        let mut query = accounts::Entity::find().filter(accounts::Column::Status.eq("active"));

        if let Some(gid) = group_id {
            query = query.filter(accounts::Column::GroupId.eq(gid));
        }

        let accounts = query
            .order_by_asc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts
            .into_iter()
            .map(|a| AccountWithScheduling {
                id: a.id,
                name: a.name,
                provider: a.provider,
                status: a.status,
                credential_type: a.credential_type,
                priority: a.priority,
                concurrent_limit: a.concurrent_limit.unwrap_or(10),
                rate_limit_rpm: a.rate_limit_rpm,
                group_id: a.group_id,
                created_at: a.created_at,
                updated_at: a.updated_at,
            })
            .collect())
    }

    /// 获取支持指定模型的账号
    pub async fn get_for_model(&self, model: &str) -> Result<Vec<accounts::Model>> {
        let provider = Self::infer_provider(model);
        self.get_available(provider).await
    }

    /// 推断模型对应的 provider
    fn infer_provider(model: &str) -> &'static str {
        if model.starts_with("claude") {
            "anthropic"
        } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
            "openai"
        } else if model.starts_with("gemini") {
            "gemini"
        } else if model.starts_with("sora") {
            "sora"
        } else {
            "openai"
        }
    }

    /// 批量获取账号并发限制
    pub async fn get_concurrency_batch(
        &self,
        account_ids: &[Uuid],
    ) -> Result<Vec<AccountConcurrency>> {
        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Id.is_in(account_ids.to_vec()))
            .all(&self.db)
            .await?;

        Ok(accounts
            .into_iter()
            .map(|a| AccountConcurrency {
                id: a.id,
                max_concurrency: a.concurrent_limit.unwrap_or(10),
            })
            .collect())
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

        self.invalidate_cache(account_id).await;

        Ok(())
    }

    /// 更新调度配置
    pub async fn update_scheduling_config(
        &self,
        account_id: Uuid,
        priority: i32,
        concurrent_limit: i32,
        rate_limit_rpm: Option<i32>,
    ) -> Result<()> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        let mut account: accounts::ActiveModel = account.into();
        account.priority = Set(priority);
        account.concurrent_limit = Set(Some(concurrent_limit));
        account.rate_limit_rpm = Set(rate_limit_rpm);
        account.updated_at = Set(Utc::now());
        account.update(&self.db).await?;

        self.invalidate_cache(account_id).await;

        Ok(())
    }

    /// 列出所有账号（保留兼容性，但建议使用 list_paged）
    #[deprecated(note = "Use list_paged() instead for better performance with large datasets")]
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
                group_id: a.group_id,
                created_at: a.created_at,
            })
            .collect())
    }

    /// 分页查询账号
    pub async fn list_paged(
        &self,
        page: u64,
        per_page: u64,
        status: Option<&str>,
        provider: Option<&str>,
        search: Option<&str>,
        group_id: Option<i64>,
    ) -> Result<(Vec<AccountInfo>, u64)> {
        let per_page = per_page.clamp(1, 200);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = accounts::Entity::find();

        if let Some(s) = status {
            query = query.filter(accounts::Column::Status.eq(s));
        }
        if let Some(p) = provider {
            query = query.filter(accounts::Column::Provider.eq(p));
        }
        if let Some(s) = search {
            query = query.filter(accounts::Column::Name.contains(s));
        }
        if let Some(gid) = group_id {
            query = query.filter(accounts::Column::GroupId.eq(gid));
        }

        let total = query.clone().count(&self.db).await?;

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
                group_id: a.group_id,
                created_at: a.created_at,
            })
            .collect();

        Ok((items, total))
    }

    /// 获取活跃账号数量
    pub async fn count_active(&self) -> Result<u64> {
        let count = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .count(&self.db)
            .await?;
        Ok(count)
    }

    /// 获取按 Provider 分组的账号统计
    pub async fn stats_by_provider(&self) -> Result<HashMap<String, ProviderStats>> {
        let accounts = accounts::Entity::find().all(&self.db).await?;

        let mut stats: HashMap<String, ProviderStats> = HashMap::new();

        for account in accounts {
            let entry = stats
                .entry(account.provider.clone())
                .or_insert(ProviderStats {
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
        self.invalidate_cache(account_id).await;

        Ok(())
    }

    /// 获取账号详情（包含加密的 credential）
    pub async fn get_with_credential(&self, account_id: Uuid) -> Result<Option<accounts::Model>> {
        // 检查缓存
        {
            let mut cache = self.single_cache.write().await;
            if let Some(cached) = cache.get(&account_id) {
                return Ok(Some(cached.clone()));
            }
        }

        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?;

        // 更新缓存
        if let Some(ref acc) = account {
            let mut cache = self.single_cache.write().await;
            cache.put(account_id, acc.clone());
        }

        Ok(account)
    }

    /// 检查账号是否支持指定模型
    pub async fn supports_model(&self, account_id: Uuid, model: &str) -> Result<bool> {
        let account = self
            .get_with_credential(account_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        let provider = Self::infer_provider(model);
        Ok(account.provider == provider)
    }

    /// 按模型过滤账号
    pub fn filter_by_model(
        accounts: &[AccountWithScheduling],
        model: &str,
    ) -> Vec<AccountWithScheduling> {
        let provider = Self::infer_provider(model);
        accounts
            .iter()
            .filter(|a| a.provider == provider)
            .cloned()
            .collect()
    }

    /// 刷新账号 Token
    pub async fn refresh_token(&self, _account_id: Uuid) -> Result<bool> {
        // TODO: 实现具体的 Token 刷新逻辑
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

        self.invalidate_cache(account_id).await;

        Ok(true)
    }

    /// 刷新账号 Tier
    pub async fn refresh_tier(&self, _account_id: Uuid) -> Result<String> {
        // TODO: 实现具体的 Tier 刷新逻辑
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

        self.invalidate_cache(account_id).await;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_provider() {
        assert_eq!(AccountService::infer_provider("claude-3-opus"), "anthropic");
        assert_eq!(AccountService::infer_provider("gpt-4"), "openai");
        assert_eq!(AccountService::infer_provider("gemini-pro"), "gemini");
        assert_eq!(AccountService::infer_provider("o1-preview"), "openai");
    }

    #[test]
    fn test_account_info_serialization() {
        let info = AccountInfo {
            id: Uuid::new_v4(),
            name: "test-account".to_string(),
            provider: "anthropic".to_string(),
            credential_type: "api_key".to_string(),
            status: "active".to_string(),
            priority: 50,
            last_error: None,
            group_id: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-account"));
    }
}
