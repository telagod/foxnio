//! Webhook Endpoint Entity
//!
//! Webhook 端点配置，用于订阅系统事件

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Webhook 事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    // 账户事件
    AccountCreated,
    AccountFailed,
    AccountExpired,

    // API Key 事件
    ApiKeyCreated,
    ApiKeyRevoked,

    // 配额事件
    QuotaExhausted,
    QuotaWarning,

    // 计费事件
    PaymentReceived,
    InvoiceGenerated,

    // 系统事件
    ModelAdded,
    ModelDeprecated,
    PriceChanged,
}

impl WebhookEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccountCreated => "account.created",
            Self::AccountFailed => "account.failed",
            Self::AccountExpired => "account.expired",
            Self::ApiKeyCreated => "api_key.created",
            Self::ApiKeyRevoked => "api_key.revoked",
            Self::QuotaExhausted => "quota.exhausted",
            Self::QuotaWarning => "quota.warning",
            Self::PaymentReceived => "payment.received",
            Self::InvoiceGenerated => "invoice.generated",
            Self::ModelAdded => "model.added",
            Self::ModelDeprecated => "model.deprecated",
            Self::PriceChanged => "price.changed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "account.created" => Some(Self::AccountCreated),
            "account.failed" => Some(Self::AccountFailed),
            "account.expired" => Some(Self::AccountExpired),
            "api_key.created" => Some(Self::ApiKeyCreated),
            "api_key.revoked" => Some(Self::ApiKeyRevoked),
            "quota.exhausted" => Some(Self::QuotaExhausted),
            "quota.warning" => Some(Self::QuotaWarning),
            "payment.received" => Some(Self::PaymentReceived),
            "invoice.generated" => Some(Self::InvoiceGenerated),
            "model.added" => Some(Self::ModelAdded),
            "model.deprecated" => Some(Self::ModelDeprecated),
            "price.changed" => Some(Self::PriceChanged),
            _ => None,
        }
    }

    /// 获取所有事件类型
    pub fn all() -> Vec<Self> {
        vec![
            Self::AccountCreated,
            Self::AccountFailed,
            Self::AccountExpired,
            Self::ApiKeyCreated,
            Self::ApiKeyRevoked,
            Self::QuotaExhausted,
            Self::QuotaWarning,
            Self::PaymentReceived,
            Self::InvoiceGenerated,
            Self::ModelAdded,
            Self::ModelDeprecated,
            Self::PriceChanged,
        ]
    }
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_endpoints")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: Uuid,
    /// 回调 URL（必须是 HTTPS）
    pub url: String,
    /// 订阅的事件列表（JSON 数组）
    pub events: JsonValue,
    /// 用于签名验证的密钥
    pub secret: String,
    /// 是否启用
    pub enabled: bool,
    /// 最大重试次数
    pub max_retries: i32,
    /// 请求超时时间（毫秒）
    pub timeout_ms: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
    #[sea_orm(has_many = "super::webhook_deliveries::Entity")]
    Deliveries,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::webhook_deliveries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Deliveries.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 检查端点是否订阅了指定事件
    pub fn is_subscribed_to(&self, event_type: &WebhookEventType) -> bool {
        if let Some(events) = self.events.as_array() {
            let event_str = event_type.as_str();
            return events
                .iter()
                .any(|e| e.as_str().map(|s| s == event_str).unwrap_or(false));
        }
        false
    }

    /// 获取订阅的事件类型列表
    pub fn get_event_types(&self) -> Vec<WebhookEventType> {
        if let Some(events) = self.events.as_array() {
            events
                .iter()
                .filter_map(|e| e.as_str().and_then(WebhookEventType::parse))
                .collect()
        } else {
            vec![]
        }
    }

    /// 验证 URL 是否有效
    pub fn is_valid_url(&self) -> bool {
        // 必须是 HTTPS
        if !self.url.starts_with("https://") {
            return false;
        }

        // 不能是私有 IP 地址（简单检查）
        let url_lower = self.url.to_lowercase();
        let blocked = [
            "https://127.",
            "https://localhost",
            "https://10.",
            "https://192.168.",
            "https://172.16.",
            "https://172.17.",
            "https://172.18.",
            "https://172.19.",
            "https://172.20.",
            "https://172.21.",
            "https://172.22.",
            "https://172.23.",
            "https://172.24.",
            "https://172.25.",
            "https://172.26.",
            "https://172.27.",
            "https://172.28.",
            "https://172.29.",
            "https://172.30.",
            "https://172.31.",
            "https://[::1]",
            "https://[0:",
        ];

        !blocked.iter().any(|b| url_lower.starts_with(b))
    }

    /// 掩码显示密钥（用于日志）
    pub fn mask_secret(&self) -> String {
        if self.secret.len() <= 8 {
            return "*".repeat(self.secret.len());
        }
        format!(
            "{}...{}",
            &self.secret[..4],
            &self.secret[self.secret.len() - 4..]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_parsing() {
        assert_eq!(
            WebhookEventType::parse("account.created"),
            Some(WebhookEventType::AccountCreated)
        );
        assert_eq!(WebhookEventType::parse("invalid.event"), None);
    }

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(WebhookEventType::AccountCreated.as_str(), "account.created");
        assert_eq!(WebhookEventType::QuotaExhausted.as_str(), "quota.exhausted");
    }

    #[test]
    fn test_valid_url() {
        let endpoint = Model {
            id: 1,
            user_id: uuid::Uuid::nil(),
            url: "https://example.com/webhook".to_string(),
            events: serde_json::json!(["account.created"]),
            secret: "test-secret".to_string(),
            enabled: true,
            max_retries: 5,
            timeout_ms: 5000,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(endpoint.is_valid_url());

        let invalid_endpoint = Model {
            url: "http://example.com/webhook".to_string(),
            ..endpoint.clone()
        };
        assert!(!invalid_endpoint.is_valid_url());
    }

    #[test]
    fn test_is_subscribed_to() {
        let endpoint = Model {
            id: 1,
            user_id: uuid::Uuid::nil(),
            url: "https://example.com/webhook".to_string(),
            events: serde_json::json!(["account.created", "account.failed"]),
            secret: "test-secret".to_string(),
            enabled: true,
            max_retries: 5,
            timeout_ms: 5000,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(endpoint.is_subscribed_to(&WebhookEventType::AccountCreated));
        assert!(endpoint.is_subscribed_to(&WebhookEventType::AccountFailed));
        assert!(!endpoint.is_subscribed_to(&WebhookEventType::QuotaExhausted));
    }
}
