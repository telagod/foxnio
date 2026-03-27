//! API Key Entity

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    #[sea_orm(unique)]
    pub key: String,
    pub name: Option<String>,
    pub prefix: String,
    pub status: String,
    pub concurrent_limit: Option<i32>,
    pub rate_limit_rpm: Option<i32>,
    pub allowed_models: Option<JsonValue>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
    #[sea_orm(has_many = "super::usages::Entity")]
    Usages,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::usages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Usages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn is_active(&self) -> bool {
        if self.status != "active" {
            return false;
        }
        if let Some(expires) = self.expires_at {
            return expires > Utc::now();
        }
        true
    }

    pub fn mask_key(&self) -> String {
        if self.key.len() < 12 {
            return self.key.clone();
        }
        format!("{}...{}", &self.key[..7], &self.key[self.key.len()-4..])
    }
}
