//! 加密凭证服务
//!
//! 提供对敏感凭证（API Keys, OAuth Tokens, TOTP Secrets）的加密管理

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
use uuid::Uuid;

use crate::entity::{
    accounts, oauth_tokens,
    oauth_tokens::{CreateOAuthToken, OAuthProviderType},
    users,
};
use crate::utils::{encryption_service, get_encryption_service, EncryptionService};

/// 加密凭证服务
pub struct CredentialService {
    db: DatabaseConnection,
}

impl CredentialService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 获取加密服务（如果可用）
    fn get_encryption() -> Option<Arc<EncryptionService>> {
        get_encryption_service()
    }

    /// 加密敏感数据
    fn encrypt(data: &str) -> Result<String> {
        let enc = get_encryption_service().context("Encryption service not initialized")?;
        enc.encrypt(data).context("Failed to encrypt data")
    }

    /// 解密敏感数据
    fn decrypt(data: &str) -> Result<String> {
        let enc = get_encryption_service().context("Encryption service not initialized")?;
        enc.decrypt(data).context("Failed to decrypt data")
    }

    /// 创建或更新 Account 的凭证（加密存储）
    ///
    /// # Arguments
    /// * `account_id` - Account ID
    /// * `credential` - 明文凭证（如 API Key）
    pub async fn set_account_credential(&self, account_id: Uuid, credential: &str) -> Result<()> {
        let encrypted = Self::encrypt(credential)?;

        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .context("Account not found")?;

        let mut active_model: accounts::ActiveModel = account.into();
        active_model.credential = Set(encrypted);
        active_model.updated_at = Set(Utc::now());
        active_model.update(&self.db).await?;

        tracing::info!(
            account_id = %account_id,
            "Account credential updated (encrypted)"
        );

        Ok(())
    }

    /// 获取 Account 的凭证（解密）
    ///
    /// # Arguments
    /// * `account_id` - Account ID
    ///
    /// # Returns
    /// 解密后的明文凭证
    pub async fn get_account_credential(&self, account_id: Uuid) -> Result<String> {
        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .context("Account not found")?;

        Self::decrypt(&account.credential)
    }

    /// 获取所有活跃 Account 的凭证（解密）
    ///
    /// # Returns
    /// Vec of (account_id, provider, credential)
    pub async fn get_active_account_credentials(&self) -> Result<Vec<(Uuid, String, String)>> {
        let accounts = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .all(&self.db)
            .await?;

        let mut result = Vec::new();
        for account in accounts {
            match Self::decrypt(&account.credential) {
                Ok(credential) => {
                    result.push((account.id, account.provider, credential));
                }
                Err(e) => {
                    tracing::warn!(
                        account_id = %account.id,
                        error = %e,
                        "Failed to decrypt credential for account"
                    );
                }
            }
        }

        Ok(result)
    }

    /// 创建 OAuth Token（加密存储）
    ///
    /// # Arguments
    /// * `create` - 创建请求
    pub async fn create_oauth_token(
        &self,
        create: CreateOAuthToken,
    ) -> Result<oauth_tokens::Model> {
        // 加密 access_token
        let encrypted_access_token = Self::encrypt(&create.access_token)?;

        // 加密 refresh_token（如果有）
        let encrypted_refresh_token = create
            .refresh_token
            .as_ref()
            .map(|t| Self::encrypt(t))
            .transpose()?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        
        // 先计算 expires_at，避免部分移动问题
        let expires_at = create.calculate_expires_at();

        let oauth_token = oauth_tokens::ActiveModel {
            id: Set(id),
            account_id: Set(create.account_id),
            provider: Set(create.provider),
            access_token: Set(encrypted_access_token),
            refresh_token: Set(encrypted_refresh_token),
            expires_at: Set(expires_at),
            token_type: Set(create.token_type),
            scope: Set(create.scope),
            metadata: Set(create.metadata),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = oauth_token.insert(&self.db).await?;

        tracing::info!(
            account_id = %create.account_id,
            provider = %model.provider,
            "OAuth token created (encrypted)"
        );

        Ok(model)
    }

    /// 更新 OAuth Token（加密存储）
    ///
    /// # Arguments
    /// * `token_id` - Token ID
    /// * `access_token` - 新的 access_token（明文）
    /// * `refresh_token` - 新的 refresh_token（明文，可选）
    /// * `expires_in` - 过期时间（秒）
    pub async fn update_oauth_token(
        &self,
        token_id: Uuid,
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
    ) -> Result<oauth_tokens::Model> {
        let token = oauth_tokens::Entity::find_by_id(token_id)
            .one(&self.db)
            .await?
            .context("OAuth token not found")?;

        let encrypted_access_token = Self::encrypt(&access_token)?;
        let encrypted_refresh_token = refresh_token
            .as_ref()
            .map(|t| Self::encrypt(t))
            .transpose()?;

        let expires_at = expires_in.map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));

        let mut active_model: oauth_tokens::ActiveModel = token.into();
        active_model.access_token = Set(encrypted_access_token);
        active_model.refresh_token = Set(encrypted_refresh_token);
        if let Some(expires_at) = expires_at {
            active_model.expires_at = Set(Some(expires_at));
        }
        active_model.updated_at = Set(Utc::now());

        let model = active_model.update(&self.db).await?;

        tracing::info!(
            token_id = %token_id,
            "OAuth token updated (encrypted)"
        );

        Ok(model)
    }

    /// 获取 OAuth Token（解密）
    ///
    /// # Arguments
    /// * `account_id` - Account ID
    /// * `provider` - OAuth 提供商
    ///
    /// # Returns
    /// (access_token, refresh_token, expires_at)
    pub async fn get_oauth_token(
        &self,
        account_id: Uuid,
        provider: &str,
    ) -> Result<Option<(String, Option<String>, Option<DateTime<Utc>>)>> {
        let token = oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::AccountId.eq(account_id))
            .filter(oauth_tokens::Column::Provider.eq(provider))
            .one(&self.db)
            .await?;

        match token {
            Some(t) => {
                let access_token = Self::decrypt(&t.access_token)?;
                let refresh_token = t
                    .refresh_token
                    .as_ref()
                    .map(|rt| Self::decrypt(rt))
                    .transpose()?;

                Ok(Some((access_token, refresh_token, t.expires_at)))
            }
            None => Ok(None),
        }
    }

    /// 设置用户的 TOTP Secret（加密存储）
    ///
    /// # Arguments
    /// * `user_id` - User ID
    /// * `totp_secret` - TOTP Secret（明文）
    pub async fn set_user_totp_secret(&self, user_id: Uuid, totp_secret: &str) -> Result<()> {
        let encrypted = Self::encrypt(totp_secret)?;

        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .context("User not found")?;

        let mut active_model: users::ActiveModel = user.into();
        active_model.totp_secret = Set(Some(encrypted));
        active_model.updated_at = Set(Utc::now());
        active_model.update(&self.db).await?;

        tracing::info!(
            user_id = %user_id,
            "User TOTP secret updated (encrypted)"
        );

        Ok(())
    }

    /// 获取用户的 TOTP Secret（解密）
    ///
    /// # Arguments
    /// * `user_id` - User ID
    ///
    /// # Returns
    /// 解密后的 TOTP Secret
    pub async fn get_user_totp_secret(&self, user_id: Uuid) -> Result<Option<String>> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .context("User not found")?;

        match user.totp_secret {
            Some(encrypted) => {
                let secret = Self::decrypt(&encrypted)?;
                Ok(Some(secret))
            }
            None => Ok(None),
        }
    }

    /// 删除用户的 TOTP Secret
    pub async fn delete_user_totp_secret(&self, user_id: Uuid) -> Result<()> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .context("User not found")?;

        let mut active_model: users::ActiveModel = user.into();
        active_model.totp_secret = Set(None);
        active_model.totp_enabled = Set(false);
        active_model.updated_at = Set(Utc::now());
        active_model.update(&self.db).await?;

        tracing::info!(
            user_id = %user_id,
            "User TOTP secret deleted"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 这些测试需要数据库连接和加密服务初始化
    // 在实际项目中应该使用 mock 或测试数据库

    #[test]
    fn test_encryption_service_available() {
        // 测试加密服务是否可以获取
        let result = get_encryption_service();
        // 在测试环境中可能未初始化，所以这里只是检查不会 panic
        println!("Encryption service available: {}", result.is_some());
    }
}
