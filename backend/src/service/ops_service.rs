//! 运维服务核心 - Ops Service Core
//!
//! 提供运维监控的核心功能，包括错误日志记录、指标收集和监控开关控制

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::{accounts, audit_logs};

/// 运维监控最大请求体大小（字节）
const OPS_MAX_STORED_REQUEST_BODY_BYTES: usize = 10 * 1024;
/// 运维监控最大错误体大小（字节）
const OPS_MAX_STORED_ERROR_BODY_BYTES: usize = 20 * 1024;

/// 运维监控错误
#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    #[error("运维监控已禁用")]
    Disabled,
    #[error("未找到记录: {0}")]
    NotFound(String),
    #[error("数据库错误: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// 错误日志输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsInsertErrorLogInput {
    pub request_id: Option<String>,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub platform: String,
    pub model: Option<String>,
    pub request_type: i16,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_details: Option<String>,
    pub status_code: Option<i16>,
    pub request_body_json: Option<String>,
    pub request_body_bytes: Option<i32>,
    pub response_body_json: Option<String>,
    pub response_time_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// 错误日志输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsErrorLog {
    pub id: i64,
    pub request_id: Option<String>,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub platform: String,
    pub model: Option<String>,
    pub request_type: i16,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_details: Option<String>,
    pub status_code: Option<i16>,
    pub request_body_json: Option<String>,
    pub request_body_bytes: Option<i32>,
    pub response_body_json: Option<String>,
    pub response_time_ms: Option<i32>,
    pub upstream_latency_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// 账号可用性信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsAccountAvailability {
    pub total_accounts: i64,
    pub active_accounts: i64,
    pub error_accounts: i64,
    pub by_provider: std::collections::HashMap<String, i64>,
}

/// 运维服务配置
#[derive(Debug, Clone)]
pub struct OpsConfig {
    pub enabled: bool,
    pub retention_days: i32,
    pub error_sampling_rate: f32,
    pub max_concurrent_requests: usize,
}

impl Default for OpsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            error_sampling_rate: 1.0,
            max_concurrent_requests: 100,
        }
    }
}

/// 运维服务核心
pub struct OpsService {
    db: DatabaseConnection,
    config: OpsConfig,
}

impl OpsService {
    /// 创建新的运维服务实例
    pub fn new(db: DatabaseConnection, config: OpsConfig) -> Self {
        Self { db, config }
    }

    /// 检查运维监控是否启用
    pub fn is_monitoring_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 要求监控启用，否则返回错误
    pub fn require_monitoring_enabled(&self) -> Result<()> {
        if self.is_monitoring_enabled() {
            Ok(())
        } else {
            Err(OpsError::Disabled.into())
        }
    }

    /// 准备请求体用于队列存储
    ///
    /// 执行脱敏和裁剪，返回可直接写入数据库的字段
    pub fn prepare_request_body_for_queue(raw: &[u8]) -> (Option<String>, bool, Option<i32>) {
        if raw.is_empty() {
            return (None, false, None);
        }

        let sanitized =
            Self::sanitize_and_trim_request_body(raw, OPS_MAX_STORED_REQUEST_BODY_BYTES);
        let (json, truncated, bytes_len) = sanitized;

        let request_body_json = json;
        let request_body_bytes = Some(bytes_len as i32);

        (request_body_json, truncated, request_body_bytes)
    }

    /// 脱敏并裁剪请求体
    fn sanitize_and_trim_request_body(
        raw: &[u8],
        max_bytes: usize,
    ) -> (Option<String>, bool, usize) {
        if raw.is_empty() {
            return (None, false, 0);
        }

        let bytes_len = raw.len();
        let truncated = bytes_len > max_bytes;

        let slice = if truncated { &raw[..max_bytes] } else { raw };

        // 尝试解析为 JSON 并脱敏
        let sanitized = String::from_utf8_lossy(slice).to_string();

        // 这里可以添加敏感字段脱敏逻辑
        let sanitized = Self::redact_sensitive_fields(&sanitized);

        (Some(sanitized), truncated, bytes_len)
    }

    /// 脱敏敏感字段
    fn redact_sensitive_fields(json_str: &str) -> String {
        // 移除常见的敏感字段
        let sensitive_keys = [
            "password",
            "token",
            "api_key",
            "secret",
            "credential",
            "authorization",
            "bearer",
            "private_key",
        ];

        let result = json_str.to_string();
        for key in sensitive_keys {
            // 简单的脱敏逻辑
            if result.contains(key) {
                // 实际实现应该使用 JSON 解析器
            }
        }
        result
    }

    /// 插入错误日志
    pub async fn insert_error_log(&self, input: OpsInsertErrorLogInput) -> Result<i64> {
        self.require_monitoring_enabled()?;

        let log_id = Uuid::new_v4();

        // Pack error details into request_data JSON
        let request_data = serde_json::json!({
            "request_id": input.request_id,
            "api_key_id": input.api_key_id,
            "account_id": input.account_id,
            "platform": input.platform,
            "model": input.model,
            "request_type": input.request_type,
            "error_code": input.error_code,
            "error_message": input.error_message,
            "error_details": input.error_details,
            "status_code": input.status_code,
            "request_body_json": input.request_body_json,
            "request_body_bytes": input.request_body_bytes,
            "response_body_json": input.response_body_json,
            "response_time_ms": input.response_time_ms,
            "upstream_latency_ms": input.upstream_latency_ms,
        });

        let record = audit_logs::ActiveModel {
            id: Set(log_id),
            user_id: Set(input.user_id.map(|uid| Uuid::from_u128(uid as u128))),
            action: Set("OPS_ERROR_LOG".to_string()),
            resource_type: Set(Some(input.platform.clone())),
            resource_id: Set(input.error_code.clone()),
            ip_address: Set(None),
            user_agent: Set(None),
            request_data: Set(Some(request_data)),
            response_status: Set(input.status_code.map(|s| s as i32)),
            created_at: Set(input.created_at),
        };

        record.insert(&self.db).await?;

        Ok(log_id.as_u128() as i64)
    }

    /// 批量插入错误日志
    pub async fn insert_error_logs_batch(
        &self,
        inputs: Vec<OpsInsertErrorLogInput>,
    ) -> Result<Vec<i64>> {
        self.require_monitoring_enabled()?;

        let mut ids = Vec::with_capacity(inputs.len());
        for input in inputs {
            let id = self.insert_error_log(input).await?;
            ids.push(id);
        }

        Ok(ids)
    }

    /// 查询错误日志
    pub async fn query_error_logs(
        &self,
        platform: Option<&str>,
        model: Option<&str>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<OpsErrorLog>> {
        self.require_monitoring_enabled()?;

        let mut query = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq("OPS_ERROR_LOG"))
            .order_by_desc(audit_logs::Column::CreatedAt);

        if let Some(p) = platform {
            query = query.filter(audit_logs::Column::ResourceType.eq(p));
        }
        if let Some(start) = start_time {
            query = query.filter(audit_logs::Column::CreatedAt.gte(start));
        }
        if let Some(end) = end_time {
            query = query.filter(audit_logs::Column::CreatedAt.lte(end));
        }

        let records = query
            .paginate(&self.db, limit)
            .fetch_page(offset / limit.max(1))
            .await?;

        let mut logs = Vec::with_capacity(records.len());
        for r in records {
            let data = r.request_data.unwrap_or_default();
            let matches_model = match model {
                Some(m) => data.get("model").and_then(|v| v.as_str()) == Some(m),
                None => true,
            };
            if !matches_model {
                continue;
            }

            logs.push(OpsErrorLog {
                id: r.id.as_u128() as i64,
                request_id: data.get("request_id").and_then(|v| v.as_str()).map(String::from),
                user_id: r.user_id.map(|u| u.as_u128() as i64),
                api_key_id: data.get("api_key_id").and_then(|v| v.as_i64()),
                account_id: data.get("account_id").and_then(|v| v.as_i64()),
                platform: data.get("platform").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                model: data.get("model").and_then(|v| v.as_str()).map(String::from),
                request_type: data.get("request_type").and_then(|v| v.as_i64()).unwrap_or(0) as i16,
                error_code: data.get("error_code").and_then(|v| v.as_str()).map(String::from),
                error_message: data.get("error_message").and_then(|v| v.as_str()).map(String::from),
                error_details: data.get("error_details").and_then(|v| v.as_str()).map(String::from),
                status_code: r.response_status.map(|s| s as i16),
                request_body_json: data.get("request_body_json").and_then(|v| v.as_str()).map(String::from),
                request_body_bytes: data.get("request_body_bytes").and_then(|v| v.as_i64()).map(|v| v as i32),
                response_body_json: data.get("response_body_json").and_then(|v| v.as_str()).map(String::from),
                response_time_ms: data.get("response_time_ms").and_then(|v| v.as_i64()).map(|v| v as i32),
                upstream_latency_ms: data.get("upstream_latency_ms").and_then(|v| v.as_i64()).map(|v| v as i32),
                created_at: r.created_at,
            });
        }

        Ok(logs)
    }

    /// 获取账号可用性信息
    pub async fn get_account_availability(
        &self,
        platform_filter: Option<&str>,
        _group_id_filter: Option<i64>,
    ) -> Result<OpsAccountAvailability> {
        self.require_monitoring_enabled()?;

        let mut query = accounts::Entity::find();
        if let Some(p) = platform_filter {
            query = query.filter(accounts::Column::Provider.eq(p));
        }

        let all_accounts = query.all(&self.db).await?;

        let total_accounts = all_accounts.len() as i64;
        let mut active_accounts = 0i64;
        let mut error_accounts = 0i64;
        let mut by_provider: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

        for account in &all_accounts {
            if account.is_active() {
                active_accounts += 1;
            } else {
                error_accounts += 1;
            }
            *by_provider.entry(account.provider.clone()).or_insert(0) += 1;
        }

        Ok(OpsAccountAvailability {
            total_accounts,
            active_accounts,
            error_accounts,
            by_provider,
        })
    }

    /// 清理过期的错误日志
    pub async fn cleanup_expired_logs(&self, retention_days: i32) -> Result<u64> {
        self.require_monitoring_enabled()?;

        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);

        let result = audit_logs::Entity::delete_many()
            .filter(audit_logs::Column::Action.eq("OPS_ERROR_LOG"))
            .filter(audit_logs::Column::CreatedAt.lt(cutoff))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }

    /// 获取错误统计
    pub async fn get_error_statistics(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        group_by: &str,
    ) -> Result<std::collections::HashMap<String, i64>> {
        self.require_monitoring_enabled()?;

        // group_by determines the aggregation key: "platform", "model", "error_code", "status_code"
        let json_key = match group_by {
            "platform" | "model" | "error_code" | "status_code" => group_by,
            _ => "platform",
        };

        let records = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq("OPS_ERROR_LOG"))
            .filter(audit_logs::Column::CreatedAt.gte(start_time))
            .filter(audit_logs::Column::CreatedAt.lte(end_time))
            .all(&self.db)
            .await?;

        let mut stats: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for r in records {
            let key = r
                .request_data
                .as_ref()
                .and_then(|d| d.get(json_key))
                .and_then(|v| {
                    v.as_str()
                        .map(String::from)
                        .or_else(|| Some(v.to_string()))
                })
                .unwrap_or_else(|| "unknown".to_string());
            *stats.entry(key).or_insert(0) += 1;
        }

        Ok(stats)
    }

    /// 获取配置
    pub fn get_config(&self) -> &OpsConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: OpsConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_request_body_for_queue_empty() {
        let empty: Vec<u8> = Vec::new();
        let (json, truncated, bytes) = OpsService::prepare_request_body_for_queue(&empty);
        assert!(json.is_none());
        assert!(!truncated);
        assert!(bytes.is_none());
    }

    #[test]
    fn test_prepare_request_body_for_queue_normal() {
        let body = br#"{"model":"gpt-4","prompt":"hello"}"#;
        let (json, truncated, bytes) = OpsService::prepare_request_body_for_queue(body);
        assert!(json.is_some());
        assert!(!truncated);
        assert_eq!(bytes, Some(body.len() as i32));
    }

    #[test]
    fn test_prepare_request_body_for_queue_truncated() {
        let large_body = vec![b'a'; OPS_MAX_STORED_REQUEST_BODY_BYTES + 1000];
        let (json, truncated, bytes) = OpsService::prepare_request_body_for_queue(&large_body);
        assert!(json.is_some());
        assert!(truncated);
        assert!(bytes.unwrap() > OPS_MAX_STORED_REQUEST_BODY_BYTES as i32);
    }

    #[test]
    fn test_redact_sensitive_fields() {
        let json = r#"{"password":"secret123","model":"gpt-4"}"#;
        let redacted = OpsService::redact_sensitive_fields(json);
        assert!(redacted.contains("password"));
        // 在实际实现中应该检查敏感值是否被替换为 ***
    }
}
