//! 网关请求转发

use anyhow::Result;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use hyper::body::Incoming;
use reqwest::Client;
use std::sync::Arc;

use crate::gateway::SharedState;

pub struct ProxyClient {
    http_client: Client,
}

impl ProxyClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { http_client: client }
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
        
        // 构建请求
        let mut req_builder = self.http_client.request(
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
                req_builder = req_builder.header(name.as_str(), value);
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
}

impl Default for ProxyClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 上游服务端点
pub struct UpstreamEndpoints;

impl UpstreamEndpoints {
    /// Anthropic API 端点
    pub fn anthropic() -> &'static str {
        "https://api.anthropic.com"
    }

    /// OpenAI API 端点
    pub fn openai() -> &'static str {
        "https://api.openai.com"
    }

    /// Google Gemini API 端点
    pub fn gemini() -> &'static str {
        "https://generativelanguage.googleapis.com"
    }
}
