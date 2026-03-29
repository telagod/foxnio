use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

/// System setting configuration
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    pub id: i64,
    pub key: String,
    pub value: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SettingCategory {
    System,
    Gateway,
    Billing,
    Security,
    RateLimit,
    Custom(String),
}

impl SettingCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            SettingCategory::System => "system",
            SettingCategory::Gateway => "gateway",
            SettingCategory::Billing => "billing",
            SettingCategory::Security => "security",
            SettingCategory::RateLimit => "rate_limit",
            SettingCategory::Custom(_) => "custom",
        }
    }
}

impl Setting {
    pub fn new(key: String, value: String, _category: SettingCategory) -> Self {
        Self {
            id: 0,
            key,
            value: Some(value),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    pub fn with_description(self, _description: String) -> Self {
        // Description field removed, method kept for compatibility
        self
    }

    pub fn public(self) -> Self {
        // is_public field removed, method kept for compatibility
        self
    }

    pub fn parse_value<T>(&self) -> Result<T, SettingError>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self
            .value
            .as_ref()
            .ok_or(SettingError::NotFound(self.key.clone()))?;
        serde_json::from_str(value).map_err(|e| SettingError::ParseError {
            key: self.key.clone(),
            source: e,
        })
    }

    pub fn to_json(&self) -> Result<String, SettingError> {
        let value = self
            .value
            .as_ref()
            .ok_or(SettingError::NotFound(self.key.clone()))?;
        serde_json::to_string(&value).map_err(|e| SettingError::SerializeError {
            key: self.key.clone(),
            source: e,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingError {
    #[error("Failed to parse setting {key}: {source}")]
    ParseError {
        key: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("Failed to serialize setting {key}: {source}")]
    SerializeError {
        key: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("Setting not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type SettingMap = HashMap<String, Setting>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_creation() {
        let setting = Setting::new(
            "max_rate_limit".to_string(),
            "1000".to_string(),
            SettingCategory::RateLimit,
        );

        assert_eq!(setting.key, "max_rate_limit");
        assert_eq!(setting.value, Some("1000".to_string()));
    }

    #[test]
    fn test_setting_with_description() {
        let setting = Setting::new(
            "api_timeout".to_string(),
            "30".to_string(),
            SettingCategory::System,
        )
        .with_description("API request timeout in seconds".to_string());

        assert!(setting.value.is_some());
    }

    #[test]
    fn test_setting_public() {
        let setting = Setting::new(
            "public_key".to_string(),
            "value".to_string(),
            SettingCategory::Custom("app".to_string()),
        )
        .public();

        assert!(setting.value.is_some());
    }
}
