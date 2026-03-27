//! Account Entity

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::usages::Entity")]
    Usages,
}

impl Related<super::usages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Usages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn provider_type(&self) -> ProviderType {
        match self.provider.as_str() {
            "anthropic" => ProviderType::Anthropic,
            "openai" => ProviderType::OpenAI,
            "gemini" => ProviderType::Gemini,
            "antigravity" => ProviderType::Antigravity,
            _ => ProviderType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    Antigravity,
    Unknown,
}

impl ProviderType {
    pub fn base_url(&self) -> &'static str {
        match self {
            ProviderType::Anthropic => "https://api.anthropic.com",
            ProviderType::OpenAI => "https://api.openai.com",
            ProviderType::Gemini => "https://generativelanguage.googleapis.com",
            ProviderType::Antigravity => "https://antigravity.so",
            ProviderType::Unknown => "",
        }
    }
}
