//! Promo Code Service

#![allow(dead_code)]

use crate::entity::{promo_code_usages, promo_codes};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreatePromoCodeRequest {
    pub code: String,
    pub bonus_amount: f64,
    #[serde(default)]
    pub max_uses: i32,
    #[serde(default)]
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromoCodeRequest {
    pub code: Option<String>,
    pub bonus_amount: Option<f64>,
    pub max_uses: Option<i32>,
    pub status: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyPromoCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct PromoCodeResponse {
    pub id: i64,
    pub code: String,
    pub bonus_amount: f64,
    pub max_uses: i32,
    pub used_count: i32,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct VerifyPromoCodeResponse {
    pub valid: bool,
    pub bonus_amount: Option<f64>,
    pub message: Option<String>,
}

impl From<promo_codes::Model> for PromoCodeResponse {
    fn from(model: promo_codes::Model) -> Self {
        Self {
            id: model.id,
            code: model.code,
            bonus_amount: model.bonus_amount,
            max_uses: model.max_uses,
            used_count: model.used_count,
            status: model.status,
            expires_at: model.expires_at,
            notes: model.notes,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub struct PromoCodeService;

impl PromoCodeService {
    /// Create a new promo code
    pub async fn create(
        db: &DatabaseConnection,
        req: CreatePromoCodeRequest,
    ) -> Result<PromoCodeResponse> {
        let now = Utc::now();
        let promo_code = promo_codes::ActiveModel {
            id: ActiveValue::NotSet,
            code: ActiveValue::Set(req.code),
            bonus_amount: ActiveValue::Set(req.bonus_amount),
            max_uses: ActiveValue::Set(req.max_uses),
            used_count: ActiveValue::Set(0),
            status: ActiveValue::Set(req.status),
            expires_at: ActiveValue::Set(req.expires_at),
            notes: ActiveValue::Set(req.notes),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = promo_code.insert(db).await?;
        Ok(result.into())
    }

    /// Get promo code by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: i64) -> Result<Option<PromoCodeResponse>> {
        let result = promo_codes::Entity::find_by_id(id).one(db).await?;

        Ok(result.map(|m| m.into()))
    }

    /// Get promo code by code
    pub async fn get_by_code(
        db: &DatabaseConnection,
        code: &str,
    ) -> Result<Option<PromoCodeResponse>> {
        let result = promo_codes::Entity::find()
            .filter(promo_codes::Column::Code.eq(code))
            .one(db)
            .await?;

        Ok(result.map(|m| m.into()))
    }

    /// List all promo codes
    pub async fn list(
        db: &DatabaseConnection,
        status: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<PromoCodeResponse>> {
        let mut query = promo_codes::Entity::find();

        if let Some(s) = status {
            query = query.filter(promo_codes::Column::Status.eq(s));
        }

        let results = query
            .order_by_desc(promo_codes::Column::CreatedAt)
            .paginate(db, page_size)
            .fetch_page(page)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Update promo code
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        req: UpdatePromoCodeRequest,
    ) -> Result<Option<PromoCodeResponse>> {
        let promo_code = promo_codes::Entity::find_by_id(id).one(db).await?;

        match promo_code {
            Some(model) => {
                let mut active_model: promo_codes::ActiveModel = model.into();

                if let Some(code) = req.code {
                    active_model.code = ActiveValue::Set(code);
                }
                if let Some(bonus_amount) = req.bonus_amount {
                    active_model.bonus_amount = ActiveValue::Set(bonus_amount);
                }
                if let Some(max_uses) = req.max_uses {
                    active_model.max_uses = ActiveValue::Set(max_uses);
                }
                if let Some(status) = req.status {
                    active_model.status = ActiveValue::Set(status);
                }
                if let Some(expires_at) = req.expires_at {
                    active_model.expires_at = ActiveValue::Set(Some(expires_at));
                }
                if let Some(notes) = req.notes {
                    active_model.notes = ActiveValue::Set(Some(notes));
                }
                active_model.updated_at = ActiveValue::Set(Utc::now());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete promo code
    pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let result = promo_codes::Entity::delete_by_id(id).exec(db).await?;

        Ok(result.rows_affected > 0)
    }

    /// Verify promo code
    pub async fn verify(
        db: &DatabaseConnection,
        req: VerifyPromoCodeRequest,
    ) -> Result<VerifyPromoCodeResponse> {
        let promo_code = promo_codes::Entity::find()
            .filter(promo_codes::Column::Code.eq(&req.code))
            .one(db)
            .await?;

        match promo_code {
            Some(model) => {
                if !model.is_valid() {
                    return Ok(VerifyPromoCodeResponse {
                        valid: false,
                        bonus_amount: None,
                        message: Some("Promo code is not valid".to_string()),
                    });
                }

                Ok(VerifyPromoCodeResponse {
                    valid: true,
                    bonus_amount: Some(model.bonus_amount),
                    message: None,
                })
            }
            None => Ok(VerifyPromoCodeResponse {
                valid: false,
                bonus_amount: None,
                message: Some("Promo code not found".to_string()),
            }),
        }
    }

    /// Use promo code
    pub async fn use_code(db: &DatabaseConnection, code: &str, user_id: i64) -> Result<f64> {
        let promo_code = promo_codes::Entity::find()
            .filter(promo_codes::Column::Code.eq(code))
            .one(db)
            .await?;

        match promo_code {
            Some(model) => {
                if !model.is_valid() {
                    return Err(anyhow::anyhow!("Promo code is not valid"));
                }

                // Check if user already used this code
                let existing_usage = promo_code_usages::Entity::find()
                    .filter(promo_code_usages::Column::PromoCodeId.eq(model.id))
                    .filter(promo_code_usages::Column::UserId.eq(user_id))
                    .one(db)
                    .await?;

                if existing_usage.is_some() {
                    return Err(anyhow::anyhow!("Promo code already used"));
                }

                // Increment usage count
                let mut active_model: promo_codes::ActiveModel = model.clone().into();
                active_model.used_count = ActiveValue::Set(model.used_count + 1);
                active_model.updated_at = ActiveValue::Set(Utc::now());
                active_model.update(db).await?;

                // Record usage
                let usage = promo_code_usages::ActiveModel {
                    id: ActiveValue::NotSet,
                    promo_code_id: ActiveValue::Set(model.id),
                    user_id: ActiveValue::Set(user_id),
                    created_at: ActiveValue::Set(Utc::now()),
                };
                usage.insert(db).await?;

                Ok(model.bonus_amount)
            }
            None => Err(anyhow::anyhow!("Promo code not found")),
        }
    }
}
