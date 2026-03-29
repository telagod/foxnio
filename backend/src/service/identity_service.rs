use crate::model::user::User;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};
use uuid::Uuid;

/// Identity service for user identity management
pub struct IdentityService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserIdentity {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IdentityProvider {
    Email,
    Google,
    GitHub,
    WeChat,
    Apple,
    Custom(String),
}

impl std::fmt::Display for IdentityProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityProvider::Email => write!(f, "email"),
            IdentityProvider::Google => write!(f, "google"),
            IdentityProvider::GitHub => write!(f, "github"),
            IdentityProvider::WeChat => write!(f, "wechat"),
            IdentityProvider::Apple => write!(f, "apple"),
            IdentityProvider::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("Identity not found")]
    NotFound,
    #[error("Identity already exists")]
    AlreadyExists,
    #[error("Invalid provider")]
    InvalidProvider,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl IdentityService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new identity for a user
    pub async fn create_identity(
        &self,
        user_id: Uuid,
        provider: IdentityProvider,
        provider_user_id: String,
        email: Option<String>,
        name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<UserIdentity, IdentityError> {
        let provider_str = provider.to_string();

        let identity = query_as::<_, UserIdentity>(
            r#"
            INSERT INTO user_identities (
                user_id, provider, provider_user_id, email, name, avatar_url
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                user_id, provider, provider_user_id, email, name, avatar_url,
                created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(&provider_str)
        .bind(&provider_user_id)
        .bind(&email)
        .bind(&name)
        .bind(&avatar_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(identity)
    }

    /// Get identity by provider and provider_user_id
    pub async fn get_by_provider(
        &self,
        provider: &IdentityProvider,
        provider_user_id: &str,
    ) -> Result<Option<UserIdentity>, IdentityError> {
        let provider_str = provider.to_string();

        let identity = query_as::<_, UserIdentity>(
            r#"
            SELECT
                user_id, provider, provider_user_id, email, name, avatar_url,
                created_at, updated_at
            FROM user_identities
            WHERE provider = $1 AND provider_user_id = $2
            "#,
        )
        .bind(&provider_str)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(identity)
    }

    /// Get all identities for a user
    pub async fn get_user_identities(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<UserIdentity>, IdentityError> {
        let identities = query_as::<_, UserIdentity>(
            r#"
            SELECT
                user_id, provider, provider_user_id, email, name, avatar_url,
                created_at, updated_at
            FROM user_identities
            WHERE user_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(identities)
    }

    /// Update identity
    pub async fn update_identity(
        &self,
        user_id: Uuid,
        provider: &IdentityProvider,
        provider_user_id: &str,
        email: Option<String>,
        name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<UserIdentity, IdentityError> {
        let provider_str = provider.to_string();

        let identity = query_as::<_, UserIdentity>(
            r#"
            UPDATE user_identities
            SET
                email = COALESCE($3, email),
                name = COALESCE($4, name),
                avatar_url = COALESCE($5, avatar_url),
                updated_at = NOW()
            WHERE user_id = $1 AND provider = $2 AND provider_user_id = $6
            RETURNING
                user_id, provider, provider_user_id, email, name, avatar_url,
                created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(&provider_str)
        .bind(&email)
        .bind(&name)
        .bind(&avatar_url)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IdentityError::NotFound)?;

        Ok(identity)
    }

    /// Delete identity
    pub async fn delete_identity(
        &self,
        user_id: Uuid,
        provider: &IdentityProvider,
    ) -> Result<(), IdentityError> {
        let provider_str = provider.to_string();

        let result = query("DELETE FROM user_identities WHERE user_id = $1 AND provider = $2")
            .bind(user_id)
            .bind(&provider_str)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(IdentityError::NotFound);
        }

        Ok(())
    }

    /// Find or create user by identity
    pub async fn find_or_create_user(
        &self,
        provider: IdentityProvider,
        provider_user_id: String,
        email: Option<String>,
        name: Option<String>,
    ) -> Result<User, IdentityError> {
        // Try to find existing identity
        if let Some(identity) = self.get_by_provider(&provider, &provider_user_id).await? {
            let user = query_as::<_, User>("SELECT * FROM users WHERE id = $1")
                .bind(identity.user_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(IdentityError::NotFound)?;
            return Ok(user);
        }

        // Create new user
        let user = query_as::<_, User>(
            r#"
            INSERT INTO users (email, role, password_hash, balance, status)
            VALUES ($1, 'user', '', 0, 'active')
            RETURNING *
            "#,
        )
        .bind(&email)
        .fetch_one(&self.pool)
        .await?;

        // Create identity for new user
        self.create_identity(user.id, provider, provider_user_id, email, name, None)
            .await?;

        Ok(user)
    }

    /// Link identity to existing user
    pub async fn link_identity(
        &self,
        user_id: Uuid,
        provider: IdentityProvider,
        provider_user_id: String,
        email: Option<String>,
        name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<UserIdentity, IdentityError> {
        // Check if identity already exists
        if self
            .get_by_provider(&provider, &provider_user_id)
            .await?
            .is_some()
        {
            return Err(IdentityError::AlreadyExists);
        }

        self.create_identity(user_id, provider, provider_user_id, email, name, avatar_url)
            .await
    }

    /// Unlink identity from user
    pub async fn unlink_identity(
        &self,
        user_id: Uuid,
        provider: &IdentityProvider,
    ) -> Result<(), IdentityError> {
        // Ensure user has at least one identity
        let identities = self.get_user_identities(user_id).await?;
        if identities.len() <= 1 {
            return Err(IdentityError::InvalidProvider);
        }

        self.delete_identity(user_id, provider).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_provider_display() {
        assert_eq!(IdentityProvider::Email.to_string(), "email");
        assert_eq!(IdentityProvider::Google.to_string(), "google");
        assert_eq!(
            IdentityProvider::Custom("custom".to_string()).to_string(),
            "custom"
        );
    }
}
