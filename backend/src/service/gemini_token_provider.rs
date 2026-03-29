use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Token provider for Gemini API
pub struct GeminiTokenProvider {
    pool: PgPool,
    cache: Arc<RwLock<TokenCache>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GeminiToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub scope: Vec<String>,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCache {
    tokens: std::collections::HashMap<i64, GeminiToken>,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Token not found")]
    TokenNotFound,
    #[error("Token expired")]
    TokenExpired,
    #[error("Refresh failed")]
    RefreshFailed,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl GeminiTokenProvider {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(TokenCache {
                tokens: std::collections::HashMap::new(),
            })),
        }
    }

    /// Get valid token for account
    pub async fn get_token(&self, account_id: i64) -> Result<GeminiToken, TokenError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(token) = cache.tokens.get(&account_id) {
                if token.expires_at > Utc::now() {
                    return Ok(token.clone());
                }
            }
        }

        // Fetch from database
        let token = query_as::<_, GeminiToken>(
            r#"
            SELECT access_token, refresh_token, expires_at, scope, token_type
            FROM gemini_tokens
            WHERE account_id = $1
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(TokenError::TokenNotFound)?;

        // Check if expired
        if token.expires_at <= Utc::now() {
            return Err(TokenError::TokenExpired);
        }

        // Update cache
        let mut cache = self.cache.write().await;
        cache.tokens.insert(account_id, token.clone());

        Ok(token)
    }

    /// Store new token
    pub async fn store_token(&self, account_id: i64, token: GeminiToken) -> Result<(), TokenError> {
        query(r#"
            INSERT INTO gemini_tokens (account_id, access_token, refresh_token, expires_at, scope, token_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (account_id) DO UPDATE SET
                access_token = EXCLUDED.access_token,
                refresh_token = EXCLUDED.refresh_token,
                expires_at = EXCLUDED.expires_at,
                scope = EXCLUDED.scope,
                token_type = EXCLUDED.token_type
            "#)
            .bind(account_id)
            .bind(&token.access_token)
            .bind(&token.refresh_token)
            .bind(token.expires_at)
            .bind(&token.scope)
            .bind(&token.token_type)
            .execute(&self.pool)
            .await?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.tokens.insert(account_id, token);

        Ok(())
    }

    /// Invalidate token cache
    pub async fn invalidate(&self, account_id: i64) {
        let mut cache = self.cache.write().await;
        cache.tokens.remove(&account_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_validation() {
        // Test would require database connection
    }
}
