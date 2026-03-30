// Claude Code Shell - 轻量级网络层
// 只提取 TLS 指纹 + HTTP 头模板，用于转发 API 请求

pub mod headers;
pub mod client;
pub mod request;
pub mod error;
pub mod sse;
pub mod tls;

use serde::{Deserialize, Serialize};
use anyhow::Result;
use reqwest::StatusCode;

// 重导出常用类型
pub use error::{AnthropicError, ErrorDetail};
pub use sse::{SseEvent, parse_sse_line, parse_sse_stream};

/// Claude Code Shell 配置
#[derive(Debug, Clone)]
pub struct ClaudeShellConfig {
    /// API Key
    pub api_key: String,
    /// API 基础 URL
    pub base_url: String,
    /// API 版本
    pub api_version: String,
    /// 是否启用流式输出
    pub stream: bool,
}

impl Default for ClaudeShellConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            api_version: "2023-06-01".to_string(),
            stream: true,
        }
    }
}

/// Claude Code Shell 客户端
pub struct ClaudeShell {
    config: ClaudeShellConfig,
    client: reqwest::Client,
}

impl ClaudeShell {
    /// 创建新的 Claude Shell 客户端
    pub fn new(config: ClaudeShellConfig) -> Result<Self> {
        let client = client::build_client()?;
        Ok(Self { config, client })
    }

    /// 发送消息请求
    pub async fn send_message(&self, request: request::MessageRequest) -> Result<request::MessageResponse> {
        let url = format!("{}/v1/messages", self.config.base_url);
        let headers = headers::build_headers(&self.config.api_key, &self.config.api_version);
        
        let response = self.client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        // 检查状态码
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            let error = error::parse_error(&body)?;
            return Err(anyhow::anyhow!("API error ({}): {}", status, error.error.message));
        }

        let message = response.json::<request::MessageResponse>().await?;
        Ok(message)
    }

    /// 发送流式消息请求
    pub async fn send_message_stream(&self, request: request::MessageRequest) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.config.base_url);
        let headers = headers::build_headers(&self.config.api_key, &self.config.api_version);
        
        let mut stream_request = request;
        stream_request.stream = Some(true);

        let response = self.client
            .post(&url)
            .headers(headers)
            .json(&stream_request)
            .send()
            .await?;

        // 检查状态码
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            let error = error::parse_error(&body)?;
            return Err(anyhow::anyhow!("API error ({}): {}", status, error.error.message));
        }

        Ok(response)
    }
    
    /// 测试 API 连接
    pub async fn test_connection(&self) -> Result<bool> {
        let request = request::MessageRequest {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![request::Message {
                role: "user".to_string(),
                content: request::MessageContent::Text("ping".to_string()),
            }],
            max_tokens: 10,
            stream: None,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            tools: None,
            metadata: None,
        };
        
        match self.send_message(request).await {
            Ok(_) => Ok(true),
            Err(e) => {
                // 如果是认证错误，返回 false
                if e.to_string().contains("401") {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClaudeShellConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.api_version, "2023-06-01");
        assert!(config.stream);
    }
}
