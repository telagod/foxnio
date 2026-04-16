//! Payment Orders Entity

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    #[sea_orm(unique)]
    pub order_no: String,
    pub provider: String,
    pub payment_type: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub provider_order_id: Option<String>,
    pub provider_data: Option<JsonValue>,
    pub payment_url: Option<String>,
    pub client_secret: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
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
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn is_pending(&self) -> bool {
        self.status == "pending"
    }

    pub fn is_paid(&self) -> bool {
        self.status == "paid" || self.status == "completed"
    }

    pub fn is_expired(&self) -> bool {
        if self.status == "expired" {
            return true;
        }
        if let Some(expires_at) = self.expires_at {
            return self.status == "pending" && Utc::now() > expires_at;
        }
        false
    }
}
