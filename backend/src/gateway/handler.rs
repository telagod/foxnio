//! 完整请求转发实现

#![allow(dead_code)]
use anyhow::{bail, Result};
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use bytes::Bytes;
use reqwest::Client;

use crate::entity::accounts;
use crate::gateway::models::{resolve_model_alias, ModelProvider};
use crate::gateway::SharedState;
use crate::service::{
    LegacyAccountService as AccountService, LegacyBillingService as BillingService, ModelRouter,
    SchedulerService,
};

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
    model_router: ModelRouter,
}

impl GatewayHandler {
    /// Creates a new GatewayHandler instance.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client fails to build.
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
            model_router: ModelRouter::new(),
        }
    }

    /// 获取模型路由器
    pub fn model_router(&self) -> &ModelRouter {
        &self.model_router
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
        let account = self
            .scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;

        // 2. 获取上游 URL 和凭证
        let (base_url, credential) = self.get_upstream_config(&account).await?;

        // 3. 构建请求
        let url = format!("{base_url}/v1/chat/completions");

        let mut req = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {credential}"));

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
            self.handle_streaming_response(state, ctx, response, account.id)
                .await
        } else {
            self.handle_normal_response(state, ctx, response, account.id)
                .await
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
        let account = self
            .scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;

        let (base_url, credential) = self.get_upstream_config(&account).await?;

        let url = format!("{base_url}/v1/messages");

        let mut req = self
            .http_client
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
            self.handle_streaming_response(state, ctx, response, account.id)
                .await
        } else {
            self.handle_normal_response(state, ctx, response, account.id)
                .await
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

        let account = self
            .scheduler_service
            .select_account(model, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model))?;

        let base_url = "https://generativelanguage.googleapis.com";
        let credential = account.credential.clone();

        // Gemini 使用不同的 URL 结构
        let url = format!("{}{}:generateContent?key={}", base_url, method, credential);

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Upstream request failed: {}", e))?;

        self.handle_normal_response(state, ctx, response, account.id)
            .await
    }

    /// 处理普通响应
    async fn handle_normal_response(
        &self,
        _state: &SharedState,
        ctx: RequestContext,
        response: reqwest::Response,
        _account_id: uuid::Uuid,
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
                let input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|t| t.as_i64())
                    .unwrap_or(0);
                let output_tokens = usage
                    .get("completion_tokens")
                    .and_then(|t| t.as_i64())
                    .unwrap_or(0);
                let model_name = model.as_str().unwrap_or(&ctx.model);

                // 记录用量
                let _ = self
                    .billing_service
                    .record_usage(crate::service::billing::RecordUsageParams {
                        user_id: ctx.user_id,
                        api_key_id: ctx.api_key_id,
                        model: model_name.to_string(),
                        input_tokens,
                        output_tokens,
                        success: true,
                        error_message: None,
                    })
                    .await;
            }
        }

        Ok(builder.body(Body::from(body))?)
    }

    /// 处理流式响应
    async fn handle_streaming_response(
        &self,
        _state: &SharedState,
        _ctx: RequestContext,
        response: reqwest::Response,
        _account_id: uuid::Uuid,
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
        let _ = self
            .account_service
            .update_status(account_id, status, error.as_deref())
            .await;
    }

    /// 处理请求（带模型路由和降级）
    pub async fn handle_request_with_routing(
        &self,
        state: &SharedState,
        model_name: &str,
        ctx: RequestContext,
        mut body: Bytes,
    ) -> Result<Response> {
        // 使用模型路由器解析模型（支持别名和降级）
        let route_result = self.model_router.route_with_fallback(model_name).await?;

        // 记录降级信息
        if route_result.is_fallback {
            tracing::warn!(
                original_model = ?route_result.original_model,
                fallback_model = ?route_result.model,
                "Model fallback triggered"
            );
        }

        // 映射请求参数
        let mut params: serde_json::Value =
            serde_json::from_slice(&body).unwrap_or_else(|_| serde_json::json!({}));
        self.model_router
            .map_request_params(route_result.model, &mut params)?;
        body = serde_json::to_vec(&params)?.into();

        // 获取上游配置
        let provider_config = &route_result.provider_config;
        let base_url = provider_config.base_url.clone();

        // 选择账号
        let account = self
            .scheduler_service
            .select_account(model_name, ctx.session_id.as_deref(), 5)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No available account for model: {}", model_name))?;

        let credential = account.credential.clone();

        // 构建请求 URL
        let url = match route_result.provider {
            ModelProvider::Anthropic => format!("{base_url}/v1/messages"),
            ModelProvider::Google => format!(
                "{}{}:generateContent?key={}",
                base_url, route_result.config.api_name, credential
            ),
            _ => format!("{base_url}/v1/chat/completions"),
        };

        // 构建请求
        let mut req = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json");

        // 设置认证头
        req = match route_result.provider {
            ModelProvider::Anthropic => req
                .header("x-api-key", &credential)
                .header("anthropic-version", "2023-06-01"),
            ModelProvider::Google => req, // Gemini 使用 URL 参数
            _ => req.header("Authorization", format!("Bearer {credential}")),
        };

        // 流式请求特殊处理
        if ctx.stream {
            req = req.header("Accept", "text/event-stream");
        }

        // 发送请求
        let model_name_owned = model_name.to_string();
        let response = req.body(body.clone()).send().await.map_err(|e| {
            // 标记模型不可用
            let router = self.model_router.clone();
            tokio::spawn(async move {
                let model = resolve_model_alias(&model_name_owned);
                if let Some(m) = model {
                    router.set_model_available(m, false).await;
                }
            });
            anyhow::anyhow!("Upstream request failed: {}", e)
        })?;

        // 处理响应
        if ctx.stream {
            self.handle_streaming_response(state, ctx, response, account.id)
                .await
        } else {
            self.handle_normal_response(state, ctx, response, account.id)
                .await
        }
    }

    /// 获取所有可用模型列表
    pub fn list_available_models(&self) -> Vec<crate::gateway::models::ModelInfo> {
        self.model_router.list_available_models()
    }

    /// 获取模型信息
    pub fn get_model_info(&self, model_name: &str) -> Option<crate::gateway::models::ModelInfo> {
        resolve_model_alias(model_name).and_then(|m| self.model_router.get_model_info(m))
    }
}
