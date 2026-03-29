//! 上游账号模型

#![allow(dead_code)]
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub credentials: Option<String>,
    #[sqlx(json)]
    pub models: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountProvider {
    Anthropic,
    OpenAI,
    Gemini,
    Antigravity,
}

impl TryFrom<String> for AccountProvider {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(AccountProvider::Anthropic),
            "openai" => Ok(AccountProvider::OpenAI),
            "gemini" => Ok(AccountProvider::Gemini),
            "antigravity" => Ok(AccountProvider::Antigravity),
            _ => Err(format!("Invalid AccountProvider: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    ApiKey,
    OAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Inactive,
    Error,
}

impl TryFrom<String> for AccountStatus {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "active" => Ok(AccountStatus::Active),
            "inactive" => Ok(AccountStatus::Inactive),
            "error" => Ok(AccountStatus::Error),
            _ => Err(format!("Invalid AccountStatus: {}", s)),
        }
    }
}

impl Account {
    pub fn provider_enum(&self) -> Result<AccountProvider, String> {
        AccountProvider::try_from(self.provider.clone())
    }

    pub fn status_enum(&self) -> Result<AccountStatus, String> {
        AccountStatus::try_from(self.status.clone())
    }
}
