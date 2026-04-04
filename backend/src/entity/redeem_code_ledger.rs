//! Redeem Code Ledger Entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "redeem_code_ledger")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub redeem_code_id: i64,
    pub user_id: Uuid,
    #[sea_orm(column_type = "String(StringLen::N(255))", nullable)]
    pub idempotency_key: Option<String>,
    #[sea_orm(column_type = "String(StringLen::N(64))")]
    pub request_fingerprint: String,
    #[sea_orm(column_type = "String(StringLen::N(20))")]
    pub code_type: String,
    pub amount: Decimal,
    pub balance_delta_cents: Option<i64>,
    pub subscription_days: Option<i64>,
    pub quota_delta: Option<Decimal>,
    pub subscription_id: Option<i64>,
    pub result_message: String,
    pub metadata: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
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
