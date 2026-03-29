//! Sora 网关服务 - Sora Gateway Service
//!
//! 处理 OpenAI Sora 视频生成 API 的网关服务

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Sora 请求上下文
#[derive(Debug, Clone)]
pub struct SoraRequestContext {
    pub request_id: String,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Sora 生成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraGenerationRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub duration: Option<i32>,
    pub aspect_ratio: Option<String>,
    pub resolution: Option<String>,
}

/// Sora 生成响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraGenerationResponse {
    pub id: String,
    pub status: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub video_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub error: Option<String>,
}

/// Sora 网关配置
#[derive(Debug, Clone)]
pub struct SoraGatewayConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub max_wait_secs: u64,
    pub poll_interval_secs: u64,
}

impl Default for SoraGatewayConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 120,
            max_wait_secs: 600,
            poll_interval_secs: 5,
        }
    }
}

/// Sora 网关服务
pub struct SoraGatewayService {
    db: sea_orm::DatabaseConnection,
    config: SoraGatewayConfig,
    http_client: reqwest::Client,
}

impl SoraGatewayService {
    /// 创建新的网关服务
    pub fn new(db: sea_orm::DatabaseConnection, config: SoraGatewayConfig) -> Self {
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

    /// 提交视频生成请求
    pub async fn submit_generation(
        &self,
        _ctx: &SoraRequestContext,
        api_key: &str,
        request: &SoraGenerationRequest,
    ) -> Result<SoraGenerationResponse> {
        let url = format!("{}/video/generations", self.config.base_url);

        let body = serde_json::to_value(request)?;

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_body = response.json::<serde_json::Value>().await?;

        if !status.is_success() {
            let error = response_body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return Ok(SoraGenerationResponse {
                id: String::new(),
                status: "failed".to_string(),
                model: request.model.clone().unwrap_or_default(),
                created_at: Utc::now(),
                video_url: None,
                thumbnail_url: None,
                error: Some(error),
            });
        }

        Ok(SoraGenerationResponse {
            id: response_body
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            status: response_body
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending")
                .to_string(),
            model: response_body
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            created_at: Utc::now(),
            video_url: None,
            thumbnail_url: None,
            error: None,
        })
    }

    /// 查询生成状态
    pub async fn get_generation_status(
        &self,
        api_key: &str,
        generation_id: &str,
    ) -> Result<SoraGenerationResponse> {
        let url = format!(
            "{}/video/generations/{}",
            self.config.base_url, generation_id
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        let response_body = response.json::<serde_json::Value>().await?;

        Ok(SoraGenerationResponse {
            id: generation_id.to_string(),
            status: response_body
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            model: response_body
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            created_at: response_body
                .get("created")
                .and_then(|v| v.as_i64())
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_default())
                .unwrap_or_else(Utc::now),
            video_url: response_body
                .get("video_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            thumbnail_url: response_body
                .get("thumbnail_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            error: response_body
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }

    /// 等待生成完成
    pub async fn wait_for_completion(
        &self,
        _ctx: &SoraRequestContext,
        api_key: &str,
        generation_id: &str,
    ) -> Result<SoraGenerationResponse> {
        let start_time = std::time::Instant::now();
        let max_wait = std::time::Duration::from_secs(self.config.max_wait_secs);
        let poll_interval = std::time::Duration::from_secs(self.config.poll_interval_secs);

        loop {
            let response = self.get_generation_status(api_key, generation_id).await?;

            match response.status.as_str() {
                "completed" | "succeeded" => {
                    return Ok(response);
                }
                "failed" | "cancelled" => {
                    return Ok(response);
                }
                _ => {
                    // 继续等待
                    if start_time.elapsed() >= max_wait {
                        return Ok(SoraGenerationResponse {
                            id: generation_id.to_string(),
                            status: "timeout".to_string(),
                            model: String::new(),
                            created_at: Utc::now(),
                            video_url: None,
                            thumbnail_url: None,
                            error: Some("Generation timed out".to_string()),
                        });
                    }

                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// 取消生成
    pub async fn cancel_generation(&self, api_key: &str, generation_id: &str) -> Result<bool> {
        let url = format!(
            "{}/video/generations/{}/cancel",
            self.config.base_url, generation_id
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sora_gateway_config() {
        let config = SoraGatewayConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.max_wait_secs, 600);
    }

    #[test]
    fn test_sora_generation_request() {
        let request = SoraGenerationRequest {
            prompt: "A cat playing piano".to_string(),
            model: Some("sora-1.0-turbo".to_string()),
            duration: Some(10),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("1080p".to_string()),
        };

        assert_eq!(request.prompt, "A cat playing piano");
        assert_eq!(request.model, Some("sora-1.0-turbo".to_string()));
    }
}
