//! Gateway 核心服务
//!
//! 网关的核心协调服务，管理请求转发、账号调度和响应处理

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    config: GatewayConfig,
    request_service: Arc<GatewayRequestService>,
    scheduler: Arc<RwLock<SchedulerService>>,
    stats: Arc<RwLock<GatewayStats>>,
}

impl GatewayService {
    /// 创建新的网关服务
    pub fn new(config: GatewayConfig, scheduler: SchedulerService) -> Self {
        Self {
            config,
            request_service: Arc::new(GatewayRequestService::default()),
            scheduler: Arc::new(RwLock::new(scheduler)),
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
        _parsed: &ParsedRequest,
        _account: &GatewayAccount,
        _route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        // TODO: 实现实际转发逻辑
        // 1. 解析 Chat Completions 请求
        // 2. 转换为目标格式
        // 3. 调用上游
        // 4. 转换响应

        Ok(ForwardResult {
            status_code: 200,
            headers: HashMap::new(),
            body: br#"{"choices":[]}"#.to_vec(),
            account_id: None,
            model: String::new(),
            latency_ms: 0,
            usage: None,
            cached: false,
        })
    }

    /// 转发 Responses 请求
    async fn forward_responses(
        &self,
        _parsed: &ParsedRequest,
        _account: &GatewayAccount,
        _route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        // TODO: 实现实际转发逻辑
        Ok(ForwardResult {
            status_code: 200,
            headers: HashMap::new(),
            body: br#"{"output":[]}"#.to_vec(),
            account_id: None,
            model: String::new(),
            latency_ms: 0,
            usage: None,
            cached: false,
        })
    }

    /// 通用转发
    async fn forward_generic(
        &self,
        _parsed: &ParsedRequest,
        _account: &GatewayAccount,
        _route: &super::gateway_request::ModelRoute,
    ) -> Result<ForwardResult> {
        // TODO: 实现实际转发逻辑
        Ok(ForwardResult {
            status_code: 200,
            headers: HashMap::new(),
            body: vec![],
            account_id: None,
            model: String::new(),
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
