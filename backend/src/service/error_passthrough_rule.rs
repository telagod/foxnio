//! Error Passthrough Rule Service

#![allow(dead_code)]

use crate::entity::error_passthrough_rules;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
pub struct CreateErrorRuleRequest {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    pub error_codes: Option<JsonValue>,
    pub keywords: Option<JsonValue>,
    #[serde(default = "default_match_mode")]
    pub match_mode: String,
    pub platforms: Option<JsonValue>,
    #[serde(default = "default_true")]
    pub passthrough_code: bool,
    pub response_code: Option<i32>,
    #[serde(default = "default_true")]
    pub passthrough_body: bool,
    pub custom_message: Option<String>,
    #[serde(default)]
    pub skip_monitoring: bool,
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_match_mode() -> String {
    "any".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdateErrorRuleRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub error_codes: Option<JsonValue>,
    pub keywords: Option<JsonValue>,
    pub match_mode: Option<String>,
    pub platforms: Option<JsonValue>,
    pub passthrough_code: Option<bool>,
    pub response_code: Option<i32>,
    pub passthrough_body: Option<bool>,
    pub custom_message: Option<String>,
    pub skip_monitoring: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorRuleResponse {
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

#[derive(Debug, Serialize)]
pub struct ApplyRuleResult {
    pub matched: bool,
    pub response_code: Option<i32>,
    pub response_body: Option<String>,
    pub skip_monitoring: bool,
}

impl From<error_passthrough_rules::Model> for ErrorRuleResponse {
    fn from(model: error_passthrough_rules::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            enabled: model.enabled,
            priority: model.priority,
            error_codes: model.error_codes,
            keywords: model.keywords,
            match_mode: model.match_mode,
            platforms: model.platforms,
            passthrough_code: model.passthrough_code,
            response_code: model.response_code,
            passthrough_body: model.passthrough_body,
            custom_message: model.custom_message,
            skip_monitoring: model.skip_monitoring,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub struct ErrorPassthroughRuleService;

impl ErrorPassthroughRuleService {
    /// Create error rule
    pub async fn create(
        db: &DatabaseConnection,
        req: CreateErrorRuleRequest,
    ) -> Result<ErrorRuleResponse> {
        let now = Utc::now();
        let rule = error_passthrough_rules::ActiveModel {
            id: ActiveValue::NotSet,
            name: ActiveValue::Set(req.name),
            enabled: ActiveValue::Set(req.enabled),
            priority: ActiveValue::Set(req.priority),
            error_codes: ActiveValue::Set(req.error_codes),
            keywords: ActiveValue::Set(req.keywords),
            match_mode: ActiveValue::Set(req.match_mode),
            platforms: ActiveValue::Set(req.platforms),
            passthrough_code: ActiveValue::Set(req.passthrough_code),
            response_code: ActiveValue::Set(req.response_code),
            passthrough_body: ActiveValue::Set(req.passthrough_body),
            custom_message: ActiveValue::Set(req.custom_message),
            skip_monitoring: ActiveValue::Set(req.skip_monitoring),
            description: ActiveValue::Set(req.description),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = rule.insert(db).await?;
        Ok(result.into())
    }

    /// List all error rules
    pub async fn list(
        db: &DatabaseConnection,
        enabled_only: bool,
    ) -> Result<Vec<ErrorRuleResponse>> {
        let mut query = error_passthrough_rules::Entity::find();

        if enabled_only {
            query = query.filter(error_passthrough_rules::Column::Enabled.eq(true));
        }

        let results = query
            .order_by_asc(error_passthrough_rules::Column::Priority)
            .all(db)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Update error rule
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        req: UpdateErrorRuleRequest,
    ) -> Result<Option<ErrorRuleResponse>> {
        let rule = error_passthrough_rules::Entity::find_by_id(id)
            .one(db)
            .await?;

        match rule {
            Some(model) => {
                let mut active_model: error_passthrough_rules::ActiveModel = model.into();

                if let Some(name) = req.name {
                    active_model.name = ActiveValue::Set(name);
                }
                if let Some(enabled) = req.enabled {
                    active_model.enabled = ActiveValue::Set(enabled);
                }
                if let Some(priority) = req.priority {
                    active_model.priority = ActiveValue::Set(priority);
                }
                if let Some(error_codes) = req.error_codes {
                    active_model.error_codes = ActiveValue::Set(Some(error_codes));
                }
                if let Some(keywords) = req.keywords {
                    active_model.keywords = ActiveValue::Set(Some(keywords));
                }
                if let Some(match_mode) = req.match_mode {
                    active_model.match_mode = ActiveValue::Set(match_mode);
                }
                if let Some(platforms) = req.platforms {
                    active_model.platforms = ActiveValue::Set(Some(platforms));
                }
                if let Some(passthrough_code) = req.passthrough_code {
                    active_model.passthrough_code = ActiveValue::Set(passthrough_code);
                }
                if let Some(response_code) = req.response_code {
                    active_model.response_code = ActiveValue::Set(Some(response_code));
                }
                if let Some(passthrough_body) = req.passthrough_body {
                    active_model.passthrough_body = ActiveValue::Set(passthrough_body);
                }
                if let Some(custom_message) = req.custom_message {
                    active_model.custom_message = ActiveValue::Set(Some(custom_message));
                }
                if let Some(skip_monitoring) = req.skip_monitoring {
                    active_model.skip_monitoring = ActiveValue::Set(skip_monitoring);
                }
                if let Some(description) = req.description {
                    active_model.description = ActiveValue::Set(Some(description));
                }
                active_model.updated_at = ActiveValue::Set(Utc::now());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete error rule
    pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let result = error_passthrough_rules::Entity::delete_by_id(id)
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Apply error rules to determine response
    pub async fn apply_rules(
        db: &DatabaseConnection,
        error_code: Option<i32>,
        error_message: Option<&str>,
        platform: Option<&str>,
        original_response_code: Option<i32>,
        original_response_body: Option<&str>,
    ) -> Result<ApplyRuleResult> {
        // Get all enabled rules ordered by priority
        let rules = error_passthrough_rules::Entity::find()
            .filter(error_passthrough_rules::Column::Enabled.eq(true))
            .order_by_asc(error_passthrough_rules::Column::Priority)
            .all(db)
            .await?;

        // Find first matching rule
        for rule in rules {
            if rule.matches(error_code, error_message, platform) {
                let response_code = if rule.passthrough_code {
                    original_response_code
                } else {
                    rule.response_code
                };

                let response_body = if rule.passthrough_body {
                    original_response_body.map(|s| s.to_string())
                } else {
                    rule.custom_message
                };

                return Ok(ApplyRuleResult {
                    matched: true,
                    response_code,
                    response_body,
                    skip_monitoring: rule.skip_monitoring,
                });
            }
        }

        // No rule matched, return original
        Ok(ApplyRuleResult {
            matched: false,
            response_code: original_response_code,
            response_body: original_response_body.map(|s| s.to_string()),
            skip_monitoring: false,
        })
    }
}
