//! 测试辅助函数
//!
//! 提供常用的测试工具和辅助函数

#![allow(dead_code)]

use serde_json::json;
use uuid::Uuid;

/// 创建测试用户 ID
pub fn test_user_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
}

/// 创建测试 API Key ID
pub fn test_api_key_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
}

/// 创建测试账号 ID
pub fn test_account_id() -> Uuid {
    Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap()
}

/// 创建测试 email
pub fn test_email() -> String {
    "test@example.com".to_string()
}

/// 创建测试密码
pub fn test_password() -> String {
    "TestPassword123!@#".to_string()
}

/// 创建测试 JWT Claims
pub fn test_claims() -> serde_json::Value {
    json!({
        "sub": test_user_id().to_string(),
        "email": test_email(),
        "role": "user",
        "exp": 9999999999_i64,
        "iat": 1000000000_i64
    })
}

/// 创建测试管理员 Claims
pub fn test_admin_claims() -> serde_json::Value {
    json!({
        "sub": test_user_id().to_string(),
        "email": "admin@example.com",
        "role": "admin",
        "exp": 9999999999_i64,
        "iat": 1000000000_i64
    })
}

/// 创建测试账号数据
pub fn test_account() -> serde_json::Value {
    json!({
        "id": test_account_id().to_string(),
        "name": "test-account",
        "provider": "openai",
        "credential_type": "api_key",
        "credential": "sk-test-key-123456",
        "status": "active",
        "priority": 1,
        "concurrent_limit": 10,
        "rate_limit_rpm": 1000
    })
}

/// 创建测试模型请求
pub fn test_chat_request() -> serde_json::Value {
    json!({
        "model": "gpt-4",
        "messages": [
            {"role": "user", "content": "Hello, world!"}
        ],
        "temperature": 0.7,
        "max_tokens": 100
    })
}

/// 创建测试流式请求
pub fn test_stream_request() -> serde_json::Value {
    json!({
        "model": "gpt-4",
        "messages": [
            {"role": "user", "content": "Hello, world!"}
        ],
        "stream": true
    })
}

/// 等待条件满足或超时
pub async fn wait_for<F, Fut>(condition: F, timeout_ms: u64)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout {
        if condition().await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!("Timeout waiting for condition");
}

/// 生成随机字符串
pub fn random_string(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// 生成随机 email
pub fn random_email() -> String {
    format!("test-{}@example.com", random_string(8))
}

/// 断言响应包含特定字段
#[macro_export]
macro_rules! assert_json_has_field {
    ($json:expr, $field:expr) => {
        assert!(
            $json.get($field).is_some(),
            "JSON does not contain field: {}",
            $field
        );
    };
}

/// 断言响应状态码
#[macro_export]
macro_rules! assert_status {
    ($response:expr, $status:expr) => {
        assert_eq!(
            $response.status(),
            $status,
            "Expected status {} but got {}",
            $status,
            $response.status()
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_string() {
        let s1 = random_string(10);
        let s2 = random_string(10);
        assert_eq!(s1.len(), 10);
        assert_eq!(s2.len(), 10);
        assert_ne!(s1, s2); // 几乎不可能相等
    }

    #[test]
    fn test_random_email() {
        let email = random_email();
        assert!(email.contains('@'));
        assert!(email.contains("test-"));
        assert!(email.ends_with("@example.com"));
    }

    #[test]
    fn test_test_helpers() {
        let user_id = test_user_id();
        assert!(!user_id.is_nil());

        let email = test_email();
        assert!(email.contains('@'));

        let password = test_password();
        assert!(password.len() >= 8);
    }

    #[tokio::test]
    async fn test_wait_for_success() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        // 在另一个任务中增加计数器
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        // 等待条件满足
        wait_for(
            || {
                let c = counter.clone();
                async move { c.load(std::sync::atomic::Ordering::SeqCst) > 0 }
            },
            1000,
        )
        .await;

        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }
}
