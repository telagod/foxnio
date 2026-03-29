//! Proxy entity

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "proxies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub tags: Option<Json>,
    pub health_check_url: Option<String>,
    pub last_check_at: Option<DateTimeWithTimeZone>,
    pub last_check_status: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get proxy URL
    pub fn url(&self) -> String {
        match (&self.username, &self.password) {
            (Some(user), Some(pass)) => {
                format!(
                    "{}://{}:{}@{}:{}",
                    self.protocol, user, pass, self.host, self.port
                )
            }
            (Some(user), None) => {
                format!("{}://{}@{}:{}", self.protocol, user, self.host, self.port)
            }
            _ => format!("{}://{}:{}", self.protocol, self.host, self.port),
        }
    }

    /// Check if proxy is healthy
    pub fn is_healthy(&self) -> bool {
        self.enabled && self.last_check_status.as_deref() == Some("healthy")
    }
}
