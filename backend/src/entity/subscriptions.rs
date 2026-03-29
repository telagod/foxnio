//! Subscription entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "subscriptions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub plan_id: String,
    pub plan_name: String,
    #[sea_orm(default_value = "active")]
    pub status: String,
    #[sea_orm(default_value = "0")]
    pub quota_limit: Decimal,
    #[sea_orm(default_value = "0")]
    pub quota_used: Decimal,
    pub rate_limit_5h: Option<Decimal>,
    pub rate_limit_1d: Option<Decimal>,
    pub rate_limit_7d: Option<Decimal>,
    pub features: Option<Json>,
    pub stripe_subscription_id: Option<String>,
    pub stripe_customer_id: Option<String>,
    pub current_period_start: Option<DateTimeWithTimeZone>,
    pub current_period_end: Option<DateTimeWithTimeZone>,
    #[sea_orm(default_value = "false")]
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if subscription is active
    pub fn is_active(&self) -> bool {
        self.status == "active" && !self.cancel_at_period_end
    }

    /// Get remaining quota
    pub fn remaining_quota(&self) -> f64 {
        (self.quota_limit - self.quota_used)
            .to_string()
            .parse()
            .unwrap_or(0.0)
    }

    /// Check if subscription has quota
    pub fn has_quota(&self, amount: f64) -> bool {
        self.remaining_quota() >= amount
    }
}
