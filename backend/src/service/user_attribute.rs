//! User Attribute Service

#![allow(dead_code)]

use crate::entity::{user_attribute_definitions, user_attribute_values};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
pub struct CreateAttributeDefinitionRequest {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub options: JsonValue,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub validation: JsonValue,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub display_order: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateAttributeDefinitionRequest {
    pub key: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub options: Option<JsonValue>,
    pub required: Option<bool>,
    pub validation: Option<JsonValue>,
    pub placeholder: Option<String>,
    pub display_order: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetAttributeValueRequest {
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct AttributeDefinitionResponse {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub options: JsonValue,
    pub required: bool,
    pub validation: JsonValue,
    pub placeholder: String,
    pub display_order: i32,
    pub enabled: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AttributeValueResponse {
    pub id: i64,
    pub user_id: i64,
    pub attribute_id: i64,
    pub attribute_key: String,
    pub attribute_name: String,
    pub value: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<user_attribute_definitions::Model> for AttributeDefinitionResponse {
    fn from(model: user_attribute_definitions::Model) -> Self {
        Self {
            id: model.id,
            key: model.key,
            name: model.name,
            description: model.description,
            type_: model.type_,
            options: model.options,
            required: model.required,
            validation: model.validation,
            placeholder: model.placeholder,
            display_order: model.display_order,
            enabled: model.enabled,
            deleted_at: model.deleted_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub struct UserAttributeService;

impl UserAttributeService {
    /// Create attribute definition
    pub async fn create_definition(
        db: &DatabaseConnection,
        req: CreateAttributeDefinitionRequest,
    ) -> Result<AttributeDefinitionResponse> {
        let now = Utc::now();
        let definition = user_attribute_definitions::ActiveModel {
            id: ActiveValue::NotSet,
            key: ActiveValue::Set(req.key),
            name: ActiveValue::Set(req.name),
            description: ActiveValue::Set(req.description),
            type_: ActiveValue::Set(req.type_),
            options: ActiveValue::Set(req.options),
            required: ActiveValue::Set(req.required),
            validation: ActiveValue::Set(req.validation),
            placeholder: ActiveValue::Set(req.placeholder),
            display_order: ActiveValue::Set(req.display_order),
            enabled: ActiveValue::Set(req.enabled),
            deleted_at: ActiveValue::Set(None),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = definition.insert(db).await?;
        Ok(result.into())
    }

    /// List attribute definitions
    pub async fn list_definitions(
        db: &DatabaseConnection,
        enabled_only: bool,
    ) -> Result<Vec<AttributeDefinitionResponse>> {
        let mut query = user_attribute_definitions::Entity::find()
            .filter(user_attribute_definitions::Column::DeletedAt.is_null());

        if enabled_only {
            query = query.filter(user_attribute_definitions::Column::Enabled.eq(true));
        }

        let results = query
            .order_by_asc(user_attribute_definitions::Column::DisplayOrder)
            .all(db)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Update attribute definition
    pub async fn update_definition(
        db: &DatabaseConnection,
        id: i64,
        req: UpdateAttributeDefinitionRequest,
    ) -> Result<Option<AttributeDefinitionResponse>> {
        let definition = user_attribute_definitions::Entity::find_by_id(id)
            .one(db)
            .await?;

        match definition {
            Some(model) => {
                let mut active_model: user_attribute_definitions::ActiveModel = model.into();

                if let Some(key) = req.key {
                    active_model.key = ActiveValue::Set(key);
                }
                if let Some(name) = req.name {
                    active_model.name = ActiveValue::Set(name);
                }
                if let Some(description) = req.description {
                    active_model.description = ActiveValue::Set(description);
                }
                if let Some(type_) = req.type_ {
                    active_model.type_ = ActiveValue::Set(type_);
                }
                if let Some(options) = req.options {
                    active_model.options = ActiveValue::Set(options);
                }
                if let Some(required) = req.required {
                    active_model.required = ActiveValue::Set(required);
                }
                if let Some(validation) = req.validation {
                    active_model.validation = ActiveValue::Set(validation);
                }
                if let Some(placeholder) = req.placeholder {
                    active_model.placeholder = ActiveValue::Set(placeholder);
                }
                if let Some(display_order) = req.display_order {
                    active_model.display_order = ActiveValue::Set(display_order);
                }
                if let Some(enabled) = req.enabled {
                    active_model.enabled = ActiveValue::Set(enabled);
                }
                active_model.updated_at = ActiveValue::Set(Utc::now());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete attribute definition (soft delete)
    pub async fn delete_definition(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let definition = user_attribute_definitions::Entity::find_by_id(id)
            .one(db)
            .await?;

        match definition {
            Some(model) => {
                let mut active_model: user_attribute_definitions::ActiveModel = model.into();
                active_model.deleted_at = ActiveValue::Set(Some(Utc::now()));
                active_model.updated_at = ActiveValue::Set(Utc::now());
                active_model.update(db).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Set attribute value for user
    pub async fn set_value(
        db: &DatabaseConnection,
        user_id: i64,
        attribute_id: i64,
        req: SetAttributeValueRequest,
    ) -> Result<AttributeValueResponse> {
        let now = Utc::now();

        // Check if definition exists and is enabled
        let definition = user_attribute_definitions::Entity::find_by_id(attribute_id)
            .one(db)
            .await?;

        let definition = match definition {
            Some(d) if d.enabled && d.deleted_at.is_none() => d,
            _ => return Err(anyhow::anyhow!("Attribute definition not found")),
        };

        // Check if value already exists
        let existing = user_attribute_values::Entity::find()
            .filter(user_attribute_values::Column::UserId.eq(user_id))
            .filter(user_attribute_values::Column::AttributeId.eq(attribute_id))
            .one(db)
            .await?;

        match existing {
            Some(model) => {
                // Update existing value
                let mut active_model: user_attribute_values::ActiveModel = model.into();
                active_model.value = ActiveValue::Set(req.value);
                active_model.updated_at = ActiveValue::Set(now);
                let result = active_model.update(db).await?;

                Ok(AttributeValueResponse {
                    id: result.id,
                    user_id: result.user_id,
                    attribute_id: result.attribute_id,
                    attribute_key: definition.key,
                    attribute_name: definition.name,
                    value: result.value,
                    created_at: result.created_at,
                    updated_at: result.updated_at,
                })
            }
            None => {
                // Create new value
                let value = user_attribute_values::ActiveModel {
                    id: ActiveValue::NotSet,
                    user_id: ActiveValue::Set(user_id),
                    attribute_id: ActiveValue::Set(attribute_id),
                    value: ActiveValue::Set(req.value),
                    created_at: ActiveValue::Set(now),
                    updated_at: ActiveValue::Set(now),
                };

                let result = value.insert(db).await?;

                Ok(AttributeValueResponse {
                    id: result.id,
                    user_id: result.user_id,
                    attribute_id: result.attribute_id,
                    attribute_key: definition.key,
                    attribute_name: definition.name,
                    value: result.value,
                    created_at: result.created_at,
                    updated_at: result.updated_at,
                })
            }
        }
    }

    /// Get attribute values for user
    pub async fn get_user_values(
        db: &DatabaseConnection,
        user_id: i64,
    ) -> Result<Vec<AttributeValueResponse>> {
        let values = user_attribute_values::Entity::find()
            .filter(user_attribute_values::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        let mut responses = Vec::new();
        for value in values {
            if let Some(definition) =
                user_attribute_definitions::Entity::find_by_id(value.attribute_id)
                    .one(db)
                    .await?
            {
                responses.push(AttributeValueResponse {
                    id: value.id,
                    user_id: value.user_id,
                    attribute_id: value.attribute_id,
                    attribute_key: definition.key,
                    attribute_name: definition.name,
                    value: value.value,
                    created_at: value.created_at,
                    updated_at: value.updated_at,
                });
            }
        }

        Ok(responses)
    }
}
