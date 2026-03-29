use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Session management for Gemini API
pub struct GeminiSession {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: i64,
    pub account_id: i64,
    pub model: String,
    pub context: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    NotFound,
    #[error("Session expired")]
    Expired,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl GeminiSession {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new session
    pub async fn create(
        &self,
        user_id: i64,
        account_id: i64,
        model: String,
        ttl_hours: i64,
    ) -> Result<Session, SessionError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(ttl_hours);

        let session = query_as::<_, Session>(r#"
            INSERT INTO gemini_sessions (user_id, account_id, model, created_at, updated_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#)
            .bind(user_id)
            .bind(account_id)
            .bind(&model)
            .bind(now)
            .bind(now)
            .bind(expires_at)
            .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Get session by ID
    pub async fn get(&self, session_id: &str) -> Result<Session, SessionError> {
        let session = query_as::<_, Session>("SELECT * FROM gemini_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(SessionError::NotFound)?;

        if session.expires_at < Utc::now() {
            return Err(SessionError::Expired);
        }

        Ok(session)
    }

    /// Add message to session
    pub async fn add_message(
        &self,
        session_id: &str,
        message: Message,
    ) -> Result<(), SessionError> {
        query(
            r#"
            UPDATE gemini_sessions
            SET context = context || $1::jsonb, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(serde_json::to_value(&message).unwrap())
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete session
    pub async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        query("DELETE FROM gemini_sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Cleanup expired sessions
    pub async fn cleanup_expired(&self) -> Result<u64, SessionError> {
        let result = query("DELETE FROM gemini_sessions WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        // Test would require database connection
    }
}
