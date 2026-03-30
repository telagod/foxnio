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
    pub ip_whitelist: Option<JsonValue>,
    pub expires_at: Option<DateTime<Utc>>,
    pub daily_quota: Option<i64>,
    pub daily_used_quota: Option<i64>,
    pub quota_reset_at: Option<DateTime<Utc>>,
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
        format!("{}...{}", &self.key[..7], &self.key[self.key.len() - 4..])
    }

    /// 检查模型是否在允许列表中
    pub fn is_model_allowed(&self, model: &str) -> bool {
        if let Some(allowed_models) = &self.allowed_models {
            if let Some(models) = allowed_models.as_array() {
                // 如果列表为空，表示允许所有模型
                if models.is_empty() {
                    return true;
                }
                return models.iter().any(|m| {
                    m.as_str()
                        .map(|s| s == model || model.starts_with(s))
                        .unwrap_or(false)
                });
            }
        }
        // 如果没有设置 allowed_models，允许所有模型
        true
    }

    /// 检查 IP 是否在白名单中
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        if let Some(ip_whitelist) = &self.ip_whitelist {
            if let Some(ips) = ip_whitelist.as_array() {
                // 如果列表为空，表示允许所有 IP
                if ips.is_empty() {
                    return true;
                }
                return ips
                    .iter()
                    .any(|allowed_ip| allowed_ip.as_str().map(|s| s == ip).unwrap_or(false));
            }
        }
        // 如果没有设置 ip_whitelist，允许所有 IP
        true
    }

    /// 检查是否超过每日配额
    pub fn is_quota_exceeded(&self) -> bool {
        if let Some(daily_quota) = self.daily_quota {
            if daily_quota <= 0 {
                return false; // 配额为 0 或负数表示无限制
            }
            let used = self.daily_used_quota.unwrap_or(0);
            return used >= daily_quota;
        }
        // 如果没有设置 daily_quota，表示无限制
        false
    }

    /// 检查是否需要重置配额
    pub fn needs_quota_reset(&self) -> bool {
        if self.daily_quota.is_none() {
            return false;
        }

        if let Some(reset_at) = self.quota_reset_at {
            // 如果重置时间已过，需要重置
            return reset_at <= Utc::now();
        }

        // 如果设置了配额但没有重置时间，需要初始化
        true
    }

    /// 获取剩余配额
    pub fn remaining_quota(&self) -> Option<i64> {
        self.daily_quota.map(|quota| {
            let used = self.daily_used_quota.unwrap_or(0);
            (quota - used).max(0)
        })
    }
}
