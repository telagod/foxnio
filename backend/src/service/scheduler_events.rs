//! 调度器事件服务
//!
//! 定义和处理调度器相关的事件

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 调度器事件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SchedulerEventType {
    AccountChanged,
    AccountGroupsChanged,
    AccountBulkChanged,
    AccountLastUsed,
    GroupChanged,
    FullRebuild,
}

impl SchedulerEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccountChanged => "account_changed",
            Self::AccountGroupsChanged => "account_groups_changed",
            Self::AccountBulkChanged => "account_bulk_changed",
            Self::AccountLastUsed => "account_last_used",
            Self::GroupChanged => "group_changed",
            Self::FullRebuild => "full_rebuild",
        }
    }
}

/// 调度器事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerEvent {
    pub id: Uuid,
    pub event_type: SchedulerEventType,
    pub account_id: Option<i64>,
    pub group_id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub processed: bool,
    pub processed_at: Option<DateTime<Utc>>,
}

impl SchedulerEvent {
    /// 创建账号变更事件
    pub fn account_changed(account_id: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::AccountChanged,
            account_id: Some(account_id),
            group_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({}),
            processed: false,
            processed_at: None,
        }
    }

    /// 创建账号分组变更事件
    pub fn account_groups_changed(account_id: i64, group_ids: Vec<i64>) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::AccountGroupsChanged,
            account_id: Some(account_id),
            group_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({ "group_ids": group_ids }),
            processed: false,
            processed_at: None,
        }
    }

    /// 创建批量账号变更事件
    pub fn account_bulk_changed(account_ids: Vec<i64>) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::AccountBulkChanged,
            account_id: None,
            group_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({ "account_ids": account_ids }),
            processed: false,
            processed_at: None,
        }
    }

    /// 创建账号最后使用事件
    pub fn account_last_used(account_id: i64, model: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::AccountLastUsed,
            account_id: Some(account_id),
            group_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({ "model": model }),
            processed: false,
            processed_at: None,
        }
    }

    /// 创建分组变更事件
    pub fn group_changed(group_id: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::GroupChanged,
            account_id: None,
            group_id: Some(group_id),
            timestamp: Utc::now(),
            payload: serde_json::json!({}),
            processed: false,
            processed_at: None,
        }
    }

    /// 创建完全重建事件
    pub fn full_rebuild(reason: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: SchedulerEventType::FullRebuild,
            account_id: None,
            group_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({ "reason": reason }),
            processed: false,
            processed_at: None,
        }
    }

    /// 标记为已处理
    pub fn mark_processed(&mut self) {
        self.processed = true;
        self.processed_at = Some(Utc::now());
    }
}

/// 事件处理器特质
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &SchedulerEvent) -> anyhow::Result<()>;
}

/// 事件分发器
pub struct EventDispatcher {
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// 注册处理器
    pub fn register<H: EventHandler + 'static>(&mut self, handler: H) {
        self.handlers.push(Box::new(handler));
    }

    /// 分发事件
    pub async fn dispatch(&self, event: &SchedulerEvent) -> anyhow::Result<()> {
        for handler in &self.handlers {
            handler.handle(event).await?;
        }
        Ok(())
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = SchedulerEvent::account_changed(123);
        assert_eq!(event.event_type, SchedulerEventType::AccountChanged);
        assert_eq!(event.account_id, Some(123));
        assert!(!event.processed);
    }

    #[test]
    fn test_event_type_str() {
        assert_eq!(
            SchedulerEventType::AccountChanged.as_str(),
            "account_changed"
        );
        assert_eq!(SchedulerEventType::FullRebuild.as_str(), "full_rebuild");
    }

    #[test]
    fn test_mark_processed() {
        let mut event = SchedulerEvent::account_changed(123);
        event.mark_processed();
        assert!(event.processed);
        assert!(event.processed_at.is_some());
    }

    #[test]
    fn test_account_groups_changed() {
        let event = SchedulerEvent::account_groups_changed(123, vec![1, 2, 3]);
        assert_eq!(event.event_type, SchedulerEventType::AccountGroupsChanged);
        assert!(event.payload["group_ids"].is_array());
    }

    #[test]
    fn test_account_bulk_changed() {
        let event = SchedulerEvent::account_bulk_changed(vec![1, 2, 3]);
        assert_eq!(event.event_type, SchedulerEventType::AccountBulkChanged);
        assert!(event.payload["account_ids"].is_array());
    }
}
