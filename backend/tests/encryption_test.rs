#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 加密服务集成测试
//!
//! 测试加密服务的完整功能，包括：
//! - 加密解密
//! - 密钥轮换
//! - 性能测试
//! - 与实体集成

#![allow(dead_code)]
#![allow(unused_imports)]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use foxnio::utils::encryption_global::{
    init_encryption_service_with_key, is_encryption_initialized,
};
use foxnio::utils::{get_encryption_service, EncryptedString, EncryptionService};

/// 创建测试用的加密服务
fn create_test_encryption_service() -> EncryptionService {
    let key = EncryptionService::generate_master_key();
    let key_bytes = BASE64.decode(&key).unwrap();
    EncryptionService::new(&key_bytes).unwrap()
}

/// 创建指定字节的密钥
fn create_key_from_bytes(bytes: [u8; 32]) -> EncryptionService {
    EncryptionService::new(&bytes).unwrap()
}

#[cfg(test)]
mod encryption_tests {
    use super::*;

    #[test]
    fn test_basic_encryption_decryption() {
        let enc = create_test_encryption_service();
        let plaintext = "my-secret-api-key-12345";

        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
        assert_ne!(encrypted, plaintext);
    }

    #[test]
    fn test_different_ciphertext_for_same_plaintext() {
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
    fn test_empty_string() {
        let enc = create_test_encryption_service();

        let encrypted = enc.encrypt("").unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_unicode_and_special_chars() {
        let enc = create_test_encryption_service();
        let test_cases = vec![
            "中文测试",
            "🔐 API密钥",
            "emoji: 🎉🚀✨",
            "special: !@#$%^&*(){}[]|\\:;\"'<>,.?/~`",
            "multiline:\nline1\nline2\r\nline3",
            "tabs:\t\t\t",
        ];

        for plaintext in test_cases {
            let encrypted = enc.encrypt(plaintext).unwrap();
            let decrypted = enc.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, plaintext, "Failed for: {}", plaintext);
        }
    }

    #[test]
    fn test_large_data() {
        let enc = create_test_encryption_service();

        // 1MB 数据
        let plaintext = "x".repeat(1024 * 1024);

        let encrypted = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = b"too-short";
        let result = EncryptionService::new(short_key);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid master key length"));
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let enc = create_test_encryption_service();

        // 无效的 base64
        let result = enc.decrypt("not-valid-base64!!!");
        assert!(result.is_err());

        // 太短的密文
        let short_ciphertext = BASE64.encode(b"short");
        let result = enc.decrypt(&short_ciphertext);
        assert!(result.is_err());

        // 格式正确的无效密文
        let fake_ciphertext = vec![0u8; 28]; // nonce(12) + tag(16)
        let fake_base64 = BASE64.encode(&fake_ciphertext);
        let result = enc.decrypt(&fake_base64);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_string_wrapper() {
        let enc = create_test_encryption_service();
        let plain = "my-secret-key";

        let encrypted_str = EncryptedString::from_plain(plain, &enc).unwrap();

        // 解密
        let decrypted = encrypted_str.to_plain(&enc).unwrap();
        assert_eq!(decrypted, plain);

        // 获取加密后的值
        let encrypted_value = encrypted_str.encrypted();
        assert!(!encrypted_value.is_empty());
        assert_ne!(encrypted_value, plain);

        // Debug 输出应该隐藏内容
        let debug_output = format!("{:?}", encrypted_str);
        assert!(debug_output.contains("[ENCRYPTED]"));
    }

    #[test]
    fn test_encrypted_string_serialization() {
        let enc = create_test_encryption_service();
        let plain = "my-secret";

        let encrypted_str = EncryptedString::from_plain(plain, &enc).unwrap();

        // JSON 序列化
        let json = serde_json::to_string(&encrypted_str).unwrap();

        // 反序列化
        let deserialized: EncryptedString = serde_json::from_str(&json).unwrap();

        // 验证可以解密
        let decrypted = deserialized.to_plain(&enc).unwrap();
        assert_eq!(decrypted, plain);
    }

    #[test]
    fn test_encrypted_string_from_database() {
        let enc = create_test_encryption_service();
        let plain = "stored-secret";

        // 模拟存储到数据库
        let encrypted = enc.encrypt(plain).unwrap();

        // 模拟从数据库读取
        let encrypted_str = EncryptedString::from_encrypted(encrypted);

        // 解密
        let decrypted = encrypted_str.to_plain(&enc).unwrap();
        assert_eq!(decrypted, plain);
    }
}

#[cfg(test)]
mod key_rotation_tests {
    use super::*;

    #[test]
    fn test_decrypt_with_old_key() {
        // 创建两个不同的密钥
        let new_key: [u8; 32] = [1u8; 32];
        let old_key: [u8; 32] = [2u8; 32];

        // 使用旧密钥加密
        let old_enc = create_key_from_bytes(old_key);
        let plaintext = "secret-data";
        let encrypted = old_enc.encrypt(plaintext).unwrap();

        // 使用支持轮换的新服务解密
        let rotation_enc = EncryptionService::with_rotation(&new_key, &old_key).unwrap();
        let decrypted = rotation_enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_with_new_key() {
        let new_key: [u8; 32] = [1u8; 32];
        let old_key: [u8; 32] = [2u8; 32];

        let rotation_enc = EncryptionService::with_rotation(&new_key, &old_key).unwrap();

        let plaintext = "secret-data";
        let encrypted = rotation_enc.encrypt(plaintext).unwrap();

        // 新密钥加密的数据应该可以用新密钥解密
        let new_enc = create_key_from_bytes(new_key);
        let decrypted = new_enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);

        // 但不能用旧密钥解密
        let old_enc = create_key_from_bytes(old_key);
        assert!(old_enc.decrypt(&encrypted).is_err());
    }

    #[test]
    fn test_rotation_mode_detection() {
        let new_key: [u8; 32] = [1u8; 32];
        let old_key: [u8; 32] = [2u8; 32];

        let single_enc = create_key_from_bytes(new_key);
        assert!(!single_enc.has_old_key());

        let rotation_enc = EncryptionService::with_rotation(&new_key, &old_key).unwrap();
        assert!(rotation_enc.has_old_key());
    }

    #[test]
    fn test_mixed_data_decryption() {
        let new_key: [u8; 32] = [1u8; 32];
        let old_key: [u8; 32] = [2u8; 32];

        let new_enc = create_key_from_bytes(new_key);
        let old_enc = create_key_from_bytes(old_key);
        let rotation_enc = EncryptionService::with_rotation(&new_key, &old_key).unwrap();

        // 用旧密钥加密
        let old_encrypted = old_enc.encrypt("old-secret").unwrap();

        // 用新密钥加密
        let new_encrypted = new_enc.encrypt("new-secret").unwrap();

        // 轮换服务应该能解密两者
        assert_eq!(rotation_enc.decrypt(&old_encrypted).unwrap(), "old-secret");
        assert_eq!(rotation_enc.decrypt(&new_encrypted).unwrap(), "new-secret");
    }
}

#[cfg(test)]
mod hashing_tests {
    use super::*;

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
    fn test_different_data_different_hash() {
        let enc = create_test_encryption_service();

        let hash1 = enc.hash_sensitive("password1").unwrap();
        let hash2 = enc.hash_sensitive("password2").unwrap();

        assert_ne!(hash1, hash2);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_encryption_performance() {
        let enc = create_test_encryption_service();
        let plaintext = "x".repeat(1000); // 1KB

        // 预热
        for _ in 0..10 {
            let encrypted = enc.encrypt(&plaintext).unwrap();
            let _ = enc.decrypt(&encrypted).unwrap();
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
    fn test_large_data_performance() {
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

    #[test]
    fn test_concurrent_encryption() {
        use std::sync::Arc;
        use std::thread;

        let enc = Arc::new(create_test_encryption_service());
        let plaintext = "concurrent-test-data";

        let mut handles = vec![];

        let start = Instant::now();

        for _ in 0..10 {
            let enc_clone = Arc::clone(&enc);
            let plain = plaintext.to_string();
            handles.push(thread::spawn(move || {
                let encrypted = enc_clone.encrypt(&plain).unwrap();
                let decrypted = enc_clone.decrypt(&encrypted).unwrap();
                assert_eq!(decrypted, plain);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        println!("10 concurrent encryptions: {:?}", elapsed);
    }
}

#[cfg(test)]
mod global_service_tests {
    use super::*;

    fn reset_global_state() {
        // 注意：由于 OnceCell 的限制，我们无法重置全局状态
        // 测试应该假设全局服务已初始化
    }

    #[test]
    fn test_init_and_check() {
        // 检查全局服务状态（可能已被其他测试初始化）
        let is_init = is_encryption_initialized();
        println!("Encryption initialized: {}", is_init);
    }

    #[test]
    fn test_get_service() {
        let service = get_encryption_service();
        println!("Service available: {}", service.is_some());
    }
}

#[cfg(test)]
mod key_generation_tests {
    use super::*;

    #[test]
    fn test_generate_master_key() {
        let key = EncryptionService::generate_master_key();
        assert!(!key.is_empty());

        // 验证可以解码为 32 字节
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);

        // 验证可以用这个密钥创建服务
        let enc = EncryptionService::new(&decoded).unwrap();

        // 验证可以正常加密解密
        let encrypted = enc.encrypt("test").unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "test");
    }

    #[test]
    fn test_generate_unique_keys() {
        let key1 = EncryptionService::generate_master_key();
        let key2 = EncryptionService::generate_master_key();

        // 每次生成的密钥应该不同
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_key_format() {
        let key = EncryptionService::generate_master_key();

        // 应该是有效的 base64
        let decoded = BASE64.decode(&key).unwrap();
        assert_eq!(decoded.len(), 32);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_max_length_string() {
        let enc = create_test_encryption_service();

        // 测试非常大的字符串（10MB）
        let plaintext = "x".repeat(10 * 1024 * 1024);

        let encrypted = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_binary_data() {
        let enc = create_test_encryption_service();

        // 测试二进制数据（所有可能的字节值）
        let mut plaintext = Vec::with_capacity(256);
        for i in 0..=255u8 {
            plaintext.push(i);
        }

        let encrypted = enc.encrypt_bytes(&plaintext).unwrap();
        let decrypted = enc.decrypt_bytes(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_null_bytes() {
        let enc = create_test_encryption_service();
        let plaintext = "hello\0world\0test";

        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_whitespace_only() {
        let enc = create_test_encryption_service();
        let test_cases = vec![" ", "  ", "\t", "\n", "\r\n", "   \t\n\r\n   "];

        for plaintext in test_cases {
            let encrypted = enc.encrypt(plaintext).unwrap();
            let decrypted = enc.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, plaintext);
        }
    }
}
