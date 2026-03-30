// Claude Code Shell - 轻量级网络层
// 只提取 TLS 指纹 + HTTP 头模板，用于转发 API 请求

pub mod headers;
pub mod client;
pub mod request;
pub mod tls;

use serde::{Deserialize, Serialize};
use anyhow::Result;

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

        let message = response.json::<request::MessageResponse>().await?;
        Ok(message)
    }

    /// 发送流式消息请求
    pub async fn send_message_stream(&self, request: request::MessageRequest) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.config.base_url);
        let mut headers = headers::build_headers(&self.config.api_key, &self.config.api_version);
        
        let mut stream_request = request;
        stream_request.stream = Some(true);

        let response = self.client
            .post(&url)
            .headers(headers)
            .json(&stream_request)
            .send()
            .await?;

        Ok(response)
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
