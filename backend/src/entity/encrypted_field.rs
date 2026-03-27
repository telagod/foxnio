//! 加密实体字段
//!
//! 提供用于 SeaORM 实体的加密字段类型，支持透明加解密。

use crate::utils::{EncryptionService, EncryptedString};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 加密字符串字段
///
/// 在数据库中存储加密后的数据，读取时自动解密。
/// 使用前需要初始化全局加密服务。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedField {
    inner: Option<EncryptedString>,
}

impl EncryptedField {
    /// 创建空的加密字段
    pub fn none() -> Self {
        Self { inner: None }
    }

    /// 从明文创建加密字段
    pub fn from_plain(plain: &str, enc: &EncryptionService) -> Result<Self, anyhow::Error> {
        if plain.is_empty() {
            return Ok(Self { inner: None });
        }
        let encrypted = EncryptedString::from_plain(plain, enc)?;
        Ok(Self {
            inner: Some(encrypted),
        })
    }

    /// 从已加密的字符串创建（如从数据库读取）
    pub fn from_encrypted(encrypted: Option<String>) -> Self {
        Self {
            inner: encrypted.map(EncryptedString::from_encrypted),
        }
    }

    /// 解密为明文
    pub fn to_plain(&self, enc: &EncryptionService) -> Result<Option<String>, anyhow::Error> {
        match &self.inner {
            Some(encrypted) => {
                let plain = encrypted.to_plain(enc)?;
                Ok(Some(plain))
            }
            None => Ok(None),
        }
    }

    /// 获取加密后的字符串
    pub fn encrypted(&self) -> Option<&str> {
        self.inner.as_ref().map(|e| e.encrypted())
    }

    /// 检查是否为空
    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    /// 检查是否有值
    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }
}

impl Default for EncryptedField {
    fn default() -> Self {
        Self::none()
    }
}

impl From<Option<String>> for EncryptedField {
    fn from(value: Option<String>) -> Self {
        Self::from_encrypted(value)
    }
}

impl From<String> for EncryptedField {
    fn from(value: String) -> Self {
        if value.is_empty() {
            Self::none()
        } else {
            Self::from_encrypted(Some(value))
        }
    }
}

/// 可为空的加密字符串（用于 SeaORM 实体）
///
/// 示例用法：
/// ```ignore
/// use entity::encrypted_field::NullableEncryptedString;
///
/// #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
/// #[sea_orm(table_name = "users")]
/// pub struct Model {
///     #[sea_orm(primary_key)]
///     pub id: Uuid,
///     pub totp_secret: NullableEncryptedString,
/// }
/// ```
pub type NullableEncryptedString = Option<String>;

/// 加密字符串辅助函数
pub struct EncryptionHelper;

impl EncryptionHelper {
    /// 加密敏感字段
    ///
    /// 如果值为空或加密服务不可用，返回原值
    pub fn encrypt_field(
        value: Option<&str>,
        enc: Option<&EncryptionService>,
    ) -> Result<Option<String>, anyhow::Error> {
        match (value, enc) {
            (Some(v), Some(enc)) if !v.is_empty() => {
                let encrypted = enc.encrypt(v)?;
                Ok(Some(encrypted))
            }
            _ => Ok(value.map(|v| v.to_string())),
        }
    }

    /// 解密敏感字段
    ///
    /// 如果值为空或加密服务不可用，返回原值
    pub fn decrypt_field(
        value: Option<&str>,
        enc: Option<&EncryptionService>,
    ) -> Result<Option<String>, anyhow::Error> {
        match (value, enc) {
            (Some(v), Some(enc)) if !v.is_empty() => {
                let decrypted = enc.decrypt(v)?;
                Ok(Some(decrypted))
            }
            _ => Ok(value.map(|v| v.to_string())),
        }
    }

    /// 验证字段是否已加密
    ///
    /// 尝试解密，如果成功则说明是加密数据
    pub fn is_encrypted(value: &str, enc: &EncryptionService) -> bool {
        enc.decrypt(value).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_encryption_service() -> EncryptionService {
        let key = EncryptionService::generate_master_key();
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(&key)
            .unwrap();
        EncryptionService::new(&key_bytes).unwrap()
    }

    #[test]
    fn test_encrypted_field_none() {
        let field = EncryptedField::none();
        assert!(field.is_none());
        assert!(!field.is_some());
        assert!(field.encrypted().is_none());
    }

    #[test]
    fn test_encrypted_field_from_plain() {
        let enc = create_test_encryption_service();
        let plain = "my-secret-key";

        let field = EncryptedField::from_plain(plain, &enc).unwrap();

        assert!(field.is_some());

        let decrypted = field.to_plain(&enc).unwrap();
        assert_eq!(decrypted, Some(plain.to_string()));

        let encrypted = field.encrypted().unwrap();
        assert_ne!(encrypted, plain);
    }

    #[test]
    fn test_encrypted_field_from_plain_empty() {
        let enc = create_test_encryption_service();

        let field = EncryptedField::from_plain("", &enc).unwrap();
        assert!(field.is_none());
    }

    #[test]
    fn test_encrypted_field_from_encrypted() {
        let enc = create_test_encryption_service();
        let plain = "my-secret-key";

        // 先加密
        let encrypted = enc.encrypt(plain).unwrap();

        // 从加密数据创建
        let field = EncryptedField::from_encrypted(Some(encrypted));

        // 解密
        let decrypted = field.to_plain(&enc).unwrap();
        assert_eq!(decrypted, Some(plain.to_string()));
    }

    #[test]
    fn test_encryption_helper_encrypt_decrypt() {
        let enc = create_test_encryption_service();
        let plain = "secret-data";

        let encrypted =
            EncryptionHelper::encrypt_field(Some(plain), Some(&enc)).unwrap();
        assert!(encrypted.is_some());
        assert_ne!(encrypted.as_ref().unwrap(), plain);

        let decrypted =
            EncryptionHelper::decrypt_field(encrypted.as_deref(), Some(&enc)).unwrap();
        assert_eq!(decrypted, Some(plain.to_string()));
    }

    #[test]
    fn test_encryption_helper_with_none() {
        let enc = create_test_encryption_service();

        let encrypted = EncryptionHelper::encrypt_field(None, Some(&enc)).unwrap();
        assert!(encrypted.is_none());

        let decrypted = EncryptionHelper::decrypt_field(None, Some(&enc)).unwrap();
        assert!(decrypted.is_none());
    }

    #[test]
    fn test_encryption_helper_without_service() {
        let plain = "plain-data";

        // 没有加密服务时，应该返回原值
        let result = EncryptionHelper::encrypt_field(Some(plain), None).unwrap();
        assert_eq!(result, Some(plain.to_string()));

        let decrypted = EncryptionHelper::decrypt_field(Some(plain), None).unwrap();
        assert_eq!(decrypted, Some(plain.to_string()));
    }

    #[test]
    fn test_is_encrypted() {
        let enc = create_test_encryption_service();
        let plain = "secret";
        let encrypted = enc.encrypt(plain).unwrap();

        assert!(EncryptionHelper::is_encrypted(&encrypted, &enc));
        assert!(!EncryptionHelper::is_encrypted(plain, &enc));
    }

    #[test]
    fn test_encrypted_field_serialization() {
        let enc = create_test_encryption_service();
        let plain = "my-secret";

        let field = EncryptedField::from_plain(plain, &enc).unwrap();

        let json = serde_json::to_string(&field).unwrap();
        let deserialized: EncryptedField = serde_json::from_str(&json).unwrap();

        let decrypted = deserialized.to_plain(&enc).unwrap();
        assert_eq!(decrypted, Some(plain.to_string()));
    }
}
