//! 故障转移实现

use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::entity::accounts;

/// 故障转移配置
pub struct FailoverConfig {
    /// 最大重试次数
    pub max_retries: usize,
    /// 重试间隔 (毫秒)
    pub retry_delay_ms: u64,
    /// 指数退避基数
    pub backoff_base: f64,
    /// 最大退避时间 (毫秒)
    pub max_backoff_ms: u64,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            backoff_base: 2.0,
            max_backoff_ms: 5000,
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
        F: FnMut(&accounts::Model) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>,
    {
        let mut last_error = None;
        let mut attempts = 0;
        
        for account in &accounts {
            // 检查账号健康状态
            if !self.is_account_healthy(&account.id).await {
                continue;
            }
            
            // 计算退避时间
            if attempts > 0 {
                let delay = self.calculate_backoff(attempts);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            // 执行请求
            match request_fn(account).await {
                Ok(result) => {
                    // 标记成功
                    self.mark_success(account.id).await;
                    return Ok((result, account.id));
                }
                Err(e) => {
                    // 标记失败
                    self.mark_failure(account.id, e.to_string()).await;
                    last_error = Some(e);
                    attempts += 1;
                    
                    if attempts >= self.config.max_retries {
                        break;
                    }
                }
            }
        }
        
        // 所有账号都失败
        bail!(
            "All accounts failed after {} attempts. Last error: {}",
            attempts,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        )
    }

    /// 检查账号是否健康
    pub async fn is_account_healthy(&self, account_id: &uuid::Uuid) -> bool {
        let status = self.health_status.read().await;
        
        if let Some(health) = status.get(account_id) {
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
        });
        
        health.is_healthy = true;
        health.last_success = Some(Instant::now());
        health.consecutive_failures = 0;
        health.last_error = None;
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
        });
        
        health.is_healthy = false;
        health.last_failure = Some(Instant::now());
        health.consecutive_failures += 1;
        health.last_error = Some(error);
    }

    /// 计算退避时间
    fn calculate_backoff(&self, attempt: usize) -> u64 {
        let backoff = self.config.retry_delay_ms as f64 
            * self.config.backoff_base.powi(attempt as i32);
        
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
}

/// 故障转移错误
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
