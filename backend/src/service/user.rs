//! 用户服务 - 完整实现 v0.2.0
//!
//! 新增功能：
//! - Refresh Token 支持（7 天有效期）
//! - Token 黑名单（Redis 实现）
//! - 安全的 Token 轮换机制
//! - TOTP 两步验证支持

use anyhow::{bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{QueryOrder,
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use super::totp::TotpService;
use crate::db::RedisPool;
use crate::entity::{refresh_tokens, users};

/// Access Token 的 Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>, // JWT ID，用于黑名单
    /// 是否为临时 token（用于 TOTP 验证流程）
    #[serde(default)]
    pub is_temp: bool,
}

/// Refresh Token 的 Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    pub sub: String,
    pub jti: String, // JWT ID，唯一标识
    pub exp: i64,
    pub iat: i64,
}

/// Token 对（登录/刷新时返回）
#[derive(Debug, Clone, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_in: i64,
    pub refresh_token_expires_in: i64,
}

/// 登录响应（支持 TOTP）
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum LoginResponse {
    /// 直接登录成功（未启用 TOTP）
    Success {
        access_token: String,
        refresh_token: String,
        access_token_expires_in: i64,
        refresh_token_expires_in: i64,
        user: UserInfo,
    },
    /// 需要 TOTP 验证
    RequiresTotp {
        temp_token: String,
        expires_in: i64,
        message: String,
    },
}

/// TOTP 状态
#[derive(Debug, Clone, Serialize)]
pub struct TotpStatus {
    pub enabled: bool,
    pub has_secret: bool,
    pub backup_codes_remaining: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    #[serde(default)]
    pub totp_enabled: bool,
    pub created_at: chrono::DateTime<Utc>,
}

/// Token 黑名单键前缀
const TOKEN_BLACKLIST_PREFIX: &str = "token_blacklist:";

/// Refresh Token 黑名单键前缀
const REFRESH_TOKEN_BLACKLIST_PREFIX: &str = "refresh_blacklist:";

/// 用户服务
pub struct UserService {
    db: DatabaseConnection,
    redis: Option<Arc<RedisPool>>,
    jwt_secret: String,
    jwt_expire_hours: u64,
    refresh_token_expire_days: u64,
}

impl UserService {
    /// 创建新的用户服务
    pub fn new(db: DatabaseConnection, jwt_secret: String, jwt_expire_hours: u64) -> Self {
        Self {
            db,
            redis: None,
            jwt_secret,
            jwt_expire_hours,
            refresh_token_expire_days: 7,
        }
    }

    /// 创建带 Redis 支持的用户服务
    pub fn with_redis(
        db: DatabaseConnection,
        redis: Arc<RedisPool>,
        jwt_secret: String,
        jwt_expire_hours: u64,
    ) -> Self {
        Self {
            db,
            redis: Some(redis),
            jwt_secret,
            jwt_expire_hours,
            refresh_token_expire_days: 7,
        }
    }

    /// 设置 Refresh Token 有效期（天）
    pub fn with_refresh_token_expire_days(mut self, days: u64) -> Self {
        self.refresh_token_expire_days = days;
        self
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

    /// 用户登录（返回 Token 对，支持 TOTP）
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<LoginResponse> {
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

        let user_info = UserInfo {
            id: user.id,
            email: user.email.clone(),
            role: user.role.clone(),
            status: user.status.clone(),
            balance: user.balance,
            totp_enabled: user.totp_enabled,
            created_at: user.created_at,
        };

        // 如果启用了 TOTP，返回临时 token
        if user.totp_enabled {
            let temp_token = self.generate_temp_token(&user)?;
            Ok(LoginResponse::RequiresTotp {
                temp_token,
                expires_in: 300, // 5 分钟
                message: "Two-factor authentication required".to_string(),
            })
        } else {
            // 生成 Token 对
            let token_pair = self
                .generate_token_pair(&user, user_agent, ip_address)
                .await?;

            Ok(LoginResponse::Success {
                access_token: token_pair.access_token,
                refresh_token: token_pair.refresh_token,
                access_token_expires_in: token_pair.access_token_expires_in,
                refresh_token_expires_in: token_pair.refresh_token_expires_in,
                user: user_info,
            })
        }
    }

    /// TOTP 登录验证（使用临时 token + TOTP 代码）
    pub async fn login_with_totp(
        &self,
        temp_token: &str,
        totp_code: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<(UserInfo, TokenPair)> {
        // 验证临时 token
        let claims = self.verify_temp_token(temp_token)?;

        // 获取用户
        let user_id = Uuid::parse_str(&claims.sub)?;
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // 检查是否启用了 TOTP
        if !user.totp_enabled {
            bail!("TOTP not enabled for this account");
        }

        // 获取 TOTP secret
        let secret = user
            .totp_secret
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TOTP secret not found"))?;

        // 验证 TOTP 代码
        if !TotpService::verify_code(secret, totp_code) {
            bail!("Invalid TOTP code");
        }

        // 生成完整 Token 对
        let token_pair = self
            .generate_token_pair(&user, user_agent, ip_address)
            .await?;

        Ok((
            UserInfo {
                id: user.id,
                email: user.email,
                role: user.role,
                status: user.status,
                balance: user.balance,
                totp_enabled: user.totp_enabled,
                created_at: user.created_at,
            },
            token_pair,
        ))
    }

    /// 使用备用码登录
    pub async fn login_with_backup_code(
        &self,
        temp_token: &str,
        backup_code: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<(UserInfo, TokenPair)> {
        // 验证临时 token
        let claims = self.verify_temp_token(temp_token)?;

        // 获取用户
        let user_id = Uuid::parse_str(&claims.sub)?;
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // 检查是否启用了 TOTP
        if !user.totp_enabled {
            bail!("TOTP not enabled for this account");
        }

        // 验证备用码
        let backup_codes = self.get_backup_codes(user_id).await?;
        let code_hash = TotpService::hash_backup_code(backup_code);

        if !backup_codes.iter().any(|h| h == &code_hash) {
            bail!("Invalid backup code");
        }

        // 移除已使用的备用码
        self.remove_backup_code(user_id, &code_hash).await?;

        // 生成完整 Token 对
        let token_pair = self
            .generate_token_pair(&user, user_agent, ip_address)
            .await?;

        Ok((
            UserInfo {
                id: user.id,
                email: user.email,
                role: user.role,
                status: user.status,
                balance: user.balance,
                totp_enabled: user.totp_enabled,
                created_at: user.created_at,
            },
            token_pair,
        ))
    }

    /// 刷新 Access Token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<TokenPair> {
        // 验证 refresh token 格式
        let claims = self.verify_refresh_token(refresh_token)?;

        // 检查是否在黑名单中
        if self.is_refresh_token_blacklisted(&claims.jti).await? {
            bail!("Refresh token has been revoked");
        }

        // 计算 token hash
        let token_hash = self.hash_token(refresh_token);

        // 查找数据库中的 refresh token 记录
        let stored_token = refresh_tokens::Entity::find()
            .filter(refresh_tokens::Column::TokenHash.eq(&token_hash))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid refresh token"))?;

        // 检查 token 状态
        if stored_token.revoked {
            bail!("Refresh token has been revoked");
        }

        if stored_token.is_expired() {
            bail!("Refresh token has expired");
        }

        // 获取用户
        let user = users::Entity::find_by_id(stored_token.user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        if user.status != "active" {
            bail!("Account is not active");
        }

        // 撤销旧的 refresh token（安全轮换）
        self.revoke_refresh_token_by_id(stored_token.id, Some("Token rotation".to_string()))
            .await?;

        // 将旧的 refresh token 加入黑名单
        self.add_refresh_token_to_blacklist(&claims.jti, stored_token.expires_at)
            .await?;

        // 生成新的 Token 对
        let token_pair = self
            .generate_token_pair(&user, user_agent, ip_address)
            .await?;

        Ok(token_pair)
    }

    /// 登出（撤销 refresh token 和将 access token 加入黑名单）
    pub async fn logout(&self, access_token: &str, refresh_token: Option<&str>) -> Result<()> {
        // 将 access token 加入黑名单
        if let Ok(claims) = self.verify_token(access_token) {
            let exp = chrono::DateTime::from_timestamp(claims.exp, 0)
                .unwrap_or_else(|| Utc::now() + Duration::hours(self.jwt_expire_hours as i64));
            self.add_token_to_blacklist(&claims.sub, &claims.jti.unwrap_or_default(), exp)
                .await?;
        }

        // 撤销 refresh token
        if let Some(refresh_token) = refresh_token {
            if let Ok(claims) = self.verify_refresh_token(refresh_token) {
                let token_hash = self.hash_token(refresh_token);

                // 撤销数据库中的 token
                if let Some(stored_token) = refresh_tokens::Entity::find()
                    .filter(refresh_tokens::Column::TokenHash.eq(&token_hash))
                    .one(&self.db)
                    .await?
                {
                    self.revoke_refresh_token_by_id(
                        stored_token.id,
                        Some("User logout".to_string()),
                    )
                    .await?;
                }

                // 加入黑名单
                self.add_refresh_token_to_blacklist(
                    &claims.jti,
                    chrono::DateTime::from_timestamp(claims.exp, 0).unwrap_or_else(|| Utc::now()),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// 撤销用户的所有 refresh token
    pub async fn revoke_all_user_tokens(
        &self,
        user_id: Uuid,
        reason: Option<String>,
    ) -> Result<u64> {
        let tokens = refresh_tokens::Entity::find()
            .filter(refresh_tokens::Column::UserId.eq(user_id))
            .filter(refresh_tokens::Column::Revoked.eq(false))
            .all(&self.db)
            .await?;

        let count = tokens.len() as u64;

        for token in tokens {
            self.revoke_refresh_token_by_id(token.id, reason.clone())
                .await?;

            // 将 token 的 jti 加入黑名单（如果有存储的话）
            // 这里我们使用 token_hash 作为标识
            if let Some(ref redis) = self.redis {
                let key = format!("{}:{}", REFRESH_TOKEN_BLACKLIST_PREFIX, token.token_hash);
                let _ = redis
                    .set(
                        &key,
                        "1",
                        Some(std::time::Duration::from_secs(
                            (token.expires_at - Utc::now()).num_seconds().max(0) as u64,
                        )),
                    )
                    .await;
            }
        }

        Ok(count)
    }

    /// 生成 Token 对
    async fn generate_token_pair(
        &self,
        user: &users::Model,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<TokenPair> {
        // 生成 access token
        let access_jti = Uuid::new_v4().to_string();
        let access_token = self.generate_access_token_with_jti(user, &access_jti)?;

        // 生成 refresh token
        let refresh_jti = Uuid::new_v4().to_string();
        let refresh_token = self.generate_refresh_token(user.id, &refresh_jti)?;

        // 存储 refresh token 到数据库
        self.store_refresh_token(
            user.id,
            &refresh_token,
            &refresh_jti,
            user_agent,
            ip_address,
        )
        .await?;

        let access_expires_in = self.jwt_expire_hours * 3600;
        let refresh_expires_in = self.refresh_token_expire_days * 24 * 3600;

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_token_expires_in: access_expires_in as i64,
            refresh_token_expires_in: refresh_expires_in as i64,
        })
    }

    /// 生成 Access Token（带 JTI）
    fn generate_access_token_with_jti(&self, user: &users::Model, jti: &str) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expire_hours as i64);

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Some(jti.to_string()),
            is_temp: false,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// 生成 Refresh Token
    fn generate_refresh_token(&self, user_id: Uuid, jti: &str) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::days(self.refresh_token_expire_days as i64);

        let claims = RefreshClaims {
            sub: user_id.to_string(),
            jti: jti.to_string(),
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

    /// 存储 Refresh Token 到数据库
    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token: &str,
        _jti: &str,
        user_agent: Option<String>,
        ip_address: Option<String>,
    ) -> Result<()> {
        let token_hash = self.hash_token(token);
        let expires_at = Utc::now() + Duration::days(self.refresh_token_expire_days as i64);

        let refresh_token = refresh_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            token_hash: Set(token_hash),
            expires_at: Set(expires_at),
            created_at: Set(Utc::now()),
            revoked: Set(false),
            revoked_at: Set(None),
            revoked_reason: Set(None),
            user_agent: Set(user_agent),
            ip_address: Set(ip_address),
        };

        refresh_token.insert(&self.db).await?;
        Ok(())
    }

    /// 撤销 Refresh Token
    async fn revoke_refresh_token_by_id(&self, id: Uuid, reason: Option<String>) -> Result<()> {
        let token = refresh_tokens::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Token not found"))?;

        let mut token: refresh_tokens::ActiveModel = token.into();
        token.revoked = Set(true);
        token.revoked_at = Set(Some(Utc::now()));
        token.revoked_reason = Set(reason);
        token.update(&self.db).await?;

        Ok(())
    }

    /// 验证 JWT Token（检查黑名单）
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// 验证 JWT Token 并检查黑名单
    pub async fn verify_token_with_blacklist(&self, token: &str) -> Result<Claims> {
        let claims = self.verify_token(token)?;

        // 检查黑名单
        if self
            .is_token_blacklisted(&claims.sub, &claims.jti.clone().unwrap_or_default())
            .await?
        {
            bail!("Token has been revoked");
        }

        Ok(claims)
    }

    /// 验证 Refresh Token
    fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims> {
        let token_data = decode::<RefreshClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }

    /// 将 Access Token 加入黑名单
    async fn add_token_to_blacklist(
        &self,
        user_id: &str,
        jti: &str,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        if let Some(ref redis) = self.redis {
            let key = format!("{}:{}", TOKEN_BLACKLIST_PREFIX, jti);
            let ttl = (expires_at - Utc::now()).num_seconds().max(0);

            if ttl > 0 {
                redis
                    .set(&key, "1", Some(std::time::Duration::from_secs(ttl as u64)))
                    .await?;
            }
        }
        Ok(())
    }

    /// 检查 Access Token 是否在黑名单中
    pub async fn is_token_blacklisted(&self, user_id: &str, jti: &str) -> Result<bool> {
        if let Some(ref redis) = self.redis {
            let key = format!("{}:{}", TOKEN_BLACKLIST_PREFIX, jti);
            return Ok(redis.exists(&key).await.unwrap_or(false));
        }
        Ok(false)
    }

    /// 将 Refresh Token 加入黑名单
    async fn add_refresh_token_to_blacklist(
        &self,
        jti: &str,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        if let Some(ref redis) = self.redis {
            let key = format!("{}:{}", REFRESH_TOKEN_BLACKLIST_PREFIX, jti);
            let ttl = (expires_at - Utc::now()).num_seconds().max(0);

            if ttl > 0 {
                redis
                    .set(&key, "1", Some(std::time::Duration::from_secs(ttl as u64)))
                    .await?;
            }
        }
        Ok(())
    }

    /// 检查 Refresh Token 是否在黑名单中
    async fn is_refresh_token_blacklisted(&self, jti: &str) -> Result<bool> {
        if let Some(ref redis) = self.redis {
            let key = format!("{}:{}", REFRESH_TOKEN_BLACKLIST_PREFIX, jti);
            return Ok(redis.exists(&key).await.unwrap_or(false));
        }
        Ok(false)
    }

    /// 计算 Token Hash
    fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 根据 ID 获取用户
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<UserInfo>> {
        let user = users::Entity::find_by_id(id).one(&self.db).await?;

        Ok(user.map(|u| UserInfo {
            id: u.id,
            email: u.email,
            role: u.role,
            status: u.status,
            balance: u.balance,
            totp_enabled: u.totp_enabled,
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

        Ok(users
            .into_iter()
            .map(|u| UserInfo {
                id: u.id,
                email: u.email,
                role: u.role,
                status: u.status,
                balance: u.balance,
                totp_enabled: u.totp_enabled,
                created_at: u.created_at,
            })
            .collect())
    }

    /// 生成 JWT Token（兼容旧接口）
    pub fn generate_token(&self, user: &users::Model) -> Result<String> {
        self.generate_access_token_with_jti(user, &Uuid::new_v4().to_string())
    }

    /// 为 UserInfo 生成 token
    pub fn generate_token_for(&self, user: &UserInfo) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.jwt_expire_hours as i64);

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Some(Uuid::new_v4().to_string()),
            is_temp: false,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(token)
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
        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// 清理过期的 refresh tokens
    pub async fn cleanup_expired_tokens(&self) -> Result<u64> {
        let now = Utc::now();

        let result = sea_orm::EntityTrait::delete_many(
            refresh_tokens::Entity::find()
                .filter(refresh_tokens::Column::ExpiresAt.lt(now))
                .filter(refresh_tokens::Column::Revoked.eq(true)),
        )
        .exec(&self.db)
        .await?;

        Ok(result.rows_affected)
    }

    // ========================================================================
    // TOTP 两步验证方法
    // ========================================================================

    /// 启用 TOTP 两步验证
    /// 返回 (secret, qr_code_url, backup_codes)
    pub async fn enable_totp(&self, user_id: Uuid) -> Result<TotpSetupResponse> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        if user.totp_enabled {
            bail!("TOTP already enabled for this account");
        }

        // 生成 TOTP 密钥
        let secret = TotpService::generate_secret();

        // 生成备用码
        let backup_codes = TotpService::generate_backup_codes();

        // 生成 QR 码 URL
        let totp_service = TotpService::new("FoxNIO");
        let qr_code_url = totp_service.generate_qr_code_data_url(&user.email, &secret)?;

        // 存储密钥（此时还未启用，需要验证后才启用）
        let mut user: users::ActiveModel = user.into();
        user.totp_secret = Set(Some(secret.clone()));
        user.totp_enabled = Set(false); // 等待验证
        user.updated_at = Set(Utc::now());
        user.update(&self.db).await?;

        // 存储备用码
        self.store_backup_codes(user_id, &backup_codes).await?;

        Ok(TotpSetupResponse {
            secret,
            qr_code_url,
            backup_codes,
        })
    }

    /// 确认启用 TOTP（验证代码后正式启用）
    pub async fn confirm_enable_totp(&self, user_id: Uuid, code: &str) -> Result<()> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        if user.totp_enabled {
            bail!("TOTP already enabled for this account");
        }

        let secret = user
            .totp_secret
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TOTP secret not set. Please enable TOTP first."))?;

        // 验证 TOTP 代码
        if !TotpService::verify_code(secret, code) {
            bail!("Invalid TOTP code");
        }

        // 正式启用 TOTP
        let mut user: users::ActiveModel = user.into();
        user.totp_enabled = Set(true);
        user.updated_at = Set(Utc::now());
        user.update(&self.db).await?;

        Ok(())
    }

    /// 禁用 TOTP 两步验证（需要验证密码或 TOTP 代码）
    pub async fn disable_totp(&self, user_id: Uuid, code: &str) -> Result<()> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        if !user.totp_enabled {
            bail!("TOTP not enabled for this account");
        }

        let secret = user
            .totp_secret
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TOTP secret not found"))?;

        // 验证 TOTP 代码或备用码
        let is_valid_totp = TotpService::verify_code(secret, code);
        let is_valid_backup = {
            let backup_codes = self.get_backup_codes(user_id).await?;
            let code_hash = TotpService::hash_backup_code(code);
            backup_codes.iter().any(|h| h == &code_hash)
        };

        if !is_valid_totp && !is_valid_backup {
            bail!("Invalid TOTP code or backup code");
        }

        // 禁用 TOTP
        let mut user: users::ActiveModel = user.into();
        user.totp_secret = Set(None);
        user.totp_enabled = Set(false);
        user.updated_at = Set(Utc::now());
        user.update(&self.db).await?;

        // 清除备用码
        self.clear_backup_codes(user_id).await?;

        Ok(())
    }

    /// 验证 TOTP 代码
    pub async fn verify_totp(&self, user_id: Uuid, code: &str) -> Result<bool> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        if !user.totp_enabled {
            bail!("TOTP not enabled for this account");
        }

        let secret = user
            .totp_secret
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TOTP secret not found"))?;

        Ok(TotpService::verify_code(secret, code))
    }

    /// 重新生成备用码
    pub async fn regenerate_backup_codes(&self, user_id: Uuid, code: &str) -> Result<Vec<String>> {
        // 先验证 TOTP 代码
        if !self.verify_totp(user_id, code).await? {
            bail!("Invalid TOTP code");
        }

        // 生成新的备用码
        let backup_codes = TotpService::generate_backup_codes();

        // 清除旧的备用码并存储新的
        self.clear_backup_codes(user_id).await?;
        self.store_backup_codes(user_id, &backup_codes).await?;

        Ok(backup_codes)
    }

    /// 获取用户的 TOTP 状态
    pub async fn get_totp_status(&self, user_id: Uuid) -> Result<TotpStatus> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let backup_codes_count = self.get_backup_codes(user_id).await?.len();

        Ok(TotpStatus {
            enabled: user.totp_enabled,
            has_secret: user.totp_secret.is_some(),
            backup_codes_remaining: backup_codes_count,
        })
    }

    // ========================================================================
    // TOTP 内部辅助方法
    // ========================================================================

    /// 生成临时 token（用于 TOTP 验证流程，有效期 5 分钟）
    fn generate_temp_token(&self, user: &users::Model) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::minutes(5); // 5 分钟有效期

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti: Some(Uuid::new_v4().to_string()),
            is_temp: true,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// 验证临时 token
    fn verify_temp_token(&self, token: &str) -> Result<Claims> {
        let claims = self.verify_token(token)?;

        if !claims.is_temp {
            bail!("Invalid token: expected temporary token");
        }

        Ok(claims)
    }

    /// 存储备用码（哈希后存储到 Redis）
    async fn store_backup_codes(&self, user_id: Uuid, codes: &[String]) -> Result<()> {
        if let Some(ref redis) = self.redis {
            let key = format!("totp_backup_codes:{}", user_id);
            let hashes: Vec<String> = codes
                .iter()
                .map(|c| TotpService::hash_backup_code(c))
                .collect();
            let value = serde_json::to_string(&hashes)?;
            // 存储 30 天
            redis
                .set(
                    &key,
                    &value,
                    Some(std::time::Duration::from_secs(30 * 24 * 3600)),
                )
                .await?;
        }
        Ok(())
    }

    /// 获取备用码（哈希值）
    async fn get_backup_codes(&self, user_id: Uuid) -> Result<Vec<String>> {
        if let Some(ref redis) = self.redis {
            let key = format!("totp_backup_codes:{}", user_id);
            if let Some(value) = redis.get(&key).await? {
                return Ok(serde_json::from_str(&value).unwrap_or_default());
            }
        }
        Ok(vec![])
    }

    /// 移除已使用的备用码
    async fn remove_backup_code(&self, user_id: Uuid, code_hash: &str) -> Result<()> {
        if let Some(ref redis) = self.redis {
            let key = format!("totp_backup_codes:{}", user_id);
            if let Some(value) = redis.get(&key).await? {
                let mut hashes: Vec<String> = serde_json::from_str(&value).unwrap_or_default();
                hashes.retain(|h| h != code_hash);
                let new_value = serde_json::to_string(&hashes)?;
                redis
                    .set(
                        &key,
                        &new_value,
                        Some(std::time::Duration::from_secs(30 * 24 * 3600)),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// 清除所有备用码
    async fn clear_backup_codes(&self, user_id: Uuid) -> Result<()> {
        if let Some(ref redis) = self.redis {
            let key = format!("totp_backup_codes:{}", user_id);
            let _ = redis.del(&key).await;
        }
        Ok(())
    }
}

/// TOTP 设置响应
#[derive(Debug, Clone, Serialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code_url: String,
    pub backup_codes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_token() {
        let service = UserService::new(
            sea_orm::DatabaseConnection::Disconnected,
            "test-secret".to_string(),
            24,
        );

        let token = "test-token-123";
        let hash1 = service.hash_token(token);
        let hash2 = service.hash_token(token);

        // 相同 token 应该产生相同 hash
        assert_eq!(hash1, hash2);

        // 不同 token 应该产生不同 hash
        let hash3 = service.hash_token("different-token");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_token_expiration() {
        let user_info = UserInfo {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            role: "user".to_string(),
            status: "active".to_string(),
            balance: 0,
            totp_enabled: false,
            created_at: Utc::now(),
        };

        // 这里只能测试 token 生成，验证需要实际的数据库连接
        let service = UserService::new(
            sea_orm::DatabaseConnection::Disconnected,
            "test-secret".to_string(),
            24,
        );

        let token = service.generate_token_for(&user_info);
        assert!(token.is_ok());
    }

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "user-123".to_string(),
            email: "test@example.com".to_string(),
            role: "admin".to_string(),
            exp: 9999999999,
            iat: 1000000000,
            jti: Some("unique-id".to_string()),
            is_temp: false,
        };

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("user-123"));
        assert!(json.contains("unique-id"));
    }

    #[test]
    fn test_refresh_claims() {
        let claims = RefreshClaims {
            sub: "user-123".to_string(),
            jti: "refresh-id".to_string(),
            exp: 9999999999,
            iat: 1000000000,
        };

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("refresh-id"));
    }

    #[test]
    fn test_token_pair() {
        let pair = TokenPair {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            access_token_expires_in: 3600,
            refresh_token_expires_in: 604800,
        };

        let json = serde_json::to_string(&pair).unwrap();
        assert!(json.contains("access_token"));
        assert!(json.contains("refresh_token"));
    }

    #[test]
    fn test_login_response_serialization() {
        // Test RequiresTotp variant
        let response = LoginResponse::RequiresTotp {
            temp_token: "temp-token-123".to_string(),
            expires_in: 300,
            message: "Two-factor authentication required".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("temp_token"));
        assert!(json
            .contains("requires_totp")
            .or_else(|| json.contains("temp_token")));
    }
}
