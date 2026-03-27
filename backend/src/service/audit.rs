//! 审计日志服务 - Audit Service
//!
//! 提供审计日志的记录、查询和管理功能

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, PaginatorTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::entity::audit_logs;
use crate::entity::audit_logs::{AuditAction, SanitizedAuditLog};

/// 审计日志条目（用于创建）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub user_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_data: Option<JsonValue>,
    pub response_status: Option<i32>,
}

impl AuditEntry {
    /// 创建用户登录审计条目
    pub fn user_login(user_id: Uuid, ip: Option<String>, ua: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::UserLogin.as_str().to_string(),
            resource_type: Some("user".to_string()),
            resource_id: Some(user_id.to_string()),
            ip_address: ip,
            user_agent: ua,
            request_data: None,
            response_status: Some(200),
        }
    }

    /// 创建用户登出审计条目
    pub fn user_logout(user_id: Uuid, ip: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::UserLogout.as_str().to_string(),
            resource_type: Some("user".to_string()),
            resource_id: Some(user_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data: None,
            response_status: Some(200),
        }
    }

    /// 创建用户注册审计条目
    pub fn user_register(user_id: Uuid, ip: Option<String>, ua: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::UserRegister.as_str().to_string(),
            resource_type: Some("user".to_string()),
            resource_id: Some(user_id.to_string()),
            ip_address: ip,
            user_agent: ua,
            request_data: None,
            response_status: Some(201),
        }
    }

    /// 创建密码修改审计条目
    pub fn password_change(user_id: Uuid, ip: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::PasswordChange.as_str().to_string(),
            resource_type: Some("user".to_string()),
            resource_id: Some(user_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data: None,
            response_status: Some(200),
        }
    }

    /// 创建 API Key 创建审计条目
    pub fn api_key_create(user_id: Uuid, key_id: Uuid, ip: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::ApiKeyCreate.as_str().to_string(),
            resource_type: Some("api_key".to_string()),
            resource_id: Some(key_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data: None,
            response_status: Some(201),
        }
    }

    /// 创建 API Key 删除审计条目
    pub fn api_key_delete(user_id: Uuid, key_id: Uuid, ip: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::ApiKeyDelete.as_str().to_string(),
            resource_type: Some("api_key".to_string()),
            resource_id: Some(key_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data: None,
            response_status: Some(200),
        }
    }

    /// 创建管理员操作审计条目
    pub fn admin_action(
        admin_id: Uuid,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        ip: Option<String>,
        request_data: Option<JsonValue>,
    ) -> Self {
        Self {
            user_id: Some(admin_id),
            action: AuditAction::AdminAction.as_str().to_string(),
            resource_type: Some(resource_type.to_string()),
            resource_id: Some(resource_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data,
            response_status: Some(200),
        }
    }

    /// 创建账户更新审计条目
    pub fn account_update(user_id: Uuid, account_id: Uuid, ip: Option<String>) -> Self {
        Self {
            user_id: Some(user_id),
            action: AuditAction::AccountUpdate.as_str().to_string(),
            resource_type: Some("account".to_string()),
            resource_id: Some(account_id.to_string()),
            ip_address: ip,
            user_agent: None,
            request_data: None,
            response_status: Some(200),
        }
    }
}

/// 审计日志查询过滤器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub user_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

/// 审计日志服务
pub struct AuditService {
    db: DatabaseConnection,
    tx: mpsc::Sender<AuditEntry>,
}

impl AuditService {
    /// 创建新的审计服务
    pub fn new(db: DatabaseConnection) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(1000);
        let db_clone = db.clone();

        // 启动异步写入任务
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                if let Err(e) = Self::write_log(&db_clone, entry).await {
                    tracing::error!("Failed to write audit log: {}", e);
                }
            }
        });

        Self { db, tx }
    }

    /// 异步记录审计日志（不阻塞请求）
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        // 如果通道已满，丢弃日志（避免阻塞）
        if self.tx.try_send(entry).is_err() {
            tracing::warn!("Audit log channel full, dropping log entry");
        }
        Ok(())
    }

    /// 同步记录审计日志（阻塞等待写入完成）
    pub async fn log_sync(&self, entry: AuditEntry) -> Result<()> {
        Self::write_log(&self.db, entry).await
    }

    /// 实际写入数据库
    async fn write_log(db: &DatabaseConnection, entry: AuditEntry) -> Result<()> {
        let now = Utc::now();
        let log = audit_logs::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(entry.user_id),
            action: Set(entry.action),
            resource_type: Set(entry.resource_type),
            resource_id: Set(entry.resource_id),
            ip_address: Set(entry.ip_address),
            user_agent: Set(entry.user_agent),
            request_data: Set(entry.request_data),
            response_status: Set(entry.response_status),
            created_at: Set(now),
        };

        log.insert(db).await?;
        Ok(())
    }

    /// 查询审计日志列表
    pub async fn list(&self, filter: AuditFilter) -> Result<Vec<audit_logs::Model>> {
        let page = filter.page.unwrap_or(1);
        let page_size = filter.page_size.unwrap_or(50).min(100);

        let mut query = audit_logs::Entity::find();

        if let Some(user_id) = filter.user_id {
            query = query.filter(audit_logs::Column::UserId.eq(user_id));
        }
        if let Some(action) = &filter.action {
            query = query.filter(audit_logs::Column::Action.eq(action));
        }
        if let Some(resource_type) = &filter.resource_type {
            query = query.filter(audit_logs::Column::ResourceType.eq(resource_type));
        }
        if let Some(resource_id) = &filter.resource_id {
            query = query.filter(audit_logs::Column::ResourceId.eq(resource_id));
        }
        if let Some(ip) = &filter.ip_address {
            query = query.filter(audit_logs::Column::IpAddress.eq(ip));
        }
        if let Some(start) = filter.start_time {
            query = query.filter(audit_logs::Column::CreatedAt.gte(start));
        }
        if let Some(end) = filter.end_time {
            query = query.filter(audit_logs::Column::CreatedAt.lte(end));
        }

        let logs = query
            .order_by_desc(audit_logs::Column::CreatedAt)
            .paginate(&self.db, page_size)
            .fetch_page(page.saturating_sub(1))
            .await?;

        Ok(logs)
    }

    /// 查询用户的审计日志
    pub async fn get_user_logs(&self, user_id: Uuid, page: u64, page_size: u64) -> Result<Vec<audit_logs::Model>> {
        let page_size = page_size.min(100);

        let logs = audit_logs::Entity::find()
            .filter(audit_logs::Column::UserId.eq(user_id))
            .order_by_desc(audit_logs::Column::CreatedAt)
            .paginate(&self.db, page_size)
            .fetch_page(page.saturating_sub(1))
            .await?;

        Ok(logs)
    }

    /// 统计审计日志数量
    pub async fn count(&self, filter: AuditFilter) -> Result<u64> {
        let mut query = audit_logs::Entity::find();

        if let Some(user_id) = filter.user_id {
            query = query.filter(audit_logs::Column::UserId.eq(user_id));
        }
        if let Some(action) = &filter.action {
            query = query.filter(audit_logs::Column::Action.eq(action));
        }
        if let Some(resource_type) = &filter.resource_type {
            query = query.filter(audit_logs::Column::ResourceType.eq(resource_type));
        }
        if let Some(start) = filter.start_time {
            query = query.filter(audit_logs::Column::CreatedAt.gte(start));
        }
        if let Some(end) = filter.end_time {
            query = query.filter(audit_logs::Column::CreatedAt.lte(end));
        }

        let count = query.count(&self.db).await?;
        Ok(count)
    }

    /// 获取敏感操作日志
    pub async fn get_sensitive_logs(&self, page: u64, page_size: u64) -> Result<Vec<audit_logs::Model>> {
        let sensitive_actions = [
            AuditAction::UserLogin.as_str(),
            AuditAction::PasswordChange.as_str(),
            AuditAction::ApiKeyCreate.as_str(),
            AuditAction::ApiKeyDelete.as_str(),
            AuditAction::AdminAction.as_str(),
            AuditAction::SecurityAlert.as_str(),
        ];

        let logs = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.is_in(sensitive_actions))
            .order_by_desc(audit_logs::Column::CreatedAt)
            .paginate(&self.db, page_size.min(100))
            .fetch_page(page.saturating_sub(1))
            .await?;

        Ok(logs)
    }

    /// 清理过期的审计日志
    pub async fn cleanup_old_logs(&self, days_to_keep: i64) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(days_to_keep);
        
        let result = audit_logs::Entity::delete_many()
            .filter(audit_logs::Column::CreatedAt.lt(cutoff))
            .exec(&self.db)
            .await?;

        Ok(result.rows_affected)
    }
}

/// 异步审计日志记录器（全局单例）
pub struct AsyncAuditLogger {
    tx: mpsc::Sender<AuditEntry>,
}

impl AsyncAuditLogger {
    /// 创建新的异步记录器
    pub fn new(db: DatabaseConnection) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(1000);

        // 启动异步写入任务
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                if let Err(e) = AuditService::write_log(&db, entry).await {
                    tracing::error!("Failed to write audit log: {}", e);
                }
            }
        });

        Self { tx }
    }

    /// 异步记录日志
    pub fn log(&self, entry: AuditEntry) {
        if self.tx.try_send(entry).is_err() {
            tracing::warn!("Audit log channel full, dropping log entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_creation() {
        let user_id = Uuid::new_v4();
        let entry = AuditEntry::user_login(user_id, Some("192.168.1.1".to_string()), None);
        
        assert_eq!(entry.user_id, Some(user_id));
        assert_eq!(entry.action, "USER_LOGIN");
        assert_eq!(entry.resource_type, Some("user".to_string()));
    }

    #[test]
    fn test_audit_filter() {
        let filter = AuditFilter {
            user_id: Some(Uuid::new_v4()),
            action: Some("USER_LOGIN".to_string()),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        };

        assert!(filter.user_id.is_some());
        assert_eq!(filter.action, Some("USER_LOGIN".to_string()));
    }

    #[test]
    fn test_sanitized_log() {
        let user_id = Uuid::new_v4();
        let entry = AuditEntry::user_login(
            user_id,
            Some("192.168.1.100".to_string()),
            Some("Mozilla/5.0".to_string()),
        );

        assert_eq!(entry.ip_address, Some("192.168.1.100".to_string()));
    }
}
