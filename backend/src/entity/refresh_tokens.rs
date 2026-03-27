//! Refresh Token Entity
//!
//! 存储 JWT refresh token 的哈希值，支持安全的 token 轮换和撤销

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "refresh_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    #[sea_orm(unique)]
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_reason: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
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
    /// 检查 token 是否已过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// 检查 token 是否有效（未过期且未撤销）
    pub fn is_valid(&self) -> bool {
        !self.revoked && !self.is_expired()
    }

    /// 检查 token 是否被撤销
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }
}

/// 创建 refresh token 的请求信息
#[derive(Debug, Clone)]
pub struct CreateRefreshToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}

/// Refresh token 的验证结果
#[derive(Debug)]
pub enum RefreshTokenValidation {
    /// 有效
    Valid { user_id: Uuid },
    /// 已过期
    Expired,
    /// 已撤销
    Revoked { reason: Option<String> },
    /// 不存在
    NotFound,
}
