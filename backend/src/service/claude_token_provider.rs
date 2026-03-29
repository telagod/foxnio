use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Token provider for Claude API
pub struct ClaudeTokenProvider {
    pool: PgPool,
    cache: Arc<RwLock<TokenCache>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClaudeToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct TokenCache {
    tokens: std::collections::HashMap<i64, ClaudeToken>,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Token not found")]
    NotFound,
    #[error("Token expired")]
    Expired,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl ClaudeTokenProvider {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(TokenCache::default())),
        }
    }

    /// Get valid token
    pub async fn get(&self, account_id: i64) -> Result<ClaudeToken, TokenError> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(token) = cache.tokens.get(&account_id) {
                if token.expires_at > Utc::now() {
                    return Ok(token.clone());
                }
            }
        }

        // Fetch from DB
        let token = query_as::<_, ClaudeToken>("SELECT access_token, refresh_token, expires_at FROM claude_tokens WHERE account_id = $1")
            .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(TokenError::NotFound)?;

        if token.expires_at <= Utc::now() {
            return Err(TokenError::Expired);
        }

        let mut cache = self.cache.write().await;
        cache.tokens.insert(account_id, token.clone());

        Ok(token)
    }

    /// Store token
    pub async fn store(&self, account_id: i64, token: ClaudeToken) -> Result<(), TokenError> {
        query(
            r#"
            INSERT INTO claude_tokens (account_id, access_token, refresh_token, expires_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (account_id) DO UPDATE SET
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                expires_at = EXCLUDED.expires_at
            "#,
        )
        .bind(account_id)
        .bind(&token.access_token)
        .bind(&token.refresh_token)
        .bind(token.expires_at)
        .execute(&self.pool)
        .await?;

        let mut cache = self.cache.write().await;
        cache.tokens.insert(account_id, token);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_provider() {
        // Test would require database connection
    }
}
