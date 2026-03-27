//! API Key 服务 - 完整实现

use anyhow::Result;
use sea_orm::{
    EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, 
    DatabaseConnection, ActiveValue, QuerySelect, PaginatorTrait,
};
use uuid::Uuid;
use rand::Rng;
use chrono::Utc;

use crate::entity::{api_keys, users};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub key_masked: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
    pub last_used_at: Option<chrono::DateTime<Utc>>,
}

pub struct ApiKeyService {
    db: DatabaseConnection,
    key_prefix: String,
}

impl ApiKeyService {
    pub fn new(db: DatabaseConnection, key_prefix: String) -> Self {
        Self { db, key_prefix }
    }

    /// 生成新的 API Key
    pub fn generate_key(&self) -> String {
        let mut rng = rand::thread_rng();
        let random_part: String = (0..48)
            .map(|_| {
                let idx = rng.gen_range(0..62);
                let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                chars.chars().nth(idx).unwrap()
            })
            .collect();
        
        format!("{}{}", self.key_prefix, random_part)
    }

    /// 为用户创建 API Key
    pub async fn create(&self, user_id: Uuid, name: Option<String>) -> Result<ApiKeyInfo> {
        // 验证用户存在
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let key = self.generate_key();
        let now = Utc::now();
        
        let api_key = api_keys::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            key: Set(key.clone()),
            name: Set(name),
            prefix: Set(self.key_prefix.clone()),
            status: Set("active".to_string()),
            concurrent_limit: Set(Some(5)),
            rate_limit_rpm: Set(Some(60)),
            allowed_models: Set(None),
            expires_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now),
        };

        let api_key = api_key.insert(&self.db).await?;

        Ok(ApiKeyInfo {
            id: api_key.id,
            user_id: api_key.user_id,
            key_masked: mask_key(&key),
            name: api_key.name,
            status: api_key.status,
            created_at: api_key.created_at,
            last_used_at: api_key.last_used_at,
        })
    }

    /// 验证 API Key
    pub async fn validate(&self, key: &str) -> Result<Option<api_keys::Model>> {
        let api_key = api_keys::Entity::find()
            .filter(api_keys::Column::Key.eq(key))
            .one(&self.db)
            .await?;

        if let Some(ref k) = api_key {
            if !k.is_active() {
                return Ok(None);
            }

            // 更新最后使用时间
            let mut update: api_keys::ActiveModel = k.clone().into();
            update.last_used_at = Set(Some(Utc::now()));
            update.update(&self.db).await?;
        }

        Ok(api_key)
    }

    /// 吊销 API Key
    pub async fn revoke(&self, user_id: Uuid, key_id: Uuid) -> Result<()> {
        let api_key = api_keys::Entity::find_by_id(key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("API Key not found"))?;

        if api_key.user_id != user_id {
            bail::anyhow!("Unauthorized");
        }

        let mut api_key: api_keys::ActiveModel = api_key.into();
        api_key.status = Set("revoked".to_string());
        api_key.update(&self.db).await?;

        Ok(())
    }

    /// 列出用户的所有 API Keys
    pub async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ApiKeyInfo>> {
        let keys = api_keys::Entity::find()
            .filter(api_keys::Column::UserId.eq(user_id))
            .order_by_desc(api_keys::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(keys.into_iter().map(|k| ApiKeyInfo {
            id: k.id,
            user_id: k.user_id,
            key_masked: k.mask_key(),
            name: k.name,
            status: k.status,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
        }).collect())
    }

    /// 删除 API Key
    pub async fn delete(&self, user_id: Uuid, key_id: Uuid) -> Result<()> {
        let api_key = api_keys::Entity::find_by_id(key_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("API Key not found"))?;

        if api_key.user_id != user_id {
            bail::anyhow!("Unauthorized");
        }

        api_key.delete(&self.db).await?;
        Ok(())
    }
}

fn mask_key(key: &str) -> String {
    if key.len() < 12 {
        return key.to_string();
    }
    format!("{}...{}", &key[..7], &key[key.len()-4..])
}
