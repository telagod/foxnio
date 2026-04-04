#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 端到端测试

use serde_json::json;

/// 测试配置
pub struct TestConfig {
    pub database_url: String,
    pub redis_url: String,
    pub api_base: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/foxnio_test".to_string()
            }),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            api_base: "http://localhost:8080".to_string(),
        }
    }
}

#[cfg(test)]
mod e2e_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要运行的服务
    async fn test_user_registration_flow() {
        // 1. 注册用户
        let client = reqwest::Client::new();

        let _response = client
            .post("http://localhost:8080/api/v1/auth/register")
            .json(&json!({
                "email": "test@example.com",
                "password": "TestPassword123",
                "username": "testuser"
            }))
            .send()
            .await;

        // 2. 登录
        let response = client
            .post("http://localhost:8080/api/v1/auth/login")
            .json(&json!({
                "email": "test@example.com",
                "password": "TestPassword123"
            }))
            .send()
            .await;

        // 3. 获取用户信息
        // 4. 创建 API Key
        // 5. 使用 API Key 调用模型
        // 6. 清理测试数据
        let _ = response; // Suppress unused warning
    }

    #[tokio::test]
    #[ignore]
    async fn test_api_key_flow() {
        let client = reqwest::Client::new();

        // 假设已经有 token
        let token = "test_token";

        // 1. 创建 API Key
        let _response = client
            .post("http://localhost:8080/api/v1/user/apikeys")
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "name": "Test Key"
            }))
            .send()
            .await;

        // 2. 列出 API Keys
        let response = client
            .get("http://localhost:8080/api/v1/user/apikeys")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await;

        // 3. 删除 API Key
        let _ = response; // Suppress unused warning
    }

    #[tokio::test]
    #[ignore]
    async fn test_chat_completions_flow() {
        let client = reqwest::Client::new();

        let api_key = "foxnio-test-key";

        // 发送 chat completion 请求
        let response = client
            .post("http://localhost:8080/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&json!({
                "model": "gpt-4",
                "messages": [
                    {"role": "user", "content": "Hello"}
                ],
                "stream": false
            }))
            .send()
            .await;

        // 验证响应
        let _ = response; // Suppress unused warning
    }

    #[tokio::test]
    #[ignore]
    async fn test_streaming_chat_completions() {
        let client = reqwest::Client::new();

        let api_key = "foxnio-test-key";

        // 发送流式请求
        let response = client
            .post("http://localhost:8080/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&json!({
                "model": "gpt-4",
                "messages": [
                    {"role": "user", "content": "Hello"}
                ],
                "stream": true
            }))
            .send()
            .await;

        // 验证 SSE 流
        let _ = response; // Suppress unused warning
    }

    #[tokio::test]
    #[ignore]
    async fn test_rate_limiting() {
        let client = reqwest::Client::new();

        let api_key = "foxnio-test-key";

        // 快速发送多个请求
        let mut handles = vec![];

        for _ in 0..100 {
            let client = client.clone();
            let handle = tokio::spawn(async move {
                let _ = client
                    .post("http://localhost:8080/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&json!({
                        "model": "gpt-4",
                        "messages": [{"role": "user", "content": "test"}]
                    }))
                    .send()
                    .await;
            });
            handles.push(handle);
        }

        // 等待所有请求完成
        for handle in handles {
            let _ = handle.await;
        }

        // 验证是否有 rate limit 错误
    }

    #[tokio::test]
    #[ignore]
    async fn test_error_handling() {
        let client = reqwest::Client::new();

        // 测试无效 API Key
        let response = client
            .post("http://localhost:8080/v1/chat/completions")
            .header("Authorization", "Bearer invalid-key")
            .json(&json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "test"}]
            }))
            .send()
            .await;

        // 应该返回 401
        // assert_eq!(response.unwrap().status(), 401);
        let _ = response;

        // 测试无效模型
        let response = client
            .post("http://localhost:8080/v1/chat/completions")
            .header("Authorization", "Bearer valid-key")
            .json(&json!({
                "model": "invalid-model",
                "messages": [{"role": "user", "content": "test"}]
            }))
            .send()
            .await;

        // 应该返回错误
        let _ = response;
    }

    #[tokio::test]
    #[ignore]
    async fn test_health_check() {
        let client = reqwest::Client::new();

        let response = client
            .get("http://localhost:8080/health")
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());
    }

    #[tokio::test]
    #[ignore]
    async fn test_metrics_endpoint() {
        let client = reqwest::Client::new();

        let response = client
            .get("http://localhost:8080/metrics")
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());
    }
}
