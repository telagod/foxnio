//! 幂等性核心服务
//!
//! 确保请求的幂等性，防止重复处理

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 幂等性状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IdempotencyStatus {
    Processing,
    Succeeded,
    FailedRetryable,
}

impl std::fmt::Display for IdempotencyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Processing => write!(f, "processing"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::FailedRetryable => write!(f, "failed_retryable"),
        }
    }
}

/// 幂等性记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    pub id: i64,
    pub scope: String,
    pub idempotency_key_hash: String,
    pub request_fingerprint: String,
    pub status: String,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub error_reason: Option<String>,
    pub locked_until: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 幂等性配置
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    pub default_ttl_seconds: u64,
    pub system_operation_ttl_seconds: u64,
    pub processing_timeout_seconds: u64,
    pub failed_retry_backoff_seconds: u64,
    pub max_stored_response_len: usize,
    pub observe_only: bool,
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 24 * 3600,     // 24 小时
            system_operation_ttl_seconds: 3600, // 1 小时
            processing_timeout_seconds: 30,     // 30 秒
            failed_retry_backoff_seconds: 5,    // 5 秒
            max_stored_response_len: 64 * 1024, // 64 KB
            observe_only: true,                 // 默认先观察再强制
        }
    }
}

/// 执行选项
#[derive(Debug, Clone)]
pub struct IdempotencyExecuteOptions {
    pub scope: String,
    pub actor_scope: String,
    pub method: String,
    pub route: String,
    pub idempotency_key: String,
    pub payload: serde_json::Value,
    pub ttl: Option<std::time::Duration>,
    pub require_key: bool,
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct IdempotencyExecuteResult<T> {
    pub data: T,
    pub replayed: bool,
}

/// 幂等性协调器
pub struct IdempotencyCoordinator {
    config: IdempotencyConfig,
    records: HashMap<String, IdempotencyRecord>,
}

impl IdempotencyCoordinator {
    /// 创建新的幂等性协调器
    pub fn new(config: IdempotencyConfig) -> Self {
        Self {
            config,
            records: HashMap::new(),
        }
    }

    /// 执行幂等操作
    pub async fn execute<T, F, Fut>(
        &self,
        options: IdempotencyExecuteOptions,
        operation: F,
    ) -> Result<IdempotencyExecuteResult<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // 1. 验证幂等性键
        if options.idempotency_key.is_empty() {
            if options.require_key {
                return Err(anyhow!("Idempotency key required"));
            }
            // 无需幂等性，直接执行
            let data = operation().await?;
            return Ok(IdempotencyExecuteResult {
                data,
                replayed: false,
            });
        }

        // 2. 计算键哈希
        let key_hash = Self::hash_key(&options.scope, &options.idempotency_key);

        // 3. 计算请求指纹
        let request_fingerprint = Self::fingerprint_request(&options);

        // 4. 检查现有记录
        if let Some(record) = self.records.get(&key_hash) {
            // 检查请求指纹是否匹配
            if record.request_fingerprint != request_fingerprint {
                return Err(anyhow!("Idempotency key reused with different payload"));
            }

            match record.status.as_str() {
                "processing" => {
                    // 检查是否超时
                    if let Some(locked_until) = record.locked_until {
                        if Utc::now() < locked_until {
                            return Err(anyhow!("Idempotent request is still processing"));
                        }
                    }
                }
                "succeeded" => {
                    // 返回缓存的响应
                    if let Some(body) = &record.response_body {
                        let data: T = serde_json::from_str(body)?;
                        return Ok(IdempotencyExecuteResult {
                            data,
                            replayed: true,
                        });
                    }
                }
                "failed_retryable" => {
                    // 检查退避时间
                    if let Some(locked_until) = record.locked_until {
                        if Utc::now() < locked_until {
                            return Err(anyhow!("Idempotent request is in retry backoff window"));
                        }
                    }
                }
                _ => {}
            }
        }

        // 5. 执行操作
        let data = operation().await?;

        Ok(IdempotencyExecuteResult {
            data,
            replayed: false,
        })
    }

    /// 创建处理中记录
    pub fn create_processing(&mut self, options: &IdempotencyExecuteOptions) -> Result<String> {
        let key_hash = Self::hash_key(&options.scope, &options.idempotency_key);
        let request_fingerprint = Self::fingerprint_request(options);
        let now = Utc::now();
        let locked_until =
            now + chrono::Duration::seconds(self.config.processing_timeout_seconds as i64);
        let expires_at = now
            + chrono::Duration::seconds(
                options
                    .ttl
                    .map(|t| t.as_secs())
                    .unwrap_or(self.config.default_ttl_seconds) as i64,
            );

        let record = IdempotencyRecord {
            id: rand_id(),
            scope: options.scope.clone(),
            idempotency_key_hash: key_hash.clone(),
            request_fingerprint,
            status: IdempotencyStatus::Processing.to_string(),
            response_status: None,
            response_body: None,
            error_reason: None,
            locked_until: Some(locked_until),
            expires_at,
            created_at: now,
            updated_at: now,
        };

        self.records.insert(key_hash.clone(), record);
        Ok(key_hash)
    }

    /// 标记成功
    pub fn mark_succeeded(
        &mut self,
        key_hash: &str,
        response_status: i32,
        response_body: &str,
    ) -> Result<()> {
        if let Some(record) = self.records.get_mut(key_hash) {
            record.status = IdempotencyStatus::Succeeded.to_string();
            record.response_status = Some(response_status);

            // 限制响应体大小
            let body = if response_body.len() > self.config.max_stored_response_len {
                &response_body[..self.config.max_stored_response_len]
            } else {
                response_body
            };
            record.response_body = Some(body.to_string());
            record.locked_until = None;
            record.updated_at = Utc::now();
        }
        Ok(())
    }

    /// 标记失败可重试
    pub fn mark_failed_retryable(&mut self, key_hash: &str, error_reason: &str) -> Result<()> {
        if let Some(record) = self.records.get_mut(key_hash) {
            record.status = IdempotencyStatus::FailedRetryable.to_string();
            record.error_reason = Some(error_reason.to_string());
            record.locked_until = Some(
                Utc::now()
                    + chrono::Duration::seconds(self.config.failed_retry_backoff_seconds as i64),
            );
            record.updated_at = Utc::now();
        }
        Ok(())
    }

    /// 获取记录
    pub fn get_record(&self, key_hash: &str) -> Option<&IdempotencyRecord> {
        self.records.get(key_hash)
    }

    /// 删除过期记录
    pub fn delete_expired(&mut self) -> usize {
        let now = Utc::now();
        let expired_keys: Vec<_> = self
            .records
            .iter()
            .filter(|(_, r)| r.expires_at < now)
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            self.records.remove(&key);
        }
        count
    }

    /// 计算键哈希
    fn hash_key(scope: &str, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(scope.as_bytes());
        hasher.update(b":");
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// 计算请求指纹
    fn fingerprint_request(options: &IdempotencyExecuteOptions) -> String {
        let mut hasher = Sha256::new();
        hasher.update(options.method.as_bytes());
        hasher.update(b":");
        hasher.update(options.route.as_bytes());
        hasher.update(b":");
        hasher.update(
            serde_json::to_string(&options.payload)
                .unwrap_or_default()
                .as_bytes(),
        );
        hex::encode(hasher.finalize())
    }

    /// 获取配置
    pub fn config(&self) -> &IdempotencyConfig {
        &self.config
    }
}

/// 生成随机 ID
fn rand_id() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}

impl Default for IdempotencyCoordinator {
    fn default() -> Self {
        Self::new(IdempotencyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_status_display() {
        assert_eq!(IdempotencyStatus::Processing.to_string(), "processing");
        assert_eq!(IdempotencyStatus::Succeeded.to_string(), "succeeded");
        assert_eq!(
            IdempotencyStatus::FailedRetryable.to_string(),
            "failed_retryable"
        );
    }

    #[test]
    fn test_hash_key() {
        let hash1 = IdempotencyCoordinator::hash_key("scope1", "key1");
        let hash2 = IdempotencyCoordinator::hash_key("scope1", "key1");
        let hash3 = IdempotencyCoordinator::hash_key("scope1", "key2");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex length
    }

    #[tokio::test]
    async fn test_execute_without_key() {
        let coordinator = IdempotencyCoordinator::default();
        let options = IdempotencyExecuteOptions {
            scope: "test".to_string(),
            actor_scope: "user".to_string(),
            method: "POST".to_string(),
            route: "/test".to_string(),
            idempotency_key: String::new(),
            payload: serde_json::json!({}),
            ttl: None,
            require_key: false,
        };

        let result: IdempotencyExecuteResult<String> = coordinator
            .execute(options, || async { Ok("success".to_string()) })
            .await
            .unwrap();

        assert_eq!(result.data, "success");
        assert!(!result.replayed);
    }

    #[test]
    fn test_config_default() {
        let config = IdempotencyConfig::default();
        assert_eq!(config.default_ttl_seconds, 24 * 3600);
        assert!(config.observe_only);
    }
}
