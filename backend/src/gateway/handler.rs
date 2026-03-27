//! 完整请求转发实现

use anyhow::{Result, bail};
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::entity::accounts;
use crate::service::{AccountService, BillingService, SchedulerService};
use crate::gateway::SharedState;

/// 请求上下文
pub struct RequestContext {
    pub user_id: uuid::Uuid,
    pub api_key_id: uuid::Uuid,
    pub model: String,
    pub stream: bool,
    pub session_id: Option<String>,
}

/// 上游响应
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub account_id: uuid::Uuid,
}

/// 流式响应
pub struct StreamingResponse {
    pub account_id: uuid::Uuid,
    pub stream: reqwest::Response,
}

/// 网关处理器
pub struct GatewayHandler {
    http_client: Client,
    account_service: AccountService,
    scheduler_service: SchedulerService,
    billing_service: BillingService,
}

impl GatewayHandler {
    pub fn new(
        account_service: AccountService,
        scheduler_service: SchedulerService,
        billing_service: BillingService,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .pool_max_idle_per_host(100)
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            http_client: client,
            account_service,
            scheduler_service,
            billing_service,
        }
    }

    /// 处理 Chat Completions 请求
    pub async fn handle_chat_completions(
        &self,
        state: &SharedState,
        ctx: RequestContext,
        body: Bytes,
    ) -> Result<Response> {
        let model = &ctx.model;
        
        // 1. 选择账号
        let account = self.scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;
        
        // 2. 获取上游 URL 和凭证
        let (base_url, credential) = self.get_upstream_config(&account).await?;
        
        // 3. 构建请求
        let url = format!("{}/v1/chat/completions", base_url);
        
        let mut req = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", credential));
        
        // 如果是流式请求
        if ctx.stream {
            req = req.header("Accept", "text/event-stream");
        }
        
        // 4. 发送请求
        let response = req
            .body(body.clone())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Upstream request failed: {}", e))?;
        
        // 5. 处理响应
        if ctx.stream {
            self.handle_streaming_response(state, ctx, response, account.id).await
        } else {
            self.handle_normal_response(state, ctx, response, account.id).await
        }
    }

    /// 处理 Anthropic Messages 请求
    pub async fn handle_messages(
        &self,
        state: &SharedState,
        ctx: RequestContext,
        body: Bytes,
    ) -> Result<Response> {
        let model = &ctx.model;
        
        // 选择 Anthropic 账号
        let account = self.scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;
        
        let (base_url, credential) = self.get_upstream_config(&account).await?;
        
        let url = format!("{}/v1/messages", base_url);
        
        let mut req = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &credential)
            .header("anthropic-version", "2023-06-01");
        
        if ctx.stream {
            req = req.header("Accept", "text/event-stream");
        }
        
        let response = req
            .body(body.clone())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Upstream request failed: {}", e))?;
        
        if ctx.stream {
            self.handle_streaming_response(state, ctx, response, account.id).await
        } else {
            self.handle_normal_response(state, ctx, response, account.id).await
        }
    }

    /// 处理 Gemini 请求
    pub async fn handle_gemini(
        &self,
        state: &SharedState,
        ctx: RequestContext,
        body: Bytes,
        method: &str,
    ) -> Result<Response> {
        let model = &ctx.model;
        
        let account = self.scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;
        
        let base_url = "https://generativelanguage.googleapis.com";
        let credential = account.credential.clone();
        
        // Gemini 使用不同的 URL 结构
        let url = format!("{}{}:generateContent?key={}", base_url, method, credential);
        
        let response = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Upstream request failed: {}", e))?;
        
        self.handle_normal_response(state, ctx, response, account.id).await
    }

    /// 处理普通响应
    async fn handle_normal_response(
        &self,
        state: &SharedState,
        ctx: RequestContext,
        response: reqwest::Response,
        account_id: uuid::Uuid,
    ) -> Result<Response> {
        let status = response.status();
        
        // 构建响应
        let mut builder = Response::builder().status(status.as_u16());
        
        // 复制响应头
        for (name, value) in response.headers() {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                builder = builder.header(name.as_str(), v);
            }
        }
        
        // 获取响应体
        let body = response.bytes().await?;
        
        // 解析 token 使用量
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body) {
            if let (Some(usage), Some(model)) = (json.get("usage"), json.get("model")) {
                let input_tokens = usage.get("prompt_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
                let output_tokens = usage.get("completion_tokens").and_then(|t| t.as_i64()).unwrap_or(0);
                let model_name = model.as_str().unwrap_or(&ctx.model);
                
                // 记录用量
                let _ = self.billing_service.record_usage(
                    ctx.user_id,
                    ctx.api_key_id,
                    Some(account_id),
                    model_name,
                    input_tokens,
                    output_tokens,
                    None,
                    true,
                    None,
                ).await;
            }
        }
        
        Ok(builder.body(Body::from(body))?)
    }

    /// 处理流式响应
    async fn handle_streaming_response(
        &self,
        state: &SharedState,
        ctx: RequestContext,
        response: reqwest::Response,
        account_id: uuid::Uuid,
    ) -> Result<Response> {
        use futures::stream::Stream;
        
        let status = response.status();
        
        // 构建响应
        let mut builder = Response::builder().status(status.as_u16());
        
        // 复制响应头
        for (name, value) in response.headers() {
            if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
                builder = builder.header(name.as_str(), v);
            }
        }
        
        // 创建流式响应体
        let stream = response.bytes_stream();
        let body = Body::from_stream(stream);
        
        Ok(builder.body(body)?)
    }

    /// 获取上游配置
    async fn get_upstream_config(&self, account: &accounts::Model) -> Result<(String, String)> {
        let (base_url, credential) = match account.provider.as_str() {
            "anthropic" => (
                "https://api.anthropic.com".to_string(),
                account.credential.clone(),
            ),
            "openai" => (
                "https://api.openai.com".to_string(),
                account.credential.clone(),
            ),
            "gemini" => (
                "https://generativelanguage.googleapis.com".to_string(),
                account.credential.clone(),
            ),
            "antigravity" => (
                "https://antigravity.so".to_string(),
                account.credential.clone(),
            ),
            "deepseek" => (
                "https://api.deepseek.com".to_string(),
                account.credential.clone(),
            ),
            _ => bail!("Unknown provider: {}", account.provider),
        };
        
        Ok((base_url, credential))
    }

    /// 更新账号状态
    async fn update_account_status(
        &self,
        account_id: uuid::Uuid,
        success: bool,
        error: Option<String>,
    ) {
        let status = if success { "active" } else { "error" };
        let _ = self.account_service.update_status(account_id, status, error.as_deref()).await;
    }
}
