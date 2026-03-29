//! Antigravity 网关服务 - Antigravity Gateway Service
//!
//! 处理 Anthropic/Claude API 请求的网关服务

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Antigravity 请求上下文
#[derive(Debug, Clone)]
pub struct AntigravityRequestContext {
    pub request_id: String,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub model: String,
    pub stream: bool,
    pub created_at: DateTime<Utc>,
}

/// Antigravity 响应信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityResponseInfo {
    pub request_id: String,
    pub status_code: u16,
    pub model: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub stop_reason: Option<String>,
    pub response_time_ms: i64,
    pub created_at: DateTime<Utc>,
}

/// Antigravity 网关配置
#[derive(Debug, Clone)]
pub struct AntigravityGatewayConfig {
    pub base_url: String,
    pub api_version: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for AntigravityGatewayConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.anthropic.com/v1".to_string(),
            api_version: "2023-06-01".to_string(),
            timeout_secs: 120,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Antigravity 网关服务
pub struct AntigravityGatewayService {
    db: sea_orm::DatabaseConnection,
    config: AntigravityGatewayConfig,
    http_client: reqwest::Client,
}

impl AntigravityGatewayService {
    /// 创建新的网关服务
    pub fn new(db: sea_orm::DatabaseConnection, config: AntigravityGatewayConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap();

        Self {
            db,
            config,
            http_client,
        }
    }

    /// 发送 Messages 请求
    pub async fn send_messages(
        &self,
        ctx: &AntigravityRequestContext,
        api_key: &str,
        request_body: serde_json::Value,
    ) -> Result<AntigravityResponseInfo> {
        let start_time = std::time::Instant::now();

        let url = format!("{}/messages", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", &self.config.api_version)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let response_body = response.json::<serde_json::Value>().await?;

        // 解析响应
        let model = response_body
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&ctx.model)
            .to_string();

        let usage = response_body.get("usage");
        let input_tokens = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_i64());
        let output_tokens = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_i64());

        let stop_reason = response_body
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(AntigravityResponseInfo {
            request_id: ctx.request_id.clone(),
            status_code,
            model,
            input_tokens,
            output_tokens,
            stop_reason,
            response_time_ms: start_time.elapsed().as_millis() as i64,
            created_at: Utc::now(),
        })
    }

    /// 检查 API 连接
    pub async fn check_connection(&self, api_key: &str) -> Result<bool> {
        // Anthropic 没有专门的检查端点，发送一个简单请求测试
        let url = format!("{}/messages", self.config.base_url);

        let test_body = serde_json::json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "Hi"}]
        });

        let response = self
            .http_client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", &self.config.api_version)
            .json(&test_body)
            .send()
            .await?;

        Ok(response.status().as_u16() != 401)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config() {
        let config = AntigravityGatewayConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com/v1");
        assert_eq!(config.api_version, "2023-06-01");
    }
}
