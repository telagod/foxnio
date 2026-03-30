//! 故障转移实现
//!
//! 包含：
//! - 账号健康状态管理
//! - 错误透传规则匹配
//! - 临时封禁机制
//! - 同账号重试逻辑

#![allow(dead_code)]
use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::entity::accounts;

/// 故障转移配置
#[derive(Debug, Clone)]
pub struct FailoverConfig {
    /// 最大重试次数
    pub max_retries: usize,
    /// 重试间隔 (毫秒)
    pub retry_delay_ms: u64,
    /// 指数退避基数
    pub backoff_base: f64,
    /// 最大退避时间 (毫秒)
    pub max_backoff_ms: u64,
    /// 同账号最大重试次数 (针对 RetryableOnSameAccount 错误)
    pub max_same_account_retries: usize,
    /// 同账号重试间隔 (毫秒)
    pub same_account_retry_delay_ms: u64,
    /// Google 配置错误临时封禁时长 (秒)
    pub google_config_error_cooldown_secs: u64,
    /// 空响应临时封禁时长 (秒)
    pub empty_response_cooldown_secs: u64,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            backoff_base: 2.0,
            max_backoff_ms: 5000,
            max_same_account_retries: 3,
            same_account_retry_delay_ms: 500,
            google_config_error_cooldown_secs: 60,
            empty_response_cooldown_secs: 60,
        }
    }
}

/// 账号健康状态
#[derive(Debug, Clone)]
pub struct AccountHealth {
    pub account_id: uuid::Uuid,
    pub is_healthy: bool,
    pub last_success: Option<Instant>,
    pub last_failure: Option<Instant>,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    /// 临时不可调度截止时间
    pub temp_unschedulable_until: Option<Instant>,
    /// 临时不可调度原因
    pub temp_unschedulable_reason: Option<String>,
}

/// 上游故障转移错误
///
/// 表示应该触发账号切换的上游错误
#[derive(Debug, Clone)]
pub struct UpstreamFailoverError {
    /// HTTP 状态码
    pub status_code: u16,
    /// 响应体
    pub response_body: Vec<u8>,
    /// 响应头
    pub response_headers: HashMap<String, String>,
    /// 是否强制缓存计费 (粘性会话切换时)
    pub force_cache_billing: bool,
    /// 是否可在同账号重试
    /// 对临时性错误（如 Google 间歇性 400、空响应），应在同一账号上重试 N 次再切换
    pub retryable_on_same_account: bool,
}

impl std::fmt::Display for UpstreamFailoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "upstream error: {} (failover, retryable={})",
            self.status_code, self.retryable_on_same_account
        )
    }
}

impl std::error::Error for UpstreamFailoverError {}

impl UpstreamFailoverError {
    /// 创建新的故障转移错误
    pub fn new(status_code: u16, response_body: Vec<u8>) -> Self {
        Self {
            status_code,
            response_body,
            response_headers: HashMap::new(),
            force_cache_billing: false,
            retryable_on_same_account: false,
        }
    }

    /// 创建可重试的故障转移错误
    pub fn retryable(status_code: u16, response_body: Vec<u8>) -> Self {
        Self {
            status_code,
            response_body,
            response_headers: HashMap::new(),
            force_cache_billing: false,
            retryable_on_same_account: true,
        }
    }

    /// 设置响应头
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.response_headers = headers;
        self
    }

    /// 设置强制缓存计费标记
    pub fn with_force_cache_billing(mut self, force: bool) -> Self {
        self.force_cache_billing = force;
        self
    }

    /// 从错误中提取消息
    pub fn extract_message(&self) -> Option<String> {
        if self.response_body.is_empty() {
            return None;
        }

        // 尝试解析为 JSON
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&self.response_body) {
            // 尝试常见的错误消息字段
            if let Some(msg) = json.get("error").and_then(|e| e.get("message")) {
                return msg.as_str().map(|s| s.to_string());
            }
            if let Some(msg) = json.get("message") {
                return msg.as_str().map(|s| s.to_string());
            }
        }

        // 尝试作为 UTF-8 字符串
        if let Ok(msg) = String::from_utf8(self.response_body.clone()) {
            let trimmed = msg.trim();
            if !trimmed.is_empty() && trimmed.len() < 512 {
                return Some(trimmed.to_string());
            }
        }

        None
    }
}

/// 临时封禁状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempUnschedState {
    /// 封禁截止时间 (Unix 时间戳)
    pub until_unix: i64,
    /// 触发时间 (Unix 时间戳)
    pub triggered_at_unix: i64,
    /// 触发的状态码
    pub status_code: u16,
    /// 匹配的关键词
    pub matched_keyword: Option<String>,
    /// 触发原因
    pub reason: String,
}

impl TempUnschedState {
    /// 检查是否仍在封禁期内
    pub fn is_active(&self) -> bool {
        let now = Utc::now().timestamp();
        now < self.until_unix
    }

    /// 获取剩余封禁时间
    pub fn remaining_duration(&self) -> Duration {
        let now = Utc::now().timestamp();
        if now >= self.until_unix {
            Duration::ZERO
        } else {
            Duration::from_secs((self.until_unix - now) as u64)
        }
    }
}

/// 故障转移管理器
pub struct FailoverManager {
    config: FailoverConfig,
    health_status: Arc<RwLock<HashMap<uuid::Uuid, AccountHealth>>>,
}

impl FailoverManager {
    pub fn new(config: FailoverConfig) -> Self {
        Self {
            config,
            health_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 执行带故障转移的请求
    pub async fn execute_with_failover<F, T>(
        &self,
        accounts: Vec<accounts::Model>,
        mut request_fn: F,
    ) -> Result<(T, uuid::Uuid)>
    where
        F: FnMut(
            &accounts::Model,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, UpstreamFailoverError>> + Send>,
        >,
    {
        let mut last_error = None;
        let mut attempts = 0;
        let mut same_account_retry_count: HashMap<uuid::Uuid, usize> = HashMap::new();
        let mut failed_accounts: HashMap<uuid::Uuid, UpstreamFailoverError> = HashMap::new();

        for account in &accounts {
            let account_id = account.id;

            // 检查账号健康状态
            if !self.is_account_healthy(&account_id).await {
                continue;
            }

            // 检查是否已失败过
            if failed_accounts.contains_key(&account_id) {
                continue;
            }

            loop {
                // 计算退避时间
                if attempts > 0 {
                    let delay = self.calculate_backoff(attempts);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }

                // 执行请求
                match request_fn(account).await {
                    Ok(result) => {
                        // 标记成功
                        self.mark_success(account_id).await;
                        return Ok((result, account_id));
                    }
                    Err(failover_err) => {
                        last_error = Some(failover_err.clone());

                        // 同账号重试：对 RetryableOnSameAccount 的临时性错误，先在同一账号上重试
                        if failover_err.retryable_on_same_account {
                            let retry_count =
                                same_account_retry_count.entry(account_id).or_insert(0);

                            if *retry_count < self.config.max_same_account_retries {
                                *retry_count += 1;

                                tracing::warn!(
                                    account_id = %account_id,
                                    upstream_status = failover_err.status_code,
                                    retry_count = *retry_count,
                                    max_retries = self.config.max_same_account_retries,
                                    "gateway.failover_same_account_retry"
                                );

                                // 等待后重试
                                tokio::time::sleep(Duration::from_millis(
                                    self.config.same_account_retry_delay_ms,
                                ))
                                .await;
                                continue;
                            }

                            // 同账号重试用尽，执行临时封禁
                            self.temp_unschedule_retryable_error(&account_id, &failover_err)
                                .await;
                        }

                        // 标记失败
                        self.mark_failure(account_id, failover_err.to_string())
                            .await;

                        // 加入失败列表
                        failed_accounts.insert(account_id, failover_err);
                        attempts += 1;

                        break;
                    }
                }
            }

            if attempts >= self.config.max_retries {
                break;
            }
        }

        // 所有账号都失败
        let last_err = last_error
            .unwrap_or_else(|| UpstreamFailoverError::new(502, b"All accounts exhausted".to_vec()));

        bail!(
            "All accounts failed after {} attempts. Last error: {}",
            attempts,
            last_err
        )
    }

    /// 对 RetryableOnSameAccount 类型的 failover 错误触发临时封禁
    pub async fn temp_unschedule_retryable_error(
        &self,
        account_id: &uuid::Uuid,
        failover_err: &UpstreamFailoverError,
    ) {
        if !failover_err.retryable_on_same_account {
            return;
        }

        let (duration, reason) = match failover_err.status_code {
            // 400: Google 配置类错误
            400 => {
                let msg = failover_err
                    .extract_message()
                    .unwrap_or_else(|| "unknown error".to_string());
                let reason = format!(
                    "400: {} (auto temp-unschedule {}s)",
                    msg, self.config.google_config_error_cooldown_secs
                );
                (
                    Duration::from_secs(self.config.google_config_error_cooldown_secs),
                    reason,
                )
            }
            // 502: 空响应
            502 => {
                let reason = format!(
                    "empty stream response (auto temp-unschedule {}s)",
                    self.config.empty_response_cooldown_secs
                );
                (
                    Duration::from_secs(self.config.empty_response_cooldown_secs),
                    reason,
                )
            }
            _ => return,
        };

        self.temp_unschedule(account_id, duration, reason).await;
    }

    /// 临时封禁账号
    pub async fn temp_unschedule(
        &self,
        account_id: &uuid::Uuid,
        duration: Duration,
        reason: String,
    ) {
        let mut status = self.health_status.write().await;

        let until = Instant::now() + duration;

        let health = status.entry(*account_id).or_insert(AccountHealth {
            account_id: *account_id,
            is_healthy: true,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            last_error: None,
            temp_unschedulable_until: None,
            temp_unschedulable_reason: None,
        });

        health.temp_unschedulable_until = Some(until);
        health.temp_unschedulable_reason = Some(reason.clone());

        tracing::info!(
            account_id = %account_id,
            duration_secs = duration.as_secs(),
            reason = %reason,
            "gateway.temp_unscheduled"
        );
    }

    /// 检查账号是否健康
    pub async fn is_account_healthy(&self, account_id: &uuid::Uuid) -> bool {
        let status = self.health_status.read().await;

        if let Some(health) = status.get(account_id) {
            // 检查临时封禁
            if let Some(until) = health.temp_unschedulable_until {
                if until > Instant::now() {
                    return false;
                }
            }

            // 连续失败超过 3 次，认为不健康
            if health.consecutive_failures >= 3 {
                // 但如果超过 5 分钟，允许重试
                if let Some(last_failure) = health.last_failure {
                    if last_failure.elapsed() > Duration::from_secs(300) {
                        return true;
                    }
                }
                return false;
            }
        }

        true
    }

    /// 标记成功
    pub async fn mark_success(&self, account_id: uuid::Uuid) {
        let mut status = self.health_status.write().await;

        let health = status.entry(account_id).or_insert(AccountHealth {
            account_id,
            is_healthy: true,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            last_error: None,
            temp_unschedulable_until: None,
            temp_unschedulable_reason: None,
        });

        health.is_healthy = true;
        health.last_success = Some(Instant::now());
        health.consecutive_failures = 0;
        health.last_error = None;
        // 成功后清除临时封禁
        health.temp_unschedulable_until = None;
        health.temp_unschedulable_reason = None;
    }

    /// 标记失败
    pub async fn mark_failure(&self, account_id: uuid::Uuid, error: String) {
        let mut status = self.health_status.write().await;

        let health = status.entry(account_id).or_insert(AccountHealth {
            account_id,
            is_healthy: true,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            last_error: None,
            temp_unschedulable_until: None,
            temp_unschedulable_reason: None,
        });

        health.is_healthy = false;
        health.last_failure = Some(Instant::now());
        health.consecutive_failures += 1;
        health.last_error = Some(error);
    }

    /// 计算退避时间
    fn calculate_backoff(&self, attempt: usize) -> u64 {
        let backoff =
            self.config.retry_delay_ms as f64 * self.config.backoff_base.powi(attempt as i32);

        backoff.min(self.config.max_backoff_ms as f64) as u64
    }

    /// 获取账号健康统计
    pub async fn get_health_stats(&self) -> HashMap<uuid::Uuid, AccountHealth> {
        self.health_status.read().await.clone()
    }

    /// 重置账号健康状态
    pub async fn reset_health(&self, account_id: &uuid::Uuid) {
        let mut status = self.health_status.write().await;
        status.remove(account_id);
    }

    /// 重置所有健康状态
    pub async fn reset_all(&self) {
        let mut status = self.health_status.write().await;
        status.clear();
    }

    /// 获取临时封禁状态
    pub async fn get_temp_unsched_state(
        &self,
        account_id: &uuid::Uuid,
    ) -> Option<TempUnschedState> {
        let status = self.health_status.read().await;

        status.get(account_id).and_then(|health| {
            health.temp_unschedulable_until.map(|until| {
                let now = Instant::now();
                let remaining = if until > now {
                    until.duration_since(now)
                } else {
                    Duration::ZERO
                };

                TempUnschedState {
                    until_unix: Utc::now().timestamp() + remaining.as_secs() as i64,
                    triggered_at_unix: Utc::now().timestamp() - 60, // 简化，实际应记录
                    status_code: 0,                                 // 简化，实际应记录
                    matched_keyword: None,
                    reason: health.temp_unschedulable_reason.clone().unwrap_or_default(),
                }
            })
        })
    }

    /// 清除临时封禁
    pub async fn clear_temp_unschedule(&self, account_id: &uuid::Uuid) {
        let mut status = self.health_status.write().await;

        if let Some(health) = status.get_mut(account_id) {
            health.temp_unschedulable_until = None;
            health.temp_unschedulable_reason = None;
        }
    }
}

/// 故障转移错误 (旧版，保留兼容性)
#[derive(Debug)]
pub struct FailoverError {
    pub attempts: usize,
    pub last_status: u16,
    pub last_message: String,
    pub account_errors: Vec<(uuid::Uuid, String)>,
}

impl std::fmt::Display for FailoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failover exhausted after {} attempts. Last error (HTTP {}): {}",
            self.attempts, self.last_status, self.last_message
        )
    }
}

impl std::error::Error for FailoverError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upstream_failover_error() {
        let err = UpstreamFailoverError::new(500, b"Internal Server Error".to_vec());
        assert_eq!(err.status_code, 500);
        assert!(!err.retryable_on_same_account);

        let err = UpstreamFailoverError::retryable(400, b"Bad Request".to_vec());
        assert_eq!(err.status_code, 400);
        assert!(err.retryable_on_same_account);
    }

    #[test]
    fn test_upstream_failover_error_extract_message() {
        let json = r#"{"error": {"message": "context length exceeded"}}"#;
        let err = UpstreamFailoverError::new(400, json.as_bytes().to_vec());
        assert_eq!(
            err.extract_message(),
            Some("context length exceeded".to_string())
        );

        let text = "plain text error message";
        let err = UpstreamFailoverError::new(500, text.as_bytes().to_vec());
        assert_eq!(err.extract_message(), Some(text.to_string()));
    }

    #[test]
    fn test_temp_unsched_state() {
        let state = TempUnschedState {
            until_unix: Utc::now().timestamp() + 60,
            triggered_at_unix: Utc::now().timestamp(),
            status_code: 400,
            matched_keyword: Some("config error".to_string()),
            reason: "config error".to_string(),
        };

        assert!(state.is_active());
        assert!(state.remaining_duration() > Duration::ZERO);
    }

    #[tokio::test]
    async fn test_failover_manager_temp_unschedule() {
        let manager = FailoverManager::new(FailoverConfig::default());
        let account_id = uuid::Uuid::new_v4();

        // 临时封禁
        manager
            .temp_unschedule(
                &account_id,
                Duration::from_secs(60),
                "test reason".to_string(),
            )
            .await;

        // 检查是否不健康
        assert!(!manager.is_account_healthy(&account_id).await);

        // 清除封禁
        manager.clear_temp_unschedule(&account_id).await;

        // 检查是否恢复健康
        assert!(manager.is_account_healthy(&account_id).await);
    }

    #[tokio::test]
    async fn test_failover_manager_temp_unschedule_retryable_error() {
        let manager = FailoverManager::new(FailoverConfig::default());
        let account_id = uuid::Uuid::new_v4();

        // 先标记成功，确保账号存在
        manager.mark_success(account_id).await;

        // 触发 400 错误的临时封禁
        let err = UpstreamFailoverError::retryable(400, b"invalid project resource name".to_vec());
        manager
            .temp_unschedule_retryable_error(&account_id, &err)
            .await;

        // 检查是否不健康
        assert!(!manager.is_account_healthy(&account_id).await);
    }
}
