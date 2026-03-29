#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 密码重置功能集成测试

use chrono::{Duration, Utc};
use foxnio::entity::password_reset_tokens;
use foxnio::service::email::{EmailSender, MockEmailSender};
use sha2::{Digest, Sha256};

/// 测试 token 生成和哈希
#[test]
fn test_token_generation_and_hashing() {
    // 生成随机 token
    let token = generate_test_token();

    // Token 应该是 64 个十六进制字符（32 字节）
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

    // 哈希 token
    let hash = hash_token(&token);

    // 哈希应该是 64 个十六进制字符（SHA256）
    assert_eq!(hash.len(), 64);

    // 相同的 token 应该产生相同的哈希
    let hash2 = hash_token(&token);
    assert_eq!(hash, hash2);

    // 不同的 token 应该产生不同的哈希
    let different_token = generate_test_token();
    let different_hash = hash_token(&different_token);
    assert_ne!(hash, different_hash);
}

/// 测试 token 有效期
#[test]
fn test_token_expiry() {
    // 创建一个过期的 token 模型
    let expired_token = password_reset_tokens::Model {
        id: uuid::Uuid::new_v4(),
        user_id: uuid::Uuid::new_v4(),
        token_hash: "test_hash".to_string(),
        expires_at: Utc::now() - Duration::hours(1),
        used_at: None,
        created_at: Utc::now() - Duration::hours(2),
    };

    assert!(expired_token.is_expired());
    assert!(!expired_token.is_valid());

    // 创建一个有效的 token 模型
    let valid_token = password_reset_tokens::Model {
        id: uuid::Uuid::new_v4(),
        user_id: uuid::Uuid::new_v4(),
        token_hash: "test_hash".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        used_at: None,
        created_at: Utc::now(),
    };

    assert!(!valid_token.is_expired());
    assert!(valid_token.is_valid());
}

/// 测试 token 使用状态
#[test]
fn test_token_usage() {
    // 创建一个已使用的 token 模型
    let used_token = password_reset_tokens::Model {
        id: uuid::Uuid::new_v4(),
        user_id: uuid::Uuid::new_v4(),
        token_hash: "test_hash".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        used_at: Some(Utc::now()),
        created_at: Utc::now(),
    };

    assert!(used_token.is_used());
    assert!(!used_token.is_valid());
}

/// 测试 MockEmailSender
#[test]
fn test_mock_email_sender() {
    let sender = MockEmailSender::new();

    // 发送密码重置邮件
    let result = sender
        .send_password_reset_email("test@example.com", "https://example.com/reset?token=abc123");

    assert!(result.is_ok());

    // 检查发送的邮件
    let emails = sender.get_sent_emails();
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].0, "test@example.com");
    assert!(emails[0].1.contains("token=abc123"));

    // 清空邮件列表
    sender.clear();
    assert_eq!(sender.get_sent_emails().len(), 0);
}

/// 测试密码验证
#[test]
fn test_password_validation() {
    // 有效密码
    assert!(validate_password_strength("password123").is_ok());
    assert!(validate_password_strength("MySecureP@ss").is_ok());
    assert!(validate_password_strength("12345678").is_ok());

    // 无效密码（太短）
    assert!(validate_password_strength("1234567").is_err());
    assert!(validate_password_strength("").is_err());

    // 密码太长
    let long_password = "a".repeat(129);
    assert!(validate_password_strength(&long_password).is_err());
}

// 辅助函数

fn generate_test_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long".to_string());
    }
    if password.len() > 128 {
        return Err("Password must not exceed 128 characters".to_string());
    }
    Ok(())
}
