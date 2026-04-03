//! 账号凭证持久化 - Account Credentials Persistence
//!
//! 安全地存储和管理账号凭证

#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entity::account_credentials;

/// 凭证类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CredentialType {
    ApiKey,
    OAuthToken,
    SessionKey,
    Password,
    Certificate,
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialType::ApiKey => write!(f, "api_key"),
            CredentialType::OAuthToken => write!(f, "oauth_token"),
            CredentialType::SessionKey => write!(f, "session_key"),
            CredentialType::Password => write!(f, "password"),
            CredentialType::Certificate => write!(f, "certificate"),
        }
    }
}

impl std::str::FromStr for CredentialType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "api_key" => Ok(CredentialType::ApiKey),
            "oauth_token" => Ok(CredentialType::OAuthToken),
            "session_key" => Ok(CredentialType::SessionKey),
            "password" => Ok(CredentialType::Password),
            "certificate" => Ok(CredentialType::Certificate),
            _ => bail!("Invalid credential type: {}", s),
        }
    }
}

/// 凭证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCredential {
    pub id: i64,
    pub account_id: Uuid,
    pub credential_type: CredentialType,
    pub encrypted_value: String,
    pub metadata: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 凭证持久化服务
pub struct AccountCredentialsPersistence {
    db: DatabaseConnection,
    encryption_key: [u8; 32], // AES-256 需要 32 字节密钥
}

impl AccountCredentialsPersistence {
    /// 创建新的凭证持久化服务
    pub fn new(db: DatabaseConnection, encryption_key: String) -> Self {
        // 从字符串派生 32 字节密钥
        let key = Self::derive_key(&encryption_key);
        Self {
            db,
            encryption_key: key,
        }
    }

    /// 从字符串派生 32 字节密钥
    fn derive_key(key_str: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(key_str.as_bytes());
        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);
        key
    }

    /// 存储凭证
    pub async fn store(
        &self,
        account_id: Uuid,
        credential_type: CredentialType,
        value: &str,
        metadata: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<i64> {
        // 加密凭证
        let encrypted = self.encrypt(value)?;

        // 检查是否已存在
        let existing = self
            .find_by_account_and_type(account_id, &credential_type)
            .await?;

        if let Some(existing) = existing {
            // 更新现有记录
            let mut model: account_credentials::ActiveModel = existing.into();
            model.encrypted_value = Set(encrypted);
            model.metadata = Set(metadata.map(|s| s.to_string()));
            model.expires_at = Set(expires_at);
            model.updated_at = Set(Utc::now());
            let updated = model.update(&self.db).await?;
            return Ok(updated.id);
        }

        // 创建新记录
        let now = Utc::now();
        let credential = account_credentials::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            account_id: Set(account_id),
            credential_type: Set(credential_type.to_string()),
            encrypted_value: Set(encrypted),
            metadata: Set(metadata.map(|s| s.to_string())),
            expires_at: Set(expires_at),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let inserted = credential.insert(&self.db).await?;

        tracing::info!(
            account_id = %account_id,
            credential_type = %credential_type,
            "Stored credential"
        );

        Ok(inserted.id)
    }

    /// 获取凭证
    pub async fn get(
        &self,
        account_id: Uuid,
        credential_type: &CredentialType,
    ) -> Result<Option<AccountCredential>> {
        let cred = self
            .find_by_account_and_type(account_id, credential_type)
            .await?;
        Ok(cred.map(|c| self.db_model_to_domain(c)))
    }

    /// 获取解密后的凭证值
    pub async fn get_decrypted(
        &self,
        account_id: Uuid,
        credential_type: &CredentialType,
    ) -> Result<Option<String>> {
        let credential = self.get(account_id, credential_type).await?;

        if let Some(cred) = credential {
            // 检查是否过期
            if let Some(expires_at) = cred.expires_at {
                if expires_at < Utc::now() {
                    return Ok(None);
                }
            }

            let decrypted = self.decrypt(&cred.encrypted_value)?;
            return Ok(Some(decrypted));
        }

        Ok(None)
    }

    /// 更新凭证
    pub async fn update(
        &self,
        account_id: Uuid,
        credential_type: &CredentialType,
        new_value: &str,
    ) -> Result<()> {
        let existing = self
            .find_by_account_and_type(account_id, credential_type)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Credential not found"))?;

        let encrypted = self.encrypt(new_value)?;

        let mut model: account_credentials::ActiveModel = existing.into();
        model.encrypted_value = Set(encrypted);
        model.updated_at = Set(Utc::now());
        model.update(&self.db).await?;

        tracing::info!(
            account_id = %account_id,
            credential_type = %credential_type,
            "Updated credential"
        );

        Ok(())
    }

    /// 删除凭证
    pub async fn delete(&self, account_id: Uuid, credential_type: &CredentialType) -> Result<bool> {
        let existing = self
            .find_by_account_and_type(account_id, credential_type)
            .await?;

        if let Some(cred) = existing {
            cred.delete(&self.db).await?;

            tracing::info!(
                account_id = %account_id,
                credential_type = %credential_type,
                "Deleted credential"
            );

            return Ok(true);
        }

        Ok(false)
    }

    /// 列出账号的所有凭证
    pub async fn list(&self, account_id: Uuid) -> Result<Vec<AccountCredential>> {
        let creds = account_credentials::Entity::find()
            .filter(account_credentials::Column::AccountId.eq(account_id))
            .all(&self.db)
            .await?;

        Ok(creds
            .into_iter()
            .map(|c| self.db_model_to_domain(c))
            .collect())
    }

    /// 检查凭证是否存在
    pub async fn exists(&self, account_id: Uuid, credential_type: &CredentialType) -> Result<bool> {
        let credential = self.get(account_id, credential_type).await?;
        Ok(credential.is_some())
    }

    /// 加密
    fn encrypt(&self, value: &str) -> Result<String> {
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.encryption_key);
        let cipher = Aes256Gcm::new(key);

        // 生成随机 nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // 加密
        let ciphertext = cipher
            .encrypt(&nonce, value.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // 组合 nonce + ciphertext 并 base64 编码
        let mut combined = nonce.to_vec();
        combined.extend(ciphertext);

        Ok(BASE64.encode(&combined))
    }

    /// 解密
    fn decrypt(&self, encrypted: &str) -> Result<String> {
        // Base64 解码
        let combined = BASE64.decode(encrypted)?;

        if combined.len() < 12 {
            bail!("Invalid encrypted data: too short");
        }

        // 分离 nonce 和 ciphertext
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // 解密
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.encryption_key);
        let cipher = Aes256Gcm::new(key);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))
    }

    /// 轮换加密密钥
    pub async fn rotate_encryption_key(&self, new_key: String) -> Result<i64> {
        tracing::info!("Starting encryption key rotation");

        let new_key_arr = Self::derive_key(&new_key);
        let new_key_ref = aes_gcm::Key::<Aes256Gcm>::from_slice(&new_key_arr);
        let new_cipher = Aes256Gcm::new(new_key_ref);

        // 获取所有凭证
        let all_creds = account_credentials::Entity::find().all(&self.db).await?;

        let mut count = 0i64;

        for cred in all_creds {
            // 用旧密钥解密
            match self.decrypt(&cred.encrypted_value) {
                Ok(plaintext) => {
                    // 用新密钥加密
                    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
                    let new_ciphertext = new_cipher
                        .encrypt(&nonce, plaintext.as_bytes())
                        .map_err(|e| anyhow::anyhow!("Re-encryption failed: {}", e))?;

                    let mut combined = nonce.to_vec();
                    combined.extend(new_ciphertext);
                    let new_encrypted = BASE64.encode(&combined);

                    // 更新数据库
                    let mut model: account_credentials::ActiveModel = cred.into();
                    model.encrypted_value = Set(new_encrypted);
                    model.updated_at = Set(Utc::now());
                    model.update(&self.db).await?;

                    count += 1;
                }
                Err(e) => {
                    tracing::error!(
                        credential_id = cred.id,
                        error = %e,
                        "Failed to decrypt credential during key rotation"
                    );
                }
            }
        }

        tracing::info!(
            "Encryption key rotation completed: {} credentials re-encrypted",
            count
        );

        Ok(count)
    }

    /// 查找账号的特定类型凭证
    async fn find_by_account_and_type(
        &self,
        account_id: Uuid,
        credential_type: &CredentialType,
    ) -> Result<Option<account_credentials::Model>> {
        let cred = account_credentials::Entity::find()
            .filter(account_credentials::Column::AccountId.eq(account_id))
            .filter(account_credentials::Column::CredentialType.eq(credential_type.to_string()))
            .one(&self.db)
            .await?;

        Ok(cred)
    }

    /// 将数据库模型转换为领域模型
    fn db_model_to_domain(&self, model: account_credentials::Model) -> AccountCredential {
        AccountCredential {
            id: model.id,
            account_id: model.account_id,
            credential_type: model
                .credential_type
                .parse()
                .unwrap_or(CredentialType::ApiKey),
            encrypted_value: model.encrypted_value,
            metadata: model.metadata,
            expires_at: model.expires_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_type_parse() {
        assert_eq!(
            "api_key".parse::<CredentialType>().unwrap(),
            CredentialType::ApiKey
        );
        assert_eq!(
            "oauth_token".parse::<CredentialType>().unwrap(),
            CredentialType::OAuthToken
        );
    }

    #[test]
    fn test_encryption_decryption() {
        let db = sea_orm::DatabaseConnection::Disconnected;
        let service = AccountCredentialsPersistence::new(db, "test-encryption-key-32b".to_string());

        let plaintext = "my-secret-api-key";
        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
        assert_ne!(plaintext, encrypted);
    }

    #[test]
    fn test_key_derivation() {
        let key1 = AccountCredentialsPersistence::derive_key("test-key");
        let key2 = AccountCredentialsPersistence::derive_key("test-key");
        let key3 = AccountCredentialsPersistence::derive_key("different-key");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
        assert_eq!(key1.len(), 32);
    }
}
