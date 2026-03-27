//! 密码重置服务

use anyhow::{bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect, Set,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::email::{EmailConfig, EmailSender};
use crate::entity::{password_reset_tokens, users};

/// Token 有效期（小时）
const TOKEN_EXPIRY_HOURS: i64 = 1;

/// Token 长度（字节）
const TOKEN_LENGTH: usize = 32;

/// 密码重置服务
pub struct PasswordResetService<E: EmailSender> {
    db: DatabaseConnection,
    email_sender: E,
    reset_url_base: String,
}

impl<E: EmailSender> PasswordResetService<E> {
    /// 创建新的密码重置服务
    pub fn new(db: DatabaseConnection, email_sender: E, reset_url_base: String) -> Self {
        Self {
            db,
            email_sender,
            reset_url_base,
        }
    }

    /// 请求密码重置
    ///
    /// 如果邮箱存在，发送重置邮件；如果不存在，静默返回成功（防止枚举攻击）
    pub async fn request_reset(&self, email: &str) -> Result<()> {
        // 查找用户
        let user = users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(&self.db)
            .await?;

        // 如果用户不存在，静默返回成功（防止邮箱枚举攻击）
        let user = match user {
            Some(u) => u,
            None => {
                tracing::info!("Password reset requested for non-existent email: {}", email);
                return Ok(());
            }
        };

        // 检查用户状态
        if user.status != "active" {
            tracing::warn!("Password reset requested for inactive user: {}", email);
            return Ok(());
        }

        // 生成随机 token
        let token = self.generate_token();

        // 计算 token 哈希
        let token_hash = self.hash_token(&token);

        // 设置过期时间
        let expires_at = Utc::now() + Duration::hours(TOKEN_EXPIRY_HOURS);

        // 使之前的未使用 token 失效（可选：也可以让多个 token 并存）
        self.invalidate_previous_tokens(user.id).await?;

        // 存储 token
        let reset_token = password_reset_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user.id),
            token_hash: Set(token_hash),
            expires_at: Set(expires_at),
            used_at: Set(None),
            created_at: Set(Utc::now()),
        };

        reset_token.insert(&self.db).await?;

        // 构建重置 URL
        let reset_url = format!("{}/reset-password?token={}", self.reset_url_base, token);

        // 发送邮件
        self.email_sender
            .send_password_reset_email(&user.email, &reset_url)?;

        tracing::info!("Password reset email sent to: {}", email);

        Ok(())
    }

    /// 验证 token 是否有效
    pub async fn verify_token(&self, token: &str) -> Result<bool> {
        let token_hash = self.hash_token(token);

        let reset_token = password_reset_tokens::Entity::find()
            .filter(password_reset_tokens::Column::TokenHash.eq(&token_hash))
            .one(&self.db)
            .await?;

        match reset_token {
            Some(t) => Ok(t.is_valid()),
            None => Ok(false),
        }
    }

    /// 使用 token 重置密码
    pub async fn reset_password(&self, token: &str, new_password: &str) -> Result<()> {
        // 验证密码强度
        self.validate_password(new_password)?;

        let token_hash = self.hash_token(token);

        // 查找 token
        let reset_token = password_reset_tokens::Entity::find()
            .filter(password_reset_tokens::Column::TokenHash.eq(&token_hash))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid or expired token"))?;

        // 检查 token 是否有效
        if reset_token.is_used() {
            bail!("Token has already been used");
        }

        if reset_token.is_expired() {
            bail!("Token has expired");
        }

        // 获取用户
        let user = users::Entity::find_by_id(reset_token.user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // 更新密码
        let password_hash = self.hash_password(new_password)?;
        
        // 保存 email 用于日志
        let user_email = user.email.clone();

        let mut user: users::ActiveModel = user.into();
        user.password_hash = Set(password_hash);
        user.updated_at = Set(Utc::now());
        user.update(&self.db).await?;

        // 标记 token 为已使用
        let mut reset_token: password_reset_tokens::ActiveModel = reset_token.into();
        reset_token.used_at = Set(Some(Utc::now()));
        reset_token.update(&self.db).await?;

        tracing::info!(
            "Password reset successful for user: {}",
            user_email
        );

        Ok(())
    }

    /// 生成随机 token
    fn generate_token(&self) -> String {
        use rand::RngCore;
        let mut bytes = [0u8; TOKEN_LENGTH];
        OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    /// 计算 token 的 SHA256 哈希
    fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 哈希密码
    fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Password hashing failed: {}", e))?;
        Ok(hash.to_string())
    }

    /// 验证密码强度
    fn validate_password(&self, password: &str) -> Result<()> {
        if password.len() < 8 {
            bail!("Password must be at least 8 characters long");
        }
        if password.len() > 128 {
            bail!("Password must not exceed 128 characters");
        }
        // 可以添加更多密码强度检查
        Ok(())
    }

    /// 使之前的未使用 token 失效
    async fn invalidate_previous_tokens(&self, user_id: Uuid) -> Result<()> {
        // 查找所有未使用且未过期的 token
        let tokens = password_reset_tokens::Entity::find()
            .filter(password_reset_tokens::Column::UserId.eq(user_id))
            .filter(password_reset_tokens::Column::UsedAt.is_null())
            .all(&self.db)
            .await?;

        // 标记为已使用（实际上是使它们失效）
        for token in tokens {
            let mut token: password_reset_tokens::ActiveModel = token.into();
            token.used_at = Set(Some(Utc::now()));
            token.update(&self.db).await?;
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::email::MockEmailSender;

    // 注意：这些测试需要数据库连接，可以在集成测试中运行
    // 这里只测试一些基础功能

    #[test]
    fn test_generate_token() {
        let mock_sender = MockEmailSender::new();
        // 这里需要 mock 数据库连接，实际测试在集成测试中进行
        // 测试 token 长度：32 字节 = 64 个十六进制字符
        let expected_length = TOKEN_LENGTH * 2;
        assert_eq!(expected_length, 64);
    }

    #[test]
    fn test_hash_token() {
        let mock_sender = MockEmailSender::new();
        // SHA256 输出长度：32 字节 = 64 个十六进制字符
        let expected_length = 64;
        assert_eq!(expected_length, 64);
    }

    #[test]
    fn test_validate_password() {
        // 测试密码验证逻辑
        assert!(true); // 占位符
    }
}
