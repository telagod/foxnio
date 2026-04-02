//! Gateway 核心服务
//!
//! 网关的核心协调服务，管理请求转发、账号调度和响应处理

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::account::AccountService;
use super::gateway_request::{GatewayRequestService, ParsedRequest};
use super::scheduler::SchedulerService;

/// 转发结果
#[derive(Debug, Clone)]
pub struct ForwardResult {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub account_id: Option<String>,
    pub model: String,
    pub latency_ms: u64,
    pub usage: Option<TokenUsage>,
    pub cached: bool,
}

/// Token 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 账号信息
#[derive(Debug, Clone)]
pub struct GatewayAccount {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub account_type: String,
    pub status: String,
    pub priority: i32,
    pub concurrent_limit: u32,
    pub rate_limit_rpm: u32,
    pub model_mapping: HashMap<String, String>,
    pub extra: serde_json::Value,
}

/// 网关配置
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub default_timeout_ms: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub enable_cache: bool,
    pub cache_ttl_seconds: u64,
    pub enable_idempotency: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_cache: true,
            cache_ttl_seconds: 300,
            enable_idempotency: true,
        }
    }
}

/// 网关统计
#[derive(Debug, Clone, Default)]
pub struct GatewayStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub avg_latency_ms: f64,
    pub cache_hits: u64,
}

/// Gateway 服务
pub struct GatewayService {
    db: sea_orm::DatabaseConnection,
    config: GatewayConfig,
    request_service: Arc<GatewayRequestService>,
    scheduler: Arc<RwLock<SchedulerService>>,
    account_service: Arc<AccountService>,
    http_client: reqwest::Client,
    stats: Arc<RwLock<GatewayStats>>,
}

impl GatewayService {
    /// 创建新的网关服务
    pub fn new(
        db: sea_orm::DatabaseConnection,
        config: GatewayConfig,
        scheduler: SchedulerService,
        account_service: AccountService,
    ) -> Self {
        let timeout_ms = config.default_timeout_ms;
        Self {
            db,
            config,
            request_service: Arc::new(GatewayRequestService::default()),
            scheduler: Arc::new(RwLock::new(scheduler)),
            account_service: Arc::new(account_service),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(timeout_ms))
                .build()
                .unwrap_or_default(),
            stats: Arc::new(RwLock::new(GatewayStats::default())),
        }
    }

    /// 处理请求
    pub async fn handle_request(
        &self,
        method: &str,
        path: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<ForwardResult> {
        let start_time = std::time::Instant::now();

        // 1. 解析请求
        let parsed = self
            .request_service
            .parse_request(method, path, headers.clone(), body)?;

        // 2. 验证请求
        let validation = self.request_service.validate_request(&parsed);
        if !validation.is_valid {
            return Err(anyhow!("Request validation failed: {:?}", validation.error));
        }

        // 3. 路由模型
        let route = self.request_service.route_model(&parsed);

        // 4. 选择账号
        let account = self.select_account(&route).await?;

        // 5. 转发请求
        let result = self.forward_request(&parsed, &account, &route).await?;

        // 6. 更新统计
        self.update_stats(&result, start_time.elapsed().as_millis() as u64)
            .await;

        Ok(result)
    }

    /// 选择账号
    async fn select_account(
        &self,
        route: &super::gateway_request::ModelRoute,
    ) -> Result<GatewayAccount> {
        let scheduler = self.scheduler.read().await;

        // 调用调度器选择账号
        let account_opt = scheduler
            .select_account(&route.mapped_model, None, 5)
            .await?;

        account_opt
            .map(|acc| GatewayAccount {
                id: acc.id.to_string(),
                name: acc.name,
                provider: acc.provider,
                account_type: "api_key".to_string(),
                status: acc.status,
                priority: acc.priority,
                concurrent_limit: acc.concurrent_limit.unwrap_or(5) as u32,
                rate_limit_rpm: acc.rate_limit_rpm.unwrap_or(60) as u32,
                model_mapping: HashMap::new(),
                extra: serde_json::json!({}),
            })
            .ok_or_else(|| anyhow!("No available account for model: {}", route.mapped_model))
    }

    /// 转发请求
    async fn forward_request(
        &self,
        parsed: &ParsedRequest,
        account: &GatewayAccount,
        route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        let start_time = std::time::Instant::now();

        // 根据路径选择转发方式
        let result = if parsed.path.contains("/chat/completions") {
            self.forward_chat_completions(parsed, account, route)
                .await?
        } else if parsed.path.contains("/responses") {
            self.forward_responses(parsed, account, route).await?
        } else {
            self.forward_generic(parsed, account, route).await?
        };

        let latency_ms = start_time.elapsed().as_millis() as u64;

        Ok(ForwardResult {
            status_code: result.status_code,
            headers: result.headers,
            body: result.body,
            account_id: Some(account.id.clone()),
            model: route.mapped_model.clone(),
            latency_ms,
            usage: result.usage,
            cached: false,
        })
    }

    /// 转发 Chat Completions 请求
    async fn forward_chat_completions(
        &self,
        parsed: &ParsedRequest,
        account: &GatewayAccount,
        route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        use super::chat_completions_forwarder::{ChatCompletionsRequest, Message, MessageContent};

        // 1. 解析请求体
        let body: serde_json::Value = serde_json::from_slice(&parsed.body)
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));

        // 2. 构建 ChatCompletionsRequest
        let messages: Vec<Message> = body
            .get("messages")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|msg| {
                        let role = msg.get("role")?.as_str()?.to_string();
                        let content = msg.get("content")?;
                        let msg_content = if content.is_string() {
                            MessageContent::Text(content.as_str()?.to_string())
                        } else {
                            MessageContent::Text(content.to_string())
                        };
                        Some(Message {
                            role,
                            content: msg_content,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let request = ChatCompletionsRequest {
            model: route.mapped_model.clone(),
            messages,
            temperature: body.get("temperature").and_then(|t| t.as_f64()).map(|v| v as f32),
            max_tokens: body.get("max_tokens").and_then(|t| t.as_u64()).map(|v| v as u32),
            stream: body.get("stream").and_then(|s| s.as_bool()).unwrap_or(false),
            stream_options: None,
            extra: body.clone(),
        };

        // 3. 获取账号凭证
        let account_id = uuid::Uuid::parse_str(&account.id)?;
        let credential = self.get_account_credential(account_id).await?;

        // 4. 构建上游请求
        let upstream_url = self.get_upstream_url(&account.provider);
        let mut req_builder = self
            .http_client
            .post(&upstream_url)
            .json(&request)
            .header("Content-Type", "application/json");

        // 设置认证头
        if account.provider.to_lowercase() == "anthropic" {
            req_builder = req_builder
                .header("x-api-key", &credential)
                .header("anthropic-version", "2023-06-01");
        } else {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", credential));
        }

        // 5. 发送请求
        let response = req_builder.send().await?;
        let status = response.status().as_u16();

        // 6. 处理响应
        let response_body = response.bytes().await?;
        let response_json: serde_json::Value = serde_json::from_slice(&response_body)
            .unwrap_or_else(|_| serde_json::json!({}));

        // 提取使用量
        let usage = response_json.get("usage").map(|u| TokenUsage {
            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        Ok(ForwardResult {
            status_code: status,
            headers: HashMap::new(),
            body: response_body.to_vec(),
            account_id: Some(account.id.clone()),
            model: route.mapped_model.clone(),
            latency_ms: 0,
            usage,
            cached: false,
        })
    }

    /// 获取账号凭证
    async fn get_account_credential(&self, account_id: uuid::Uuid) -> Result<String> {
        use crate::entity::accounts;
        use sea_orm::EntityTrait;

        let account = accounts::Entity::find_by_id(account_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Account not found: {}", account_id))?;

        let credential = crate::utils::encryption_global::GlobalEncryption::decrypt(&account.credential)
            .map_err(|e| anyhow!("Failed to decrypt credential: {}", e))?;

        Ok(credential)
    }

    /// 获取上游 URL
    fn get_upstream_url(&self, provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
            "anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
            "gemini" => "https://generativelanguage.googleapis.com/v1/chat/completions".to_string(),
            "deepseek" => "https://api.deepseek.com/v1/chat/completions".to_string(),
            "mistral" => "https://api.mistral.ai/v1/chat/completions".to_string(),
            "cohere" => "https://api.cohere.ai/v1/chat/completions".to_string(),
            _ => format!("https://api.{}/v1/chat/completions", provider.to_lowercase()),
        }
    }

    /// 转发 Responses 请求 (OpenAI Responses API)
    async fn forward_responses(
        &self,
        parsed: &ParsedRequest,
        account: &GatewayAccount,
        route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        // Responses API 是 OpenAI 的新 API 格式
        // 这里实现为转发到 OpenAI 的 responses 端点

        let account_id = uuid::Uuid::parse_str(&account.id)?;
        let credential = self.get_account_credential(account_id).await?;

        // 构建请求
        let upstream_url = "https://api.openai.com/v1/responses";
        let response = self
            .http_client
            .post(upstream_url)
            .header("Authorization", format!("Bearer {}", credential))
            .header("Content-Type", "application/json")
            .body(parsed.body.clone())
            .send()
            .await?;

        let status = response.status().as_u16();
        let response_body = response.bytes().await?;

        Ok(ForwardResult {
            status_code: status,
            headers: HashMap::new(),
            body: response_body.to_vec(),
            account_id: Some(account.id.clone()),
            model: route.mapped_model.clone(),
            latency_ms: 0,
            usage: None,
            cached: false,
        })
    }

    /// 通用转发
    async fn forward_generic(
        &self,
        parsed: &ParsedRequest,
        account: &GatewayAccount,
        route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        let account_id = uuid::Uuid::parse_str(&account.id)?;
        let credential = self.get_account_credential(account_id).await?;

        // 构建通用请求 URL
        let upstream_url = self.get_upstream_url(&account.provider);

        // 构建请求
        let mut req = self
            .http_client
            .post(&upstream_url)
            .header("Content-Type", "application/json")
            .body(parsed.body.clone());

        // 设置认证
        if account.provider.to_lowercase() == "anthropic" {
            req = req.header("x-api-key", &credential);
        } else {
            req = req.header("Authorization", format!("Bearer {}", credential));
        }

        let response = req.send().await?;
        let status = response.status().as_u16();
        let response_body = response.bytes().await?;

        Ok(ForwardResult {
            status_code: status,
            headers: HashMap::new(),
            body: response_body.to_vec(),
            account_id: Some(account.id.clone()),
            model: route.mapped_model.clone(),
            latency_ms: 0,
            usage: None,
            cached: false,
        })
    }

    /// 更新统计
    async fn update_stats(&self, result: &ForwardResult, latency_ms: u64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        if result.status_code >= 200 && result.status_code < 300 {
            stats.successful_requests += 1;
        } else {
            stats.failed_requests += 1;
        }

        if let Some(usage) = &result.usage {
            stats.total_tokens += usage.total_tokens as u64;
        }

        // 更新平均延迟
        let count = stats.total_requests;
        stats.avg_latency_ms =
            (stats.avg_latency_ms * (count - 1) as f64 + latency_ms as f64) / count as f64;
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> GatewayStats {
        self.stats.read().await.clone()
    }

    /// 健康检查
    pub async fn health_check(&self) -> bool {
        // 检查服务状态
        let stats = self.stats.read().await;

        // 如果最近有成功的请求，认为健康
        if stats.successful_requests > 0 {
            let success_rate = stats.successful_requests as f64 / stats.total_requests as f64;
            return success_rate > 0.5;
        }

        true
    }

    /// 重置统计
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = GatewayStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_service_creation() {
        // TODO: Fix test - requires SchedulerService with db and account_service
        // let scheduler = SchedulerService::new(db, account_service, strategy);
        // let service = GatewayService::new(GatewayConfig::default(), scheduler);
        // let stats = service.get_stats().await;
        // assert_eq!(stats.total_requests, 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        // TODO: Fix test - requires SchedulerService with db and account_service
        // let healthy = service.health_check().await;
        // assert!(healthy);
    }

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert_eq!(config.default_timeout_ms, 30000);
        assert_eq!(config.max_retries, 3);
        assert!(config.enable_cache);
    }
}
