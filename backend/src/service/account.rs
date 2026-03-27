//! 上游账号服务 - 完整实现

use anyhow::Result;
use sea_orm::{
    EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, 
    DatabaseConnection, ActiveValue, QuerySelect, PaginatorTrait,
};
use uuid::Uuid;
use chrono::Utc;
use serde_json::json;

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
        error: Option<&str>
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

    /// 列出所有账号
    pub async fn list_all(&self) -> Result<Vec<AccountInfo>> {
        let accounts = accounts::Entity::find()
            .order_by_desc(accounts::Column::Priority)
            .all(&self.db)
            .await?;

        Ok(accounts.into_iter().map(|a| AccountInfo {
            id: a.id,
            name: a.name,
            provider: a.provider,
            credential_type: a.credential_type,
            status: a.status,
            priority: a.priority,
            last_error: a.last_error,
            created_at: a.created_at,
        }).collect())
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
}
