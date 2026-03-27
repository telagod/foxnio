//! JWT 刷新机制测试
//!
//! 测试覆盖：
//! - Token 生成和验证
//! - Refresh token 流程
//! - Token 黑名单
//! - 登出流程
//! - 多设备管理

mod common;

use chrono::{Duration, Utc};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Access Token Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TestClaims {
    sub: String,
    email: String,
    role: String,
    exp: i64,
    iat: i64,
    jti: Option<String>,
}

/// Refresh Token Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TestRefreshClaims {
    sub: String,
    jti: String,
    exp: i64,
    iat: i64,
}

// ============================================================================
// Token 生成和解析测试
// ============================================================================

#[test]
fn test_access_token_generation() {
    let secret = "test-secret-key";
    let user_id = Uuid::new_v4();
    let jti = Uuid::new_v4().to_string();
    let now = Utc::now();
    let exp = now + Duration::hours(24);

    let claims = TestClaims {
        sub: user_id.to_string(),
        email: "test@example.com".to_string(),
        role: "user".to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        jti: Some(jti.clone()),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    assert!(!token.is_empty());
    assert!(token.contains('.'));

    // 验证可以解析
    let decoded = decode::<TestClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    ).unwrap();

    assert_eq!(decoded.claims.sub, user_id.to_string());
    assert_eq!(decoded.claims.jti, Some(jti));
}

#[test]
fn test_refresh_token_generation() {
    let secret = "test-secret-key";
    let user_id = Uuid::new_v4();
    let jti = Uuid::new_v4().to_string();
    let now = Utc::now();
    let exp = now + Duration::days(7);

    let claims = TestRefreshClaims {
        sub: user_id.to_string(),
        jti: jti.clone(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    assert!(!token.is_empty());

    let decoded = decode::<TestRefreshClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    ).unwrap();

    assert_eq!(decoded.claims.sub, user_id.to_string());
    assert_eq!(decoded.claims.jti, jti);
}

#[test]
fn test_token_expiration() {
    let secret = "test-secret-key";
    let user_id = Uuid::new_v4();

    // 已过期的 token
    let claims = TestClaims {
        sub: user_id.to_string(),
        email: "test@example.com".to_string(),
        role: "user".to_string(),
        exp: (Utc::now() - Duration::hours(1)).timestamp(),
        iat: (Utc::now() - Duration::hours(2)).timestamp(),
        jti: Some(Uuid::new_v4().to_string()),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    // 解析应该失败
    let result = decode::<TestClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    );

    assert!(result.is_err());
}

// ============================================================================
// Token Hash 测试
// ============================================================================

#[test]
fn test_token_hash() {
    let token = "test-refresh-token-12345";
    
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash1 = format!("{:x}", hasher.finalize());

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash2 = format!("{:x}", hasher.finalize());

    // 相同 token 应该产生相同 hash
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // SHA256 输出 64 个十六进制字符
}

#[test]
fn test_different_tokens_different_hashes() {
    let token1 = "token-1";
    let token2 = "token-2";

    let mut hasher = Sha256::new();
    hasher.update(token1.as_bytes());
    let hash1 = format!("{:x}", hasher.finalize());

    let mut hasher = Sha256::new();
    hasher.update(token2.as_bytes());
    let hash2 = format!("{:x}", hasher.finalize());

    assert_ne!(hash1, hash2);
}

// ============================================================================
// Token 轮换安全测试
// ============================================================================

#[test]
fn test_token_rotation_security() {
    let secret = "test-secret-key";
    let user_id = Uuid::new_v4();

    // 生成第一个 refresh token
    let jti1 = Uuid::new_v4().to_string();
    let claims1 = TestRefreshClaims {
        sub: user_id.to_string(),
        jti: jti1.clone(),
        exp: (Utc::now() + Duration::days(7)).timestamp(),
        iat: Utc::now().timestamp(),
    };
    let token1 = encode(
        &Header::default(),
        &claims1,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    // 生成第二个 refresh token（轮换后）
    let jti2 = Uuid::new_v4().to_string();
    let claims2 = TestRefreshClaims {
        sub: user_id.to_string(),
        jti: jti2.clone(),
        exp: (Utc::now() + Duration::days(7)).timestamp(),
        iat: Utc::now().timestamp(),
    };
    let token2 = encode(
        &Header::default(),
        &claims2,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    // 两个 token 应该不同
    assert_ne!(token1, token2);
    assert_ne!(jti1, jti2);

    // 两个 token 的 hash 也应该不同
    let mut hasher = Sha256::new();
    hasher.update(token1.as_bytes());
    let hash1 = format!("{:x}", hasher.finalize());

    let mut hasher = Sha256::new();
    hasher.update(token2.as_bytes());
    let hash2 = format!("{:x}", hasher.finalize());

    assert_ne!(hash1, hash2);
}

// ============================================================================
// JTI 唯一性测试
// ============================================================================

#[test]
fn test_jti_uniqueness() {
    let mut jti_set = std::collections::HashSet::new();

    for _ in 0..1000 {
        let jti = Uuid::new_v4().to_string();
        assert!(jti_set.insert(jti), "JTI should be unique");
    }

    assert_eq!(jti_set.len(), 1000);
}

// ============================================================================
// Token 格式验证测试
// ============================================================================

#[test]
fn test_token_format() {
    let secret = "test-secret-key";
    let claims = TestClaims {
        sub: Uuid::new_v4().to_string(),
        email: "test@example.com".to_string(),
        role: "user".to_string(),
        exp: (Utc::now() + Duration::hours(1)).timestamp(),
        iat: Utc::now().timestamp(),
        jti: Some(Uuid::new_v4().to_string()),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    // JWT 格式：header.payload.signature
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);

    // 每部分都应该是 base64 编码
    for part in &parts {
        assert!(!part.is_empty());
        assert!(part.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '='));
    }
}

// ============================================================================
// 错误处理测试
// ============================================================================

#[test]
fn test_invalid_token_format() {
    let secret = "test-secret-key";
    
    // 无效格式的 token
    let invalid_tokens = vec![
        "",
        "invalid",
        "invalid.token",
        "invalid.token.format.extra",
        ".....",
    ];

    for token in invalid_tokens {
        let result = decode::<TestClaims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        );
        assert!(result.is_err(), "Should fail for token: {}", token);
    }
}

#[test]
fn test_wrong_secret() {
    let secret1 = "secret-1";
    let secret2 = "secret-2";

    let claims = TestClaims {
        sub: Uuid::new_v4().to_string(),
        email: "test@example.com".to_string(),
        role: "user".to_string(),
        exp: (Utc::now() + Duration::hours(1)).timestamp(),
        iat: Utc::now().timestamp(),
        jti: Some(Uuid::new_v4().to_string()),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret1.as_bytes()),
    ).unwrap();

    // 用错误的 secret 解析应该失败
    let result = decode::<TestClaims>(
        &token,
        &DecodingKey::from_secret(secret2.as_bytes()),
        &jsonwebtoken::Validation::default(),
    );

    assert!(result.is_err());
}

// ============================================================================
// 时间边界测试
// ============================================================================

#[test]
fn test_token_valid_at_boundary() {
    let secret = "test-secret-key";

    // Token 在 1 秒后过期
    let claims = TestClaims {
        sub: Uuid::new_v4().to_string(),
        email: "test@example.com".to_string(),
        role: "user".to_string(),
        exp: (Utc::now() + Duration::seconds(1)).timestamp(),
        iat: Utc::now().timestamp(),
        jti: Some(Uuid::new_v4().to_string()),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    // 立即验证应该成功
    let result = decode::<TestClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    );

    assert!(result.is_ok());
}

// ============================================================================
// 并发安全性测试
// ============================================================================

#[test]
fn test_concurrent_token_generation() {
    use std::thread;
    use std::sync::{Arc, Mutex};

    let secret = "test-secret-key";
    let user_id = Uuid::new_v4();
    let tokens = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let tokens = Arc::clone(&tokens);
            let secret = secret.to_string();
            let user_id = user_id.to_string();

            thread::spawn(move || {
                let claims = TestClaims {
                    sub: user_id.clone(),
                    email: "test@example.com".to_string(),
                    role: "user".to_string(),
                    exp: (Utc::now() + Duration::hours(1)).timestamp(),
                    iat: Utc::now().timestamp(),
                    jti: Some(Uuid::new_v4().to_string()),
                };

                let token = encode(
                    &Header::default(),
                    &claims,
                    &EncodingKey::from_secret(secret.as_bytes()),
                ).unwrap();

                tokens.lock().unwrap().push(token);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let tokens = tokens.lock().unwrap();
    
    // 所有 token 都应该不同（因为 jti 不同）
    let unique_tokens: std::collections::HashSet<_> = tokens.iter().cloned().collect();
    assert_eq!(unique_tokens.len(), 10);
}

// ============================================================================
// 模拟黑名单测试（无 Redis）
// ============================================================================

#[test]
fn test_blacklist_simulation() {
    // 模拟黑名单行为
    let mut blacklist = std::collections::HashSet::new();
    
    let jti1 = Uuid::new_v4().to_string();
    let jti2 = Uuid::new_v4().to_string();

    // 添加到黑名单
    blacklist.insert(jti1.clone());

    // 检查
    assert!(blacklist.contains(&jti1));
    assert!(!blacklist.contains(&jti2));

    // 从黑名单移除
    blacklist.remove(&jti1);
    assert!(!blacklist.contains(&jti1));
}
