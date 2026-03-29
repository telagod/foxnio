use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Token provider for Antigravity API
pub struct AntigravityTokenProvider {
    pool: PgPool,
    cache: Arc<RwLock<TokenCache>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AntigravityToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub scope: Vec<String>,
}

#[derive(Debug, Default)]
struct TokenCache {
    tokens: std::collections::HashMap<i64, AntigravityToken>,
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

impl AntigravityTokenProvider {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(TokenCache::default())),
        }
    }

    /// Get valid token
    pub async fn get(&self, account_id: i64) -> Result<AntigravityToken, TokenError> {
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
        let token = query_as::<_, AntigravityToken>(
            r#"
            SELECT access_token, refresh_token, expires_at, scope
            FROM antigravity_tokens
            WHERE account_id = $1
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(TokenError::NotFound)?;

        if token.expires_at <= Utc::now() {
            return Err(TokenError::Expired);
        }

        // Update cache
        let mut cache = self.cache.write().await;
        cache.tokens.insert(account_id, token.clone());

        Ok(token)
    }

    /// Store token
    pub async fn store(&self, account_id: i64, token: AntigravityToken) -> Result<(), TokenError> {
        query(r#"
            INSERT INTO antigravity_tokens (account_id, access_token, refresh_token, expires_at, scope)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (account_id) DO UPDATE SET
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                expires_at = EXCLUDED.expires_at,
                scope = EXCLUDED.scope
            "#)
            .bind(account_id)
            .bind(&token.access_token)
            .bind(&token.refresh_token)
            .bind(token.expires_at)
            .bind(&token.scope)
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
    fn test_token_provider_creation() {
        // Test would require database connection
    }
}
