//! Gemini API 客户端
//!
//! 提供与 Google Gemini API 的通信能力

use anyhow::{bail, Result};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::Client;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, warn};

use super::types::*;

/// Gemini API 基础 URL
pub const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";
pub const GEMINI_CLI_BASE_URL: &str = "https://cloudcode-pa.googleapis.com";

/// Gemini 客户端配置
#[derive(Debug, Clone)]
pub struct GeminiClientConfig {
    /// 基础 URL
    pub base_url: String,
    /// 请求超时
    pub timeout: Duration,
    /// 最大重试次数
    pub max_retries: u32,
}

impl Default for GeminiClientConfig {
    fn default() -> Self {
        Self {
            base_url: GEMINI_API_BASE_URL.to_string(),
            timeout: Duration::from_secs(300),
            max_retries: 3,
        }
    }
}

/// Gemini API 客户端
#[derive(Debug, Clone)]
pub struct GeminiClient {
    http_client: Client,
    config: GeminiClientConfig,
}

impl GeminiClient {
    /// 创建新的 Gemini 客户端
    pub fn new(config: GeminiClientConfig) -> Self {
        let http_client = Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(100)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client,
            config,
        }
    }

    /// 使用默认配置创建客户端
    pub fn with_defaults() -> Self {
        Self::new(GeminiClientConfig::default())
    }

    /// 获取基础 URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// 列出可用模型
    pub async fn list_models(&self, api_key: &str) -> Result<GeminiModelsListResponse> {
        let url = format!("{}/v1beta/models?key={}", self.config.base_url, api_key);

        let response = self
            .http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to list models: {} - {}", status, body);
        }

        let models = response.json().await?;
        Ok(models)
    }

    /// 获取单个模型信息
    pub async fn get_model(&self, model: &str, api_key: &str) -> Result<GeminiModel> {
        let url = format!(
            "{}/v1beta/models/{}?key={}",
            self.config.base_url, model, api_key
        );

        let response = self
            .http_client
            .get(&url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to get model: {} - {}", status, body);
        }

        let model = response.json().await?;
        Ok(model)
    }

    /// 生成内容（非流式）
    pub async fn generate_content(
        &self,
        model: &str,
        request: &GenerateContentRequest,
        api_key: &str,
    ) -> Result<GenerateContentResponse> {
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.config.base_url, model, api_key
        );

        debug!("Sending generateContent request to model: {}", model);

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!("Gemini API error: {} - {}", status, body);
            bail!("Gemini API error: {} - {}", status, body);
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// 生成内容（流式）
    pub async fn stream_generate_content(
        &self,
        model: &str,
        request: &GenerateContentRequest,
        api_key: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>> {
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.config.base_url, model, api_key
        );

        debug!("Sending streamGenerateContent request to model: {}", model);

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!("Gemini streaming API error: {} - {}", status, body);
            bail!("Gemini streaming API error: {} - {}", status, body);
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(stream.map(|r| {
            r.map_err(|e| anyhow::anyhow!("Stream error: {}", e))
        })))
    }

    /// 计算 Token 数量
    pub async fn count_tokens(
        &self,
        model: &str,
        request: &GenerateContentRequest,
        api_key: &str,
    ) -> Result<TokenCountResponse> {
        let url = format!(
            "{}/v1beta/models/{}:countTokens?key={}",
            self.config.base_url, model, api_key
        );

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("Token count error: {} - {}", status, body);
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// 嵌入内容
    pub async fn embed_content(
        &self,
        model: &str,
        request: &EmbedContentRequest,
        api_key: &str,
    ) -> Result<EmbedContentResponse> {
        let url = format!(
            "{}/v1beta/models/{}:embedContent?key={}",
            self.config.base_url, model, api_key
        );

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("Embed content error: {} - {}", status, body);
        }

        let result = response.json().await?;
        Ok(result)
    }
}

/// Token 计数响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenCountResponse {
    pub total_tokens: i32,
}

/// 嵌入请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbedContentRequest {
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimensionality: Option<i32>,
}

/// 嵌入响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbedContentResponse {
    pub embedding: ContentEmbedding,
}

/// 内容嵌入
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentEmbedding {
    pub values: Vec<f32>,
}

/// 解析模型动作
///
/// 将路径如 "gemini-2.0-flash:generateContent" 解析为 (model, action)
pub fn parse_model_action(path: &str) -> Result<(String, String)> {
    let path = path.trim();
    if path.is_empty() {
        bail!("Empty model action path");
    }

    // 标准格式: {model}:{action}
    if let Some(pos) = path.find(':') {
        if pos > 0 && pos < path.len() - 1 {
            return Ok((path[..pos].to_string(), path[pos + 1..].to_string()));
        }
    }

    // 回退格式: {model}/{action}
    if let Some(pos) = path.find('/') {
        if pos > 0 && pos < path.len() - 1 {
            return Ok((path[..pos].to_string(), path[pos + 1..].to_string()));
        }
    }

    bail!("Invalid model action path: {}", path);
}

/// 规范化模型名称
///
/// 将 "gemini-2.0-flash" 转换为 "models/gemini-2.0-flash"
pub fn normalize_model_name(model: &str) -> String {
    if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    }
}

/// 提取纯模型名称
///
/// 将 "models/gemini-2.0-flash" 转换为 "gemini-2.0-flash"
pub fn extract_model_name(model: &str) -> &str {
    model.strip_prefix("models/").unwrap_or(model)
}

/// 构建 Gemini 错误响应
pub fn build_error_response(code: i32, message: &str, status: &str) -> GeminiErrorResponse {
    GeminiErrorResponse {
        error: GeminiError {
            code,
            message: message.to_string(),
            status: status.to_string(),
            details: None,
        },
    }
}

/// HTTP 状态码转 Gemini 状态字符串
pub fn http_status_to_gemini_status(status: u16) -> &'static str {
    match status {
        200..=299 => "OK",
        400 => "INVALID_ARGUMENT",
        401 => "UNAUTHENTICATED",
        403 => "PERMISSION_DENIED",
        404 => "NOT_FOUND",
        409 => "ALREADY_EXISTS",
        429 => "RESOURCE_EXHAUSTED",
        499 => "CANCELLED",
        500 => "INTERNAL",
        501 => "NOT_IMPLEMENTED",
        502 | 503 => "UNAVAILABLE",
        504 => "DEADLINE_EXCEEDED",
        _ => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_action() {
        let (model, action) = parse_model_action("gemini-2.0-flash:generateContent").unwrap();
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(action, "generateContent");

        let (model, action) = parse_model_action("gemini-2.0-flash/streamGenerateContent").unwrap();
        assert_eq!(model, "gemini-2.0-flash");
        assert_eq!(action, "streamGenerateContent");
    }

    #[test]
    fn test_normalize_model_name() {
        assert_eq!(
            normalize_model_name("gemini-2.0-flash"),
            "models/gemini-2.0-flash"
        );
        assert_eq!(
            normalize_model_name("models/gemini-2.0-flash"),
            "models/gemini-2.0-flash"
        );
    }

    #[test]
    fn test_extract_model_name() {
        assert_eq!(
            extract_model_name("models/gemini-2.0-flash"),
            "gemini-2.0-flash"
        );
        assert_eq!(extract_model_name("gemini-2.0-flash"), "gemini-2.0-flash");
    }

    #[test]
    fn test_http_status_to_gemini_status() {
        assert_eq!(http_status_to_gemini_status(200), "OK");
        assert_eq!(http_status_to_gemini_status(400), "INVALID_ARGUMENT");
        assert_eq!(http_status_to_gemini_status(401), "UNAUTHENTICATED");
        assert_eq!(http_status_to_gemini_status(429), "RESOURCE_EXHAUSTED");
        assert_eq!(http_status_to_gemini_status(503), "UNAVAILABLE");
    }

    #[test]
    fn test_build_error_response() {
        let err = build_error_response(404, "Model not found", "NOT_FOUND");
        assert_eq!(err.error.code, 404);
        assert_eq!(err.error.message, "Model not found");
        assert_eq!(err.error.status, "NOT_FOUND");
    }
}
