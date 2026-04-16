//! Account Entity

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 账号类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AccountType {
    /// API Key 认证
    #[default]
    ApiKey,
    /// OAuth 认证
    OAuth,
    /// Setup Token（一次性设置）
    SetupToken,
    /// 上游代理
    Upstream,
    /// AWS Bedrock
    Bedrock,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::OAuth => "oauth",
            Self::SetupToken => "setup_token",
            Self::Upstream => "upstream",
            Self::Bedrock => "bedrock",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "api_key" => Self::ApiKey,
            "oauth" => Self::OAuth,
            "setup_token" => Self::SetupToken,
            "upstream" => Self::Upstream,
            "bedrock" => Self::Bedrock,
            _ => Self::ApiKey, // 默认
        }
    }

    /// 检查是否是 OAuth 类型
    pub fn is_oauth(&self) -> bool {
        matches!(self, Self::OAuth | Self::SetupToken)
    }

    /// 检查是否需要刷新 token
    pub fn needs_token_refresh(&self) -> bool {
        matches!(self, Self::OAuth)
    }
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub credential_type: String,
    pub credential: String,
    pub metadata: Option<JsonValue>,
    pub status: String,
    pub last_error: Option<String>,
    pub priority: i32,
    pub concurrent_limit: Option<i32>,
    pub rate_limit_rpm: Option<i32>,
    /// 默认分组 ID（可选）
    pub group_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::usages::Entity")]
    Usages,
    #[sea_orm(
        belongs_to = "super::groups::Entity",
        from = "Column::GroupId",
        to = "super::groups::Column::Id"
    )]
    Group,
}

impl Related<super::usages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Usages.def()
    }
}

impl Related<super::groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Group.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn provider_type(&self) -> ProviderType {
        match self.provider.as_str() {
            "anthropic" | "claude" => ProviderType::Anthropic,
            "openai" => ProviderType::OpenAI,
            "gemini" => ProviderType::Gemini,
            "droid" => ProviderType::Droid,
            "antigravity" => ProviderType::Antigravity,
            _ => ProviderType::Unknown,
        }
    }

    /// 获取账号类型
    pub fn account_type(&self) -> AccountType {
        AccountType::parse(&self.credential_type)
    }

    /// 检查是否是 OAuth 账号
    pub fn is_oauth_account(&self) -> bool {
        self.account_type().is_oauth()
    }

    /// 从 metadata 获取 OAuth refresh_token（如果存在）
    pub fn get_refresh_token(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("refresh_token"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// 从 metadata 获取 OAuth token 过期时间
    pub fn get_token_expires_at(&self) -> Option<DateTime<Utc>> {
        self.metadata
            .as_ref()
            .and_then(|m| m.get("token_expires_at"))
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// 检查 OAuth token 是否需要刷新
    pub fn needs_token_refresh(&self) -> bool {
        if !self.is_oauth_account() {
            return false;
        }

        match self.get_token_expires_at() {
            Some(expires_at) => {
                // 提前 5 分钟刷新
                Utc::now() >= expires_at - chrono::Duration::minutes(5)
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    Droid,
    Antigravity,
    Unknown,
}

impl ProviderType {
    pub fn base_url(&self) -> &'static str {
        match self {
            ProviderType::Anthropic => "https://api.anthropic.com",
            ProviderType::OpenAI => "https://api.openai.com",
            ProviderType::Gemini => "https://generativelanguage.googleapis.com",
            ProviderType::Droid => "http://127.0.0.1:3000",
            ProviderType::Antigravity => "https://antigravity.so",
            ProviderType::Unknown => "",
        }
    }

    /// 检查是否支持 OAuth
    pub fn supports_oauth(&self) -> bool {
        matches!(
            self,
            ProviderType::Anthropic
                | ProviderType::OpenAI
                | ProviderType::Gemini
                | ProviderType::Droid
                | ProviderType::Antigravity
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_type() {
        assert_eq!(AccountType::ApiKey.as_str(), "api_key");
        assert_eq!(AccountType::parse("oauth"), AccountType::OAuth);
        assert!(AccountType::OAuth.is_oauth());
        assert!(!AccountType::ApiKey.is_oauth());
    }

    #[test]
    fn test_account_type_default() {
        let default_type = AccountType::default();
        assert_eq!(default_type, AccountType::ApiKey);
    }

    #[test]
    fn test_provider_type_oauth_support() {
        assert!(ProviderType::Anthropic.supports_oauth());
        assert!(ProviderType::OpenAI.supports_oauth());
        assert!(ProviderType::Gemini.supports_oauth());
        assert!(!ProviderType::Unknown.supports_oauth());
    }
}
