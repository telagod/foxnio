//! 网关请求转发
//!
//! 支持 HTTP/2 和 HTTP/1.1 自动协商

use anyhow::Result;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use hyper::body::Incoming;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Http2ClientConfig;
use crate::gateway::SharedState;

/// HTTP/2 代理客户端
pub struct ProxyClient {
    http_client: Client,
    http2_client: Option<Client>,
    config: Http2ClientConfig,
}

impl ProxyClient {
    /// 创建新的代理客户端
    pub fn new() -> Self {
        Self::with_config(Http2ClientConfig::default())
    }

    /// 使用配置创建代理客户端
    pub fn with_config(config: Http2ClientConfig) -> Self {
        let http_client = Self::build_client(&config, false);
        let http2_client = if config.enabled {
            Some(Self::build_client(&config, true))
        } else {
            None
        };

        Self {
            http_client,
            http2_client,
            config,
        }
    }

    /// 构建 HTTP 客户端
    fn build_client(config: &Http2ClientConfig, http2_only: bool) -> Client {
        let mut builder = Client::builder()
            .timeout(config.request_timeout())
            .connect_timeout(config.connect_timeout())
            .pool_max_idle_per_host(config.max_idle_connections)
            .pool_idle_timeout(config.pool_keep_alive())
            .tcp_keepalive(config.tcp_keepalive())
            .tcp_nodelay(config.tcp_nodelay);

        if http2_only {
            // HTTP/2 only mode
            builder = builder
                .http2_initial_stream_window_size(config.initial_stream_window_size)
                .http2_prior_knowledge();
        } else if config.auto_negotiate {
            // Auto negotiate: support both HTTP/1.1 and HTTP/2
            builder = builder
                .http2_initial_stream_window_size(config.initial_stream_window_size);
        }

        builder.build().expect("Failed to create HTTP client")
    }

    /// 转发请求到上游
    pub async fn proxy_request(
        &self,
        state: &SharedState,
        upstream_url: &str,
        request: Request<Body>,
        api_key: Option<&str>,
    ) -> Result<Response> {
        let method = request.method().clone();
        let uri = request.uri().clone();
        let path = uri.path();

        // 构建上游 URL
        let upstream_uri = format!("{}{}", upstream_url, path);

        // 选择客户端 (基于配置和上游 URL)
        let client = self.select_client(upstream_url);

        // 构建请求
        let mut req_builder = client.request(
            match method {
                Method::GET => reqwest::Method::GET,
                Method::POST => reqwest::Method::POST,
                Method::PUT => reqwest::Method::PUT,
                Method::DELETE => reqwest::Method::DELETE,
                Method::PATCH => reqwest::Method::PATCH,
                _ => reqwest::Method::POST,
            },
            &upstream_uri,
        );

        // 添加请求头
        let headers = request.headers().clone();
        for (name, value) in headers.iter() {
            if name != "host" && name != "content-length" {
                // 将 axum 的 HeaderValue 转换为 reqwest 的 HeaderValue
                if let Ok(reqwest_value) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                    req_builder = req_builder.header(name.as_str(), reqwest_value);
                }
            }
        }

        // 添加上游 API Key
        if let Some(key) = api_key {
            req_builder = req_builder.header("x-api-key", key);
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        // 添加请求体
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024 * 10).await?;
        if !body.is_empty() {
            req_builder = req_builder.body(body);
        }

        // 发送请求
        let response = req_builder.send().await?;

        // 转换响应
        let status = response.status();
        let mut builder = Response::builder().status(status.as_u16());

        for (name, value) in response.headers() {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                builder = builder.header(name.as_str(), v);
            }
        }

        let body = response.bytes().await?;
        Ok(builder.body(Body::from(body))?)
    }

    /// 选择合适的客户端
    fn select_client(&self, upstream_url: &str) -> &Client {
        // 如果启用了 HTTP/2 且支持 HTTP/2 prior knowledge
        if self.config.enabled {
            // 检查上游是否已知支持 HTTP/2
            if self.is_http2_upstream(upstream_url) {
                return self.http2_client.as_ref().unwrap_or(&self.http_client);
            }
        }
        &self.http_client
    }

    /// 检查上游是否支持 HTTP/2
    fn is_http2_upstream(&self, upstream_url: &str) -> bool {
        // 已知支持 HTTP/2 的上游服务
        let http2_upstreams = [
            "api.anthropic.com",
            "api.openai.com",
            "generativelanguage.googleapis.com",
            "api.cohere.ai",
        ];

        http2_upstreams
            .iter()
            .any(|&upstream| upstream_url.contains(upstream))
    }

    /// 获取客户端配置
    pub fn config(&self) -> &Http2ClientConfig {
        &self.config
    }
}

impl Default for ProxyClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 上游服务端点
pub struct UpstreamEndpoints;

impl UpstreamEndpoints {
    /// Anthropic API 端点 (支持 HTTP/2)
    pub fn anthropic() -> &'static str {
        "https://api.anthropic.com"
    }

    /// OpenAI API 端点 (支持 HTTP/2)
    pub fn openai() -> &'static str {
        "https://api.openai.com"
    }

    /// Google Gemini API 端点 (支持 HTTP/2)
    pub fn gemini() -> &'static str {
        "https://generativelanguage.googleapis.com"
    }

    /// Cohere API 端点 (支持 HTTP/2)
    pub fn cohere() -> &'static str {
        "https://api.cohere.ai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_client_creation() {
        let client = ProxyClient::new();
        assert!(client.config.enabled);
    }

    #[test]
    fn test_http2_client_config() {
        let config = Http2ClientConfig {
            enabled: true,
            pool_size: 16,
            auto_negotiate: true,
            ..Default::default()
        };
        let client = ProxyClient::with_config(config);
        assert!(client.http2_client.is_some());
    }

    #[test]
    fn test_is_http2_upstream() {
        let client = ProxyClient::new();
        assert!(client.is_http2_upstream("https://api.anthropic.com"));
        assert!(client.is_http2_upstream("https://api.openai.com"));
        assert!(!client.is_http2_upstream("https://unknown-api.com"));
    }
}
