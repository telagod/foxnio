//! 全局加密服务管理
//!
//! 提供全局的加密服务实例，支持运行时初始化和密钥轮换。

use crate::utils::{EncryptionError, EncryptionService};
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// 全局加密服务实例
static ENCRYPTION_SERVICE: OnceCell<Arc<EncryptionService>> = OnceCell::new();

/// 初始化全局加密服务
///
/// 从环境变量 FOXNIO_MASTER_KEY 读取密钥并初始化加密服务。
/// 如果环境变量未设置，将返回错误。
///
/// # Environment Variables
/// - `FOXNIO_MASTER_KEY`: Base64 编码的 32 字节主密钥
///   - 单密钥模式: `base64-encoded-key`
///   - 密钥轮换模式: `new-key:old-key`（冒号分隔）
///
/// # Example
/// ```ignore
/// // 在应用启动时调用
/// init_encryption_service().expect("Failed to initialize encryption");
///
/// // 之后可以使用全局服务
/// let enc = get_encryption_service().unwrap();
/// let encrypted = enc.encrypt("secret").unwrap();
/// ```
pub fn init_encryption_service() -> Result<(), EncryptionError> {
    let service = EncryptionService::from_env()
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    ENCRYPTION_SERVICE.set(Arc::new(service)).map_err(|_| {
        EncryptionError::EncryptionFailed("Encryption service already initialized".to_string())
    })?;

    tracing::info!("Global encryption service initialized");
    Ok(())
}

/// 使用指定的密钥初始化全局加密服务
///
/// 用于测试或自定义密钥管理场景
pub fn init_encryption_service_with_key(master_key: &[u8]) -> Result<(), EncryptionError> {
    let service = EncryptionService::new(master_key)
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    ENCRYPTION_SERVICE.set(Arc::new(service)).map_err(|_| {
        EncryptionError::EncryptionFailed("Encryption service already initialized".to_string())
    })?;

    tracing::info!("Global encryption service initialized with custom key");
    Ok(())
}

/// 使用密钥轮换初始化全局加密服务
///
/// # Arguments
/// * `master_key` - 新的主密钥（用于加密）
/// * `old_master_key` - 旧的主密钥（用于解密旧数据）
pub fn init_encryption_service_with_rotation(
    master_key: &[u8],
    old_master_key: &[u8],
) -> Result<(), EncryptionError> {
    let service = EncryptionService::with_rotation(master_key, old_master_key)
        .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

    ENCRYPTION_SERVICE.set(Arc::new(service)).map_err(|_| {
        EncryptionError::EncryptionFailed("Encryption service already initialized".to_string())
    })?;

    tracing::info!("Global encryption service initialized with key rotation");
    Ok(())
}

/// 获取全局加密服务实例
///
/// 如果服务未初始化，返回 None
pub fn get_encryption_service() -> Option<Arc<EncryptionService>> {
    ENCRYPTION_SERVICE.get().cloned()
}

/// 获取全局加密服务实例，如果未初始化则 panic
///
/// 用于确保加密服务已初始化的场景
pub fn encryption_service() -> Arc<EncryptionService> {
    ENCRYPTION_SERVICE
        .get()
        .expect("Encryption service not initialized. Call init_encryption_service() first.")
        .clone()
}

/// 检查加密服务是否已初始化
pub fn is_encryption_initialized() -> bool {
    ENCRYPTION_SERVICE.get().is_some()
}

/// 重置加密服务（仅用于测试）
///
/// # Safety
/// 此函数会清除全局加密服务，仅应在测试中使用
#[cfg(test)]
pub fn reset_encryption_service() {
    // OnceCell 不支持 reset，所以这个函数主要用于文档说明
    // 在测试中应该使用独立的 EncryptionService 实例
}

/// 加密辅助函数
pub struct GlobalEncryption;

impl GlobalEncryption {
    /// 加密字符串
    pub fn encrypt(plaintext: &str) -> Result<String, EncryptionError> {
        let enc = get_encryption_service().ok_or(EncryptionError::MasterKeyNotConfigured)?;
        enc.encrypt(plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))
    }

    /// 解密字符串
    pub fn decrypt(ciphertext: &str) -> Result<String, EncryptionError> {
        let enc = get_encryption_service().ok_or(EncryptionError::MasterKeyNotConfigured)?;
        enc.decrypt(ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }

    /// 哈希敏感数据
    pub fn hash_sensitive(data: &str) -> Result<String, EncryptionError> {
        let enc = get_encryption_service().ok_or(EncryptionError::MasterKeyNotConfigured)?;
        enc.hash_sensitive(data)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))
    }

    /// 验证哈希
    pub fn verify_hash(data: &str, hash: &str) -> bool {
        get_encryption_service()
            .map(|enc| enc.verify_hash(data, hash))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    // 注意：由于 OnceCell 的特性，这些测试需要按顺序运行
    // 使用 #[serial] 或类似的机制

    fn create_test_key() -> Vec<u8> {
        let key = EncryptionService::generate_master_key();
        BASE64.decode(&key).unwrap()
    }

    #[test]
    fn test_init_and_get_service() {
        // 由于全局状态，这个测试可能会影响其他测试
        // 在实际测试中应该使用独立的 EncryptionService 实例
        let key = create_test_key();

        // 检查初始化状态（可能已被其他测试初始化）
        let was_initialized = is_encryption_initialized();

        if !was_initialized {
            let result = init_encryption_service_with_key(&key);
            assert!(result.is_ok());
            assert!(is_encryption_initialized());
        }

        let service = get_encryption_service();
        assert!(service.is_some());
    }

    #[test]
    fn test_global_encryption_encrypt_decrypt() {
        let key = create_test_key();

        if !is_encryption_initialized() {
            let _ = init_encryption_service_with_key(&key);
        }

        let enc = encryption_service();
        let plaintext = "test-secret-data";

        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
