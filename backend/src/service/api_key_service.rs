use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{query, query_as, FromRow, PgPool};

fn sha256_hash(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

/// API key service for managing API keys
pub struct ApiKeyService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: i64,
    pub key_hash: String,
    pub key_prefix: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiKeyError {
    #[error("API key not found")]
    NotFound,
    #[error("API key expired")]
    Expired,
    #[error("Invalid API key")]
    InvalidKey,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl ApiKeyService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create API key
    pub async fn create(
        &self,
        user_id: i64,
        name: String,
        permissions: Vec<String>,
    ) -> Result<(ApiKey, String), ApiKeyError> {
        // Generate random key
        let key = format!("sk-{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let key_prefix = key[..10].to_string();
        let key_hash = sha256_hash(key.as_bytes());

        let api_key = query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (user_id, key_hash, key_prefix, name, permissions, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(name)
        .bind(&permissions)
        .fetch_one(&self.pool)
        .await?;

        Ok((api_key, key))
    }

    /// Validate API key
    pub async fn validate(&self, key: &str) -> Result<ApiKey, ApiKeyError> {
        let key_hash = sha256_hash(key.as_bytes());

        let api_key = query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE key_hash = $1")
            .bind(&key_hash)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ApiKeyError::InvalidKey)?;

        // Check expiration
        if let Some(expires_at) = api_key.expires_at {
            if expires_at < Utc::now() {
                return Err(ApiKeyError::Expired);
            }
        }

        // Update last used
        query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(api_key.id)
            .execute(&self.pool)
            .await?;

        Ok(api_key)
    }

    /// List API keys for user
    pub async fn list_for_user(&self, user_id: i64) -> Result<Vec<ApiKey>, ApiKeyError> {
        let keys = query_as::<_, ApiKey>(
            "SELECT * FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(keys)
    }

    /// Delete API key
    pub async fn delete(&self, id: i64) -> Result<(), ApiKeyError> {
        query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_service() {
        // Test would require database connection
    }
}
