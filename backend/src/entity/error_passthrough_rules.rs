//! Error Passthrough Rule Entity

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "error_passthrough_rules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub error_codes: Option<JsonValue>,
    pub keywords: Option<JsonValue>,
    pub match_mode: String,
    pub platforms: Option<JsonValue>,
    pub passthrough_code: bool,
    pub response_code: Option<i32>,
    pub passthrough_body: bool,
    pub custom_message: Option<String>,
    pub skip_monitoring: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 检查错误是否匹配该规则
    pub fn matches(
        &self,
        error_code: Option<i32>,
        error_message: Option<&str>,
        platform: Option<&str>,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查平台匹配
        if let Some(platforms) = &self.platforms {
            if let Some(platforms_array) = platforms.as_array() {
                if !platforms_array.is_empty() {
                    if let Some(p) = platform {
                        if !platforms_array.iter().any(|pl| pl.as_str() == Some(p)) {
                            return false;
                        }
                    }
                }
            }
        }

        let code_matches = if let Some(codes) = &self.error_codes {
            if let Some(codes_array) = codes.as_array() {
                if codes_array.is_empty() {
                    true
                } else if let Some(code) = error_code {
                    codes_array.iter().any(|c| c.as_i64() == Some(code as i64))
                } else {
                    false
                }
            } else {
                true
            }
        } else {
            true
        };

        let keyword_matches = if let Some(keywords) = &self.keywords {
            if let Some(keywords_array) = keywords.as_array() {
                if keywords_array.is_empty() {
                    true
                } else if let Some(msg) = error_message {
                    let msg_lower = msg.to_lowercase();
                    keywords_array.iter().any(|k| {
                        k.as_str()
                            .map(|keyword| msg_lower.contains(&keyword.to_lowercase()))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            } else {
                true
            }
        } else {
            true
        };

        match self.match_mode.as_str() {
            "all" => code_matches && keyword_matches,
            _ => code_matches || keyword_matches, // "any"
        }
    }
}
