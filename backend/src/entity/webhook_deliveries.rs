//! Webhook Delivery Entity
//!
//! Webhook 投递记录，用于跟踪每次发送的状态和结果

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Webhook 投递状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// 等待发送
    Pending,
    /// 发送成功
    Success,
    /// 发送失败（不再重试）
    Failed,
    /// 重试中
    Retrying,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Retrying => "retrying",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "retrying" => Self::Retrying,
            _ => Self::Pending,
        }
    }
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_deliveries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 关联的端点 ID
    pub endpoint_id: i64,
    /// 事件类型
    pub event_type: String,
    /// 事件负载
    pub payload: JsonValue,
    /// 投递状态
    pub status: String,
    /// HTTP 响应码
    pub response_code: Option<i32>,
    /// HTTP 响应体
    pub response_body: Option<String>,
    /// 已尝试次数
    pub attempts: i32,
    /// 最大尝试次数
    pub max_attempts: i32,
    /// 下次重试时间
    pub next_retry_at: Option<DateTime<Utc>>,
    /// 成功投递时间
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::webhook_endpoints::Entity",
        from = "Column::EndpointId",
        to = "super::webhook_endpoints::Column::Id"
    )]
    Endpoint,
}

impl Related<super::webhook_endpoints::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Endpoint.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 获取状态枚举
    pub fn get_status(&self) -> DeliveryStatus {
        DeliveryStatus::parse(&self.status)
    }

    /// 检查是否可以重试
    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_attempts
            && matches!(
                self.get_status(),
                DeliveryStatus::Pending | DeliveryStatus::Retrying
            )
    }

    /// 检查是否需要立即重试
    pub fn needs_retry_now(&self) -> bool {
        if !self.can_retry() {
            return false;
        }

        if let Some(next_retry) = self.next_retry_at {
            return next_retry <= Utc::now();
        }

        // 如果没有设置下次重试时间，表示需要立即重试
        true
    }

    /// 检查是否已完成（成功或永久失败）
    pub fn is_completed(&self) -> bool {
        matches!(
            self.get_status(),
            DeliveryStatus::Success | DeliveryStatus::Failed
        )
    }

    /// 检查是否成功
    pub fn is_success(&self) -> bool {
        self.get_status() == DeliveryStatus::Success
    }

    /// 获取响应摘要（用于日志）
    pub fn response_summary(&self) -> String {
        match (self.response_code, &self.response_body) {
            (Some(code), Some(body)) => {
                let truncated = if body.len() > 200 {
                    format!("{}...", &body[..200])
                } else {
                    body.clone()
                };
                format!("HTTP {}: {}", code, truncated)
            }
            (Some(code), None) => format!("HTTP {} (no body)", code),
            (None, _) => "No response".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delivery_status_parsing() {
        assert_eq!(DeliveryStatus::parse("pending"), DeliveryStatus::Pending);
        assert_eq!(DeliveryStatus::parse("success"), DeliveryStatus::Success);
        assert_eq!(DeliveryStatus::parse("failed"), DeliveryStatus::Failed);
        assert_eq!(DeliveryStatus::parse("retrying"), DeliveryStatus::Retrying);
        assert_eq!(DeliveryStatus::parse("invalid"), DeliveryStatus::Pending);
    }

    #[test]
    fn test_can_retry() {
        let delivery = Model {
            id: 1,
            endpoint_id: 1,
            event_type: "account.created".to_string(),
            payload: serde_json::json!({}),
            status: "retrying".to_string(),
            response_code: None,
            response_body: None,
            attempts: 2,
            max_attempts: 5,
            next_retry_at: Some(Utc::now()),
            delivered_at: None,
            created_at: Utc::now(),
        };

        assert!(delivery.can_retry());

        let max_delivery = Model {
            attempts: 5,
            ..delivery.clone()
        };
        assert!(!max_delivery.can_retry());
    }

    #[test]
    fn test_needs_retry_now() {
        let delivery = Model {
            id: 1,
            endpoint_id: 1,
            event_type: "account.created".to_string(),
            payload: serde_json::json!({}),
            status: "retrying".to_string(),
            response_code: None,
            response_body: None,
            attempts: 2,
            max_attempts: 5,
            next_retry_at: Some(Utc::now() - chrono::Duration::seconds(10)),
            delivered_at: None,
            created_at: Utc::now(),
        };

        assert!(delivery.needs_retry_now());

        let future_delivery = Model {
            next_retry_at: Some(Utc::now() + chrono::Duration::seconds(60)),
            ..delivery.clone()
        };
        assert!(!future_delivery.needs_retry_now());
    }

    #[test]
    fn test_is_completed() {
        let success_delivery = Model {
            id: 1,
            endpoint_id: 1,
            event_type: "account.created".to_string(),
            payload: serde_json::json!({}),
            status: "success".to_string(),
            response_code: Some(200),
            response_body: Some("OK".to_string()),
            attempts: 1,
            max_attempts: 5,
            next_retry_at: None,
            delivered_at: Some(Utc::now()),
            created_at: Utc::now(),
        };

        assert!(success_delivery.is_completed());
        assert!(success_delivery.is_success());

        let pending_delivery = Model {
            status: "pending".to_string(),
            ..success_delivery.clone()
        };
        assert!(!pending_delivery.is_completed());
    }
}
