//! OAuth Token Entity
//!
//! 存储 OAuth 令牌，支持加密存储 access_token 和 refresh_token

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// OAuth 提供商类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    Custom,
}

impl OAuthProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAI,
            "gemini" => Self::Gemini,
            _ => Self::Custom,
        }
    }
}

impl std::fmt::Display for OAuthProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub account_id: Uuid,
    pub provider: String,
    /// 加密存储的 access_token
    pub access_token: String,
    /// 加密存储的 refresh_token
    pub refresh_token: Option<String>,
    /// Token 过期时间
    pub expires_at: Option<DateTime<Utc>>,
    /// Token 类型（通常为 "Bearer"）
    pub token_type: Option<String>,
    /// OAuth scope
    pub scope: Option<String>,
    /// 额外元数据（如 client_id, redirect_uri 等）
    pub metadata: Option<JsonValue>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Account,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 检查 token 是否已过期
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() >= expires_at,
            None => false, // 没有过期时间视为永不过期
        }
    }

    /// 检查是否需要刷新（提前 5 分钟刷新）
    pub fn needs_refresh(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                Utc::now() >= expires_at - chrono::Duration::minutes(5)
            }
            None => false,
        }
    }

    /// 获取 OAuth 提供商类型
    pub fn provider_type(&self) -> OAuthProviderType {
        OAuthProviderType::from_str(&self.provider)
    }

    /// 是否有 refresh_token
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// 创建 OAuth token 的请求
#[derive(Debug, Clone)]
pub struct CreateOAuthToken {
    pub account_id: Uuid,
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>, // 秒数
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub metadata: Option<JsonValue>,
}

impl CreateOAuthToken {
    pub fn new(
        account_id: Uuid,
        provider: String,
        access_token: String,
    ) -> Self {
        Self {
            account_id,
            provider,
            access_token,
            refresh_token: None,
            expires_in: None,
            token_type: None,
            scope: None,
            metadata: None,
        }
    }

    pub fn with_refresh_token(mut self, refresh_token: String) -> Self {
        self.refresh_token = Some(refresh_token);
        self
    }

    pub fn with_expires_in(mut self, expires_in: i64) -> Self {
        self.expires_in = Some(expires_in);
        self
    }

    pub fn with_token_type(mut self, token_type: String) -> Self {
        self.token_type = Some(token_type);
        self
    }

    pub fn with_scope(mut self, scope: String) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 计算过期时间
    pub fn calculate_expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_in.map(|seconds| {
            Utc::now() + chrono::Duration::seconds(seconds)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_provider_type() {
        assert_eq!(OAuthProviderType::Anthropic.as_str(), "anthropic");
        assert_eq!(
            OAuthProviderType::from_str("openai"),
            OAuthProviderType::OpenAI
        );
    }

    #[test]
    fn test_create_oauth_token() {
        let create = CreateOAuthToken::new(
            Uuid::new_v4(),
            "anthropic".to_string(),
            "test_access_token".to_string(),
        )
        .with_refresh_token("test_refresh_token".to_string())
        .with_expires_in(3600);

        assert!(create.refresh_token.is_some());
        assert!(create.calculate_expires_at().is_some());
    }
}
