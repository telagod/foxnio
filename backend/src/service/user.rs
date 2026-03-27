//! 用户服务 - 完整实现

use anyhow::{Result, bail};
use argon2::{password_hash::{rand_core::OsRng, PasswordHasher, PasswordHash, PasswordVerifier, SaltString}, Argon2};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use sea_orm::{
    EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, 
    DatabaseConnection, ActiveValue, QuerySelect, PaginatorTrait,
};
use uuid::Uuid;
use chrono::{Duration, Utc};

use crate::entity::users;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub created_at: chrono::DateTime<Utc>,
}

pub struct UserService {
    db: DatabaseConnection,
    jwt_secret: String,
    jwt_expire_hours: u64,
}

impl UserService {
    pub fn new(db: DatabaseConnection, jwt_secret: String, jwt_expire_hours: u64) -> Self {
        Self { db, jwt_secret, jwt_expire_hours }
    }

    /// 注册新用户
    pub async fn register(&self, email: &str, password: &str) -> Result<UserInfo> {
        // 检查邮箱是否已存在
        let existing = users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            bail!("Email already registered");
        }

        // 哈希密码
        let password_hash = self.hash_password(password)?;
        let now = Utc::now();

        // 创建用户
        let user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            email: Set(email.to_string()),
            password_hash: Set(password_hash),
            balance: Set(0),
            role: Set("user".to_string()),
            status: Set("active".to_string()),
            totp_secret: Set(None),
            totp_enabled: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let user = user.insert(&self.db).await?;

        Ok(UserInfo {
            id: user.id,
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
            created_at: user.created_at,
        })
    }

    /// 用户登录
    pub async fn login(&self, email: &str, password: &str) -> Result<(UserInfo, String)> {
        // 查找用户
        let user = users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

        // 验证密码
        if !self.verify_password(password, &user.password_hash)? {
            bail!("Invalid credentials");
        }

        // 检查状态
        if user.status != "active" {
            bail!("Account is not active");
        }

        // 生成 JWT
        let token = self.generate_token(&user)?;

        let user_info = UserInfo {
            id: user.id,
            email: user.email,
            role: user.role,
            status: user.status,
            balance: user.balance,
            created_at: user.created_at,
        };

        Ok((user_info, token))
    }

    /// 根据 ID 获取用户
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<UserInfo>> {
        let user = users::Entity::find_by_id(id)
            .one(&self.db)
            .await?;

        Ok(user.map(|u| UserInfo {
            id: u.id,
            email: u.email,
            role: u.role,
            status: u.status,
            balance: u.balance,
            created_at: u.created_at,
        }))
    }

    /// 更新余额
    pub async fn update_balance(&self, id: Uuid, delta: i64) -> Result<i64> {
        let user = users::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let new_balance = user.balance + delta;
        if new_balance < 0 {
            bail!("Insufficient balance");
        }

        let mut user: users::ActiveModel = user.into();
        user.balance = Set(new_balance);
        user.updated_at = Set(Utc::now());
        let user = user.update(&self.db).await?;

        Ok(user.balance)
    }

    /// 列出所有用户（管理后台）
    pub async fn list_all(&self, page: u64, per_page: u64) -> Result<Vec<UserInfo>> {
        let users = users::Entity::find()
            .order_by_desc(users::Column::CreatedAt)
            .paginate(&self.db, per_page)
            .fetch_page(page.saturating_sub(1))
            .await?;

        Ok(users.into_iter().map(|u| UserInfo {
            id: u.id,
            email: u.email,
            role: u.role,
            status: u.status,
            balance: u.balance,
            created_at: u.created_at,
        }).collect())
    }

    /// 生成 JWT Token
    pub fn generate_token(&self, user: &users::Model) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expire_hours as i64);
        
        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// 验证 JWT Token
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// 哈希密码
    fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt)?;
        Ok(hash.to_string())
    }

    /// 验证密码
    fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)?;
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }
}
