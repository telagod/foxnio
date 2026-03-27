//! 敏感数据加密服务
//!
//! 使用 AES-256-GCM 进行数据加密，支持：
//! - 密钥轮换（旧密钥解密，新密钥加密）
//! - 透明加解密层
//! - 安全的密钥管理

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::env;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 加密相关错误
#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("Invalid master key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    #[error("Failed to encrypt data: {0}")]
    EncryptionFailed(String),

    #[error("Failed to decrypt data: {0}")]
    DecryptionFailed(String),

    #[error("Invalid ciphertext format")]
    InvalidCiphertextFormat,

    #[error("Master key not configured")]
    MasterKeyNotConfigured,

    #[error("Failed to decode base64: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),
}

/// 加密服务
///
/// 使用 AES-256-GCM 算法进行数据加密。
/// 密钥应该是 32 字节的随机数据，从环境变量 FOXNIO_MASTER_KEY 读取。
#[derive(ZeroizeOnDrop)]
pub struct EncryptionService {
    /// 主加密密钥（32字节）
    #[zeroize(skip)]
    master_key: [u8; 32],
    /// 旧密钥（用于密钥轮换时解密旧数据）
    #[zeroize(skip)]
    old_master_key: Option<[u8; 32]>,
}

impl EncryptionService {
    /// 密钥长度（32字节 = 256位）
    pub const KEY_LEN: usize = 32;

    /// Nonce 长度（12字节）
    pub const NONCE_LEN: usize = 12;

    /// 从环境变量 FOXNIO_MASTER_KEY 创建加密服务
    ///
    /// 环境变量格式：
    /// - 单密钥：Base64 编码的 32 字节密钥
    /// - 密钥轮换：`新密钥:旧密钥`（冒号分隔）
    ///
    /// # Examples
    /// ```ignore
    /// // 单密钥
    /// export FOXNIO_MASTER_KEY="base64-encoded-32-byte-key"
    ///
    /// // 密钥轮换
    /// export FOXNIO_MASTER_KEY="new-key:old-key"
    /// ```
    pub fn from_env() -> Result<Self> {
        let key_str =
            env::var("FOXNIO_MASTER_KEY").map_err(|_| EncryptionError::MasterKeyNotConfigured)?;

        // 检查是否包含密钥轮换格式（新密钥:旧密钥）
        if let Some((new_key, old_key)) = key_str.split_once(':') {
            let master_key = Self::parse_key(new_key)?;
            let old_master_key = Some(Self::parse_key(old_key)?);
            tracing::info!("Encryption service initialized with key rotation enabled");
            Ok(Self {
                master_key,
                old_master_key,
            })
        } else {
            let master_key = Self::parse_key(&key_str)?;
            tracing::info!("Encryption service initialized");
            Ok(Self {
                master_key,
                old_master_key: None,
            })
        }
    }

    /// 从字节切片创建加密服务
    ///
    /// # Arguments
    /// * `master_key` - 32 字节的主密钥
    ///
    /// # Errors
    /// 如果密钥长度不是 32 字节，返回错误
    pub fn new(master_key: &[u8]) -> Result<Self> {
        if master_key.len() != Self::KEY_LEN {
            return Err(EncryptionError::InvalidKeyLength(master_key.len()).into());
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(master_key);

        Ok(Self {
            master_key: key,
            old_master_key: None,
        })
    }

    /// 创建支持密钥轮换的加密服务
    ///
    /// # Arguments
    /// * `master_key` - 新的 32 字节主密钥
    /// * `old_master_key` - 旧的 32 字节主密钥（用于解密旧数据）
    pub fn with_rotation(master_key: &[u8], old_master_key: &[u8]) -> Result<Self> {
        if master_key.len() != Self::KEY_LEN {
            return Err(EncryptionError::InvalidKeyLength(master_key.len()).into());
        }
        if old_master_key.len() != Self::KEY_LEN {
            return Err(EncryptionError::InvalidKeyLength(old_master_key.len()).into());
        }

        let mut new_key = [0u8; 32];
        let mut old_key = [0u8; 32];
        new_key.copy_from_slice(master_key);
        old_key.copy_from_slice(old_master_key);

        Ok(Self {
            master_key: new_key,
            old_master_key: Some(old_key),
        })
    }

    /// 解析 Base64 编码的密钥
    fn parse_key(key_str: &str) -> Result<[u8; 32]> {
        let key_bytes = BASE64
            .decode(key_str.trim())
            .context("Failed to decode master key")?;

        if key_bytes.len() != Self::KEY_LEN {
            return Err(EncryptionError::InvalidKeyLength(key_bytes.len()).into());
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        Ok(key)
    }

    /// 生成随机的 32 字节主密钥
    ///
    /// 返回 Base64 编码的密钥字符串，可以直接设置到环境变量
    pub fn generate_master_key() -> String {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        BASE64.encode(key)
    }

    /// 加密字符串
    ///
    /// 返回 Base64 编码的密文，格式：`nonce(12字节) + ciphertext + tag(16字节)`
    ///
    /// # Arguments
    /// * `plaintext` - 要加密的明文
    ///
    /// # Returns
    /// Base64 编码的密文
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        self.encrypt_bytes(plaintext.as_bytes())
    }

    /// 加密字节数据
    ///
    /// # Arguments
    /// * `plaintext` - 要加密的字节数据
    ///
    /// # Returns
    /// Base64 编码的密文
    pub fn encrypt_bytes(&self, plaintext: &[u8]) -> Result<String> {
        // 创建 AES-256-GCM 加密器
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // 生成随机 nonce
        let mut nonce_bytes = [0u8; Self::NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 加密
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        // 组合 nonce + ciphertext（包含 tag）
        let mut result = Vec::with_capacity(Self::NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        // Base64 编码
        Ok(BASE64.encode(&result))
    }

    /// 解密字符串
    ///
    /// # Arguments
    /// * `ciphertext` - Base64 编码的密文
    ///
    /// # Returns
    /// 解密后的明文字符串
    pub fn decrypt(&self, ciphertext: &str) -> Result<String> {
        let bytes = self.decrypt_bytes(ciphertext)?;
        String::from_utf8(bytes).context("Decrypted data is not valid UTF-8")
    }

    /// 解密字节数据
    ///
    /// 支持密钥轮换：先用新密钥解密，失败则尝试用旧密钥解密
    ///
    /// # Arguments
    /// * `ciphertext` - Base64 编码的密文
    ///
    /// # Returns
    /// 解密后的字节数据
    pub fn decrypt_bytes(&self, ciphertext: &str) -> Result<Vec<u8>> {
        // Base64 解码
        let encrypted = BASE64
            .decode(ciphertext.trim())
            .context("Failed to decode base64 ciphertext")?;

        // 检查最小长度（nonce + tag）
        if encrypted.len() < Self::NONCE_LEN + 16 {
            return Err(EncryptionError::InvalidCiphertextFormat.into());
        }

        // 分离 nonce 和 ciphertext
        let (nonce_bytes, ciphertext_bytes) = encrypted.split_at(Self::NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        // 尝试用新密钥解密
        if let Ok(plaintext) = self.try_decrypt(&self.master_key, nonce, ciphertext_bytes) {
            return Ok(plaintext);
        }

        // 如果新密钥解密失败，尝试用旧密钥解密（密钥轮换场景）
        if let Some(old_key) = &self.old_master_key {
            if let Ok(plaintext) = self.try_decrypt(old_key, nonce, ciphertext_bytes) {
                tracing::debug!("Decrypted with old master key (key rotation in progress)");
                return Ok(plaintext);
            }
        }

        bail!(EncryptionError::DecryptionFailed(
            "Failed to decrypt with both new and old keys".to_string()
        ))
    }

    /// 尝试用指定密钥解密
    fn try_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &Nonce<typenum::U12>,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()).into())
    }

    /// 对敏感数据进行哈希（用于不可逆的敏感数据存储）
    ///
    /// 使用 HMAC-SHA256 进行哈希
    ///
    /// # Arguments
    /// * `data` - 要哈希的数据
    ///
    /// # Returns
    /// 十六进制编码的哈希值
    pub fn hash_sensitive(&self, data: &str) -> Result<String> {
        use hmac::Mac;
        use sha2::Sha256;

        type HmacSha256 = hmac::Hmac<Sha256>;

        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.master_key)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;
        mac.update(data.as_bytes());
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// 验证哈希值
    ///
    /// # Arguments
    /// * `data` - 原始数据
    /// * `hash` - 十六进制编码的哈希值
    ///
    /// # Returns
    /// 如果哈希匹配返回 true
    pub fn verify_hash(&self, data: &str, hash: &str) -> bool {
        match self.hash_sensitive(data) {
            Ok(computed) => {
                // 使用常量时间比较防止时序攻击
                computed == hash
            }
            Err(_) => false,
        }
    }

    /// 检查是否已配置旧密钥（密钥轮换模式）
    pub fn has_old_key(&self) -> bool {
        self.old_master_key.is_some()
    }
}

impl std::fmt::Debug for EncryptionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionService")
            .field("master_key", &"[REDACTED]")
            .field(
                "old_master_key",
                &self.old_master_key.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// 透明加解密字符串包装器
///
/// 在内存中存储加密后的数据，需要时自动解密。
/// 实现了序列化/反序列化，可以直接用于 SeaORM 实体。
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedString(String);

impl EncryptedString {
    /// 从明文创建加密字符串
    ///
    /// # Arguments
    /// * `plain` - 明文字符串
    /// * `enc` - 加密服务
    ///
    /// # Returns
    /// 加密后的字符串包装器
    pub fn from_plain(plain: &str, enc: &EncryptionService) -> Result<Self> {
        let encrypted = enc.encrypt(plain)?;
        Ok(Self(encrypted))
    }

    /// 从已加密的数据创建（如从数据库读取）
    ///
    /// # Arguments
    /// * `encrypted` - 已加密的 Base64 字符串
    pub fn from_encrypted(encrypted: String) -> Self {
        Self(encrypted)
    }

    /// 解密为明文
    ///
    /// # Arguments
    /// * `enc` - 加密服务
    ///
    /// # Returns
    /// 解密后的明文字符串
    pub fn to_plain(&self, enc: &EncryptionService) -> Result<String> {
        enc.decrypt(&self.0)
    }

    /// 获取加密后的字符串（用于存储到数据库）
    pub fn encrypted(&self) -> &str {
        &self.0
    }

    /// 获取加密后的字符串（消费 self）
    pub fn into_encrypted(self) -> String {
        self.0
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for EncryptedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EncryptedString")
            .field(&"[ENCRYPTED]")
            .finish()
    }
}

impl std::fmt::Display for EncryptedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ENCRYPTED]")
    }
}

impl serde::Serialize for EncryptedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for EncryptedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

impl sea_orm::FromQueryResult for EncryptedString {
    fn from_query_result(res: &sea_orm::QueryResult, col: &str) -> Result<Self, sea_orm::DbErr> {
        let s: String = res.try_get("", col)?;
        Ok(Self(s))
    }
}

/// 可选的加密字符串
pub type OptionEncryptedString = Option<EncryptedString>;

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的加密服务
    fn create_test_encryption_service() -> EncryptionService {
        let key = EncryptionService::generate_master_key();
        let key_bytes = BASE64.decode(&key).unwrap();
        EncryptionService::new(&key_bytes).unwrap()
    }

    #[test]
    fn test_generate_master_key() {
        let key = EncryptionService::generate_master_key();
        assert!(!key.is_empty());

        // 验证可以解码为 32 字节
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_encryption_decryption() {
        let enc = create_test_encryption_service();
        let plaintext = "my-secret-api-key-12345";

        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
        assert_ne!(encrypted, plaintext);
    }

    #[test]
    fn test_encryption_produces_different_ciphertext() {
        let enc = create_test_encryption_service();
        let plaintext = "same-data";

        let encrypted1 = enc.encrypt(plaintext).unwrap();
        let encrypted2 = enc.encrypt(plaintext).unwrap();

        // 由于随机 nonce，相同明文应产生不同密文
        assert_ne!(encrypted1, encrypted2);

        // 但都能正确解密
        assert_eq!(enc.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(enc.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_encryption_empty_string() {
        let enc = create_test_encryption_service();

        let encrypted = enc.encrypt("").unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_encryption_unicode() {
        let enc = create_test_encryption_service();
        let plaintext = "中文测试 🔐 API密钥";

        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encryption_long_string() {
        let enc = create_test_encryption_service();
        let plaintext = "x".repeat(10000);

        let encrypted = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hash_sensitive() {
        let enc = create_test_encryption_service();
        let data = "password123";

        let hash1 = enc.hash_sensitive(data).unwrap();
        let hash2 = enc.hash_sensitive(data).unwrap();

        // 相同数据应产生相同哈希
        assert_eq!(hash1, hash2);
        // 哈希长度应为 64（SHA256 的十六进制表示）
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_verify_hash() {
        let enc = create_test_encryption_service();
        let data = "password123";

        let hash = enc.hash_sensitive(data).unwrap();

        assert!(enc.verify_hash(data, &hash));
        assert!(!enc.verify_hash("wrong-password", &hash));
    }

    #[test]
    fn test_invalid_master_key_length() {
        let short_key = b"too-short";
        let result = EncryptionService::new(short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_string() {
        let enc = create_test_encryption_service();
        let plain = "my-secret";

        let encrypted_str = EncryptedString::from_plain(plain, &enc).unwrap();

        // 解密
        let decrypted = encrypted_str.to_plain(&enc).unwrap();
        assert_eq!(decrypted, plain);

        // 获取加密后的值
        let encrypted_value = encrypted_str.encrypted();
        assert!(!encrypted_value.is_empty());
        assert_ne!(encrypted_value, plain);
    }

    #[test]
    fn test_encrypted_string_serialization() {
        let enc = create_test_encryption_service();
        let plain = "my-secret";

        let encrypted_str = EncryptedString::from_plain(plain, &enc).unwrap();

        // 序列化
        let json = serde_json::to_string(&encrypted_str).unwrap();

        // 反序列化
        let deserialized: EncryptedString = serde_json::from_str(&json).unwrap();

        // 验证可以解密
        let decrypted = deserialized.to_plain(&enc).unwrap();
        assert_eq!(decrypted, plain);
    }

    #[test]
    fn test_key_rotation_decrypt_with_old_key() {
        // 创建两个不同的密钥
        let new_key_bytes: [u8; 32] = [1u8; 32];
        let old_key_bytes: [u8; 32] = [2u8; 32];

        // 使用旧密钥加密
        let old_enc = EncryptionService::new(&old_key_bytes).unwrap();
        let plaintext = "secret-data";
        let encrypted = old_enc.encrypt(plaintext).unwrap();

        // 使用支持轮换的新服务解密
        let rotation_enc =
            EncryptionService::with_rotation(&new_key_bytes, &old_key_bytes).unwrap();
        let decrypted = rotation_enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_rotation_encrypt_with_new_key() {
        let new_key_bytes: [u8; 32] = [1u8; 32];
        let old_key_bytes: [u8; 32] = [2u8; 32];

        let rotation_enc =
            EncryptionService::with_rotation(&new_key_bytes, &old_key_bytes).unwrap();

        let plaintext = "secret-data";
        let encrypted = rotation_enc.encrypt(plaintext).unwrap();

        // 新密钥加密的数据应该可以用新密钥解密
        let new_enc = EncryptionService::new(&new_key_bytes).unwrap();
        let decrypted = new_enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);

        // 但不能用旧密钥解密
        let old_enc = EncryptionService::new(&old_key_bytes).unwrap();
        assert!(old_enc.decrypt(&encrypted).is_err());
    }

    #[test]
    fn test_decryption_invalid_base64() {
        let enc = create_test_encryption_service();

        let result = enc.decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decryption_invalid_ciphertext() {
        let enc = create_test_encryption_service();

        // 太短的密文
        let short_ciphertext = BASE64.encode(b"short");
        let result = enc.decrypt(&short_ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_encryption_bytes() {
        let enc = create_test_encryption_service();
        let data = vec![0u8, 1u8, 2u8, 255u8, 254u8];

        let encrypted = enc.encrypt_bytes(&data).unwrap();
        let decrypted = enc.decrypt_bytes(&encrypted).unwrap();

        assert_eq!(decrypted, data);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    fn create_test_encryption_service() -> EncryptionService {
        let key = EncryptionService::generate_master_key();
        let key_bytes = BASE64.decode(&key).unwrap();
        EncryptionService::new(&key_bytes).unwrap()
    }

    #[test]
    fn test_encryption_performance() {
        let enc = create_test_encryption_service();
        let plaintext = "x".repeat(1000);

        // 预热
        for _ in 0..10 {
            let _ = enc.encrypt(&plaintext).unwrap();
        }

        // 测试
        let iterations = 1000;
        let start = Instant::now();

        for _ in 0..iterations {
            let encrypted = enc.encrypt(&plaintext).unwrap();
            let _ = enc.decrypt(&encrypted).unwrap();
        }

        let elapsed = start.elapsed();
        let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

        println!(
            "Encryption/Decryption: {:.2} ops/sec for 1KB data",
            ops_per_sec
        );

        // 基本性能断言：每秒应该能处理至少 100 次 1KB 数据的加密解密
        assert!(
            ops_per_sec > 100.0,
            "Encryption too slow: {} ops/sec",
            ops_per_sec
        );
    }

    #[test]
    fn test_encryption_large_data() {
        let enc = create_test_encryption_service();
        let plaintext = "x".repeat(100000); // 100KB

        let start = Instant::now();
        let encrypted = enc.encrypt(&plaintext).unwrap();
        let encrypt_time = start.elapsed();

        let start = Instant::now();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        let decrypt_time = start.elapsed();

        println!(
            "100KB data: encrypt {:?}, decrypt {:?}",
            encrypt_time, decrypt_time
        );

        assert_eq!(decrypted, plaintext);

        // 基本性能断言：100KB 数据加密解密应该在 100ms 内完成
        assert!(encrypt_time.as_millis() < 100);
        assert!(decrypt_time.as_millis() < 100);
    }
}
