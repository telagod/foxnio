//! OpenAI 网关服务 - OpenAI Gateway Service
//!
//! 处理 OpenAI API 请求的网关服务

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OpenAI 请求上下文
#[derive(Debug, Clone)]
pub struct OpenAIRequestContext {
    pub request_id: String,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub model: String,
    pub stream: bool,
    pub created_at: DateTime<Utc>,
}

/// OpenAI 响应信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIResponseInfo {
    pub request_id: String,
    pub status_code: u16,
    pub model: String,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub finish_reason: Option<String>,
    pub response_time_ms: i64,
    pub created_at: DateTime<Utc>,
}

/// OpenAI 网关配置
#[derive(Debug, Clone)]
pub struct OpenAIGatewayConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub enable_sticky_session: bool,
    pub sticky_session_ttl_secs: u64,
}

impl Default for OpenAIGatewayConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 120,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_sticky_session: true,
            sticky_session_ttl_secs: 3600,
        }
    }
}

/// OpenAI 网关服务
pub struct OpenAIGatewayService {
    db: sea_orm::DatabaseConnection,
    config: OpenAIGatewayConfig,
    http_client: reqwest::Client,
}

impl OpenAIGatewayService {
    /// 创建新的 OpenAI 网关服务
    pub fn new(db: sea_orm::DatabaseConnection, config: OpenAIGatewayConfig) -> Self {
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

    /// 发送 Chat Completions 请求
    pub async fn send_chat_completions(
        &self,
        ctx: &OpenAIRequestContext,
        api_key: &str,
        request_body: serde_json::Value,
    ) -> Result<OpenAIResponseInfo> {
        let start_time = std::time::Instant::now();

        // 构建请求
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let response_body = response.json::<serde_json::Value>().await?;

        // 解析响应
        let model = request_body["model"]
            .as_str()
            .unwrap_or(&ctx.model)
            .to_string();
        let usage = response_body.get("usage");

        let prompt_tokens = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_i64());
        let completion_tokens = usage
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|v| v.as_i64());
        let total_tokens = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(|v| v.as_i64());

        let finish_reason = response_body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("finish_reason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(OpenAIResponseInfo {
            request_id: ctx.request_id.clone(),
            status_code,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            finish_reason,
            response_time_ms: start_time.elapsed().as_millis() as i64,
            created_at: Utc::now(),
        })
    }

    /// 发送 Embeddings 请求
    pub async fn send_embeddings(
        &self,
        ctx: &OpenAIRequestContext,
        api_key: &str,
        request_body: serde_json::Value,
    ) -> Result<OpenAIResponseInfo> {
        let start_time = std::time::Instant::now();

        let url = format!("{}/embeddings", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let response_body = response.json::<serde_json::Value>().await?;

        let model = request_body["model"]
            .as_str()
            .unwrap_or(&ctx.model)
            .to_string();
        let usage = response_body.get("usage");

        let prompt_tokens = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_i64());
        let total_tokens = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(|v| v.as_i64());

        Ok(OpenAIResponseInfo {
            request_id: ctx.request_id.clone(),
            status_code,
            model,
            prompt_tokens,
            completion_tokens: None,
            total_tokens,
            finish_reason: None,
            response_time_ms: start_time.elapsed().as_millis() as i64,
            created_at: Utc::now(),
        })
    }

    /// 检查 API 连接
    pub async fn check_connection(&self, api_key: &str) -> Result<bool> {
        let url = format!("{}/models", self.config.base_url);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// 获取可用模型列表
    pub async fn list_models(&self, api_key: &str) -> Result<Vec<String>> {
        let url = format!("{}/models", self.config.base_url);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;

        let models = body
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config() {
        let config = OpenAIGatewayConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.timeout_secs, 120);
    }
}
