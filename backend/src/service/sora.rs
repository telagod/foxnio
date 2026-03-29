//! Sora 平台支持（视频生成）
//!
//! 实现 Sora API 客户端、视频生成、任务状态查询等功能

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sora API 配置
#[derive(Debug, Clone)]
pub struct SoraConfig {
    /// API Key
    pub api_key: String,
    /// API Base URL
    pub base_url: String,
    /// 媒体存储 URL
    pub media_url: String,
    /// 超时时间（秒）
    pub timeout_secs: u64,
}

impl SoraConfig {
    /// 创建新的配置
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            media_url: "https://api.openai.com/v1/media".to_string(),
            timeout_secs: 300,
        }
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("SORA_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .context("SORA_API_KEY or OPENAI_API_KEY environment variable not set")?;

        let base_url = std::env::var("SORA_BASE_URL")
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let media_url =
            std::env::var("SORA_MEDIA_URL").unwrap_or_else(|_| format!("{}/media", base_url));

        Ok(Self {
            api_key,
            base_url,
            media_url,
            timeout_secs: 300,
        })
    }

    /// 设置 Base URL
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// 设置媒体 URL
    pub fn with_media_url(mut self, url: String) -> Self {
        self.media_url = url;
        self
    }
}

/// Sora 视频生成请求
#[derive(Debug, Clone, Serialize)]
pub struct SoraGenerateRequest {
    /// 提示词
    pub prompt: String,
    /// 模型名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 视频时长（秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// 分辨率
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// 风格
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// 参考图片 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_image: Option<String>,
}

/// Sora 视频生成响应
#[derive(Debug, Clone, Deserialize)]
pub struct SoraGenerateResponse {
    /// 任务 ID
    pub id: String,
    /// 任务状态
    pub status: SoraTaskStatus,
    /// 创建时间
    #[serde(rename = "created_at")]
    pub created_at: DateTime<Utc>,
    /// 完成时间
    #[serde(rename = "finished_at", default)]
    pub finished_at: Option<DateTime<Utc>>,
    /// 视频信息
    pub video: Option<SoraVideoInfo>,
    /// 错误信息
    pub error: Option<SoraError>,
}

/// Sora 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoraTaskStatus {
    /// 排队中
    Queued,
    /// 处理中
    Processing,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// Sora 视频信息
#[derive(Debug, Clone, Deserialize)]
pub struct SoraVideoInfo {
    /// 视频 URL
    pub url: String,
    /// 缩略图 URL
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// 时长（秒）
    #[serde(default)]
    pub duration: Option<u32>,
    /// 分辨率
    #[serde(default)]
    pub resolution: Option<String>,
    /// 文件大小（字节）
    #[serde(default)]
    pub size: Option<u64>,
}

/// Sora 错误信息
#[derive(Debug, Clone, Deserialize)]
pub struct SoraError {
    /// 错误类型
    #[serde(rename = "type")]
    pub error_type: String,
    /// 错误消息
    pub message: String,
    /// 错误代码
    #[serde(default)]
    pub code: Option<String>,
}

/// Sora 任务状态查询响应
#[derive(Debug, Clone, Deserialize)]
pub struct SoraTaskStatusResponse {
    /// 任务 ID
    pub id: String,
    /// 任务状态
    pub status: SoraTaskStatus,
    /// 进度百分比 (0-100)
    #[serde(default)]
    pub progress: Option<u32>,
    /// 视频信息
    pub video: Option<SoraVideoInfo>,
    /// 错误信息
    pub error: Option<SoraError>,
}

/// Sora 媒体上传响应
#[derive(Debug, Clone, Deserialize)]
pub struct SoraMediaUploadResponse {
    /// 媒体 ID
    pub id: String,
    /// 上传 URL
    #[serde(rename = "upload_url")]
    pub upload_url: String,
    /// 媒体 URL
    #[serde(rename = "media_url")]
    pub media_url: String,
}

/// Sora API 客户端
pub struct SoraClient {
    config: SoraConfig,
    http_client: Client,
}

impl SoraClient {
    /// 创建新的客户端
    pub fn new(config: SoraConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// 从环境变量创建客户端
    pub fn from_env() -> Result<Self> {
        let config = SoraConfig::from_env()?;
        Self::new(config)
    }

    /// 生成视频
    pub async fn generate_video(
        &self,
        request: SoraGenerateRequest,
    ) -> Result<SoraGenerateResponse> {
        let url = format!("{}/videos/generations", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send video generation request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Sora video generation failed: {} - {}", status, body);
        }

        let result: SoraGenerateResponse = response
            .json()
            .await
            .context("Failed to parse video generation response")?;

        Ok(result)
    }

    /// 查询任务状态
    pub async fn get_task_status(&self, task_id: &str) -> Result<SoraTaskStatusResponse> {
        let url = format!("{}/videos/generations/{}", self.config.base_url, task_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to get task status")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to get task status: {} - {}", status, body);
        }

        let result: SoraTaskStatusResponse = response
            .json()
            .await
            .context("Failed to parse task status response")?;

        Ok(result)
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        let url = format!(
            "{}/videos/generations/{}/cancel",
            self.config.base_url, task_id
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to cancel task")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to cancel task: {} - {}", status, body);
        }

        Ok(())
    }

    /// 列出任务
    pub async fn list_tasks(
        &self,
        limit: Option<u32>,
        after: Option<&str>,
    ) -> Result<Vec<SoraGenerateResponse>> {
        let mut url = format!("{}/videos/generations", self.config.base_url);
        let mut params = Vec::new();

        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }

        if let Some(after) = after {
            params.push(format!("after={}", after));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to list tasks")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to list tasks: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct ListResponse {
            data: Vec<SoraGenerateResponse>,
        }

        let result: ListResponse = response
            .json()
            .await
            .context("Failed to parse task list response")?;

        Ok(result.data)
    }

    /// 上传媒体文件
    pub async fn upload_media(&self, file_path: &str) -> Result<SoraMediaUploadResponse> {
        let url = format!("{}/uploads", self.config.media_url);

        // 读取文件
        let file_content = std::fs::read(file_path).context("Failed to read media file")?;

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/octet-stream")
            .body(file_content)
            .send()
            .await
            .context("Failed to upload media")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to upload media: {} - {}", status, body);
        }

        let result: SoraMediaUploadResponse = response
            .json()
            .await
            .context("Failed to parse media upload response")?;

        Ok(result)
    }

    /// 获取签名 URL（用于访问视频）
    pub async fn get_signed_url(&self, media_id: &str) -> Result<String> {
        let url = format!("{}/media/{}/signed-url", self.config.base_url, media_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .context("Failed to get signed URL")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to get signed URL: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct SignedUrlResponse {
            url: String,
        }

        let result: SignedUrlResponse = response
            .json()
            .await
            .context("Failed to parse signed URL response")?;

        Ok(result.url)
    }

    /// 等待任务完成
    pub async fn wait_for_completion(
        &self,
        task_id: &str,
        poll_interval_secs: u64,
        max_wait_secs: u64,
    ) -> Result<SoraGenerateResponse> {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_secs(poll_interval_secs);
        let max_wait = std::time::Duration::from_secs(max_wait_secs);

        loop {
            if start.elapsed() > max_wait {
                bail!(
                    "Task {} did not complete within {} seconds",
                    task_id,
                    max_wait_secs
                );
            }

            let status = self.get_task_status(task_id).await?;

            match status.status {
                SoraTaskStatus::Completed => {
                    // 获取完整响应
                    let url = format!("{}/videos/generations/{}", self.config.base_url, task_id);
                    let response = self
                        .http_client
                        .get(&url)
                        .header("Authorization", format!("Bearer {}", self.config.api_key))
                        .send()
                        .await?;

                    return response
                        .json()
                        .await
                        .context("Failed to parse final response");
                }
                SoraTaskStatus::Failed => {
                    bail!(
                        "Task {} failed: {}",
                        task_id,
                        status.error.map(|e| e.message).unwrap_or_default()
                    );
                }
                SoraTaskStatus::Cancelled => {
                    bail!("Task {} was cancelled", task_id);
                }
                _ => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
}

/// Sora 定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraPricing {
    /// 模型名称
    pub model: String,
    /// 每秒价格
    pub price_per_second: f64,
    /// 币种
    pub currency: String,
}

/// Sora 定价表
pub fn get_pricing_table() -> HashMap<&'static str, SoraPricing> {
    let mut table = HashMap::new();

    table.insert(
        "sora-1.0",
        SoraPricing {
            model: "sora-1.0".to_string(),
            price_per_second: 0.05,
            currency: "USD".to_string(),
        },
    );

    table
}

/// 计算视频生成费用
pub fn calculate_cost(duration_secs: u32, model: &str) -> f64 {
    let pricing_table = get_pricing_table();
    let pricing = pricing_table
        .get(model)
        .unwrap_or_else(|| pricing_table.get("sora-1.0").unwrap());

    pricing.price_per_second * duration_secs as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = SoraConfig::new("test_key".to_string());

        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn test_task_status_serialization() {
        let status = SoraTaskStatus::Processing;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"processing\"");

        let parsed: SoraTaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SoraTaskStatus::Processing);
    }

    #[test]
    fn test_generate_request() {
        let request = SoraGenerateRequest {
            prompt: "A cat playing piano".to_string(),
            model: Some("sora-1.0".to_string()),
            duration: Some(10),
            resolution: Some("1080p".to_string()),
            style: None,
            reference_image: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"prompt\":\"A cat playing piano\""));
    }

    #[test]
    fn test_pricing_table() {
        let table = get_pricing_table();

        assert!(table.contains_key("sora-1.0"));
        let pricing = table.get("sora-1.0").unwrap();
        assert_eq!(pricing.price_per_second, 0.05);
    }

    #[test]
    fn test_calculate_cost() {
        let cost = calculate_cost(10, "sora-1.0");
        assert_eq!(cost, 0.5); // 10 seconds * $0.05/sec
    }
}
