use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Session management for Anthropic API
pub struct AnthropicSession {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: i64,
    pub account_id: i64,
    pub messages: serde_json::Value,
    pub created_at: DateTime<Utc>,
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

impl AnthropicSession {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new session
    pub async fn create(
        &self,
        user_id: i64,
        account_id: i64,
        ttl_hours: i64,
    ) -> Result<Session, SessionError> {
        let now = Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        let session = query_as::<_, Session>(
            r#"
            INSERT INTO anthropic_sessions (id, user_id, account_id, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(account_id)
        .bind(now)
        .bind(now + chrono::Duration::hours(ttl_hours))
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Get session
    pub async fn get(&self, id: &str) -> Result<Session, SessionError> {
        let session = query_as::<_, Session>("SELECT * FROM anthropic_sessions WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(SessionError::NotFound)?;

        if session.expires_at < Utc::now() {
            return Err(SessionError::Expired);
        }

        Ok(session)
    }

    /// Add message
    pub async fn add_message(&self, id: &str, message: Message) -> Result<(), SessionError> {
        query(
            r#"
            UPDATE anthropic_sessions
            SET messages = messages || $1::jsonb
            WHERE id = $2
            "#,
        )
        .bind(serde_json::to_value(&message).unwrap())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete session
    pub async fn delete(&self, id: &str) -> Result<(), SessionError> {
        query("DELETE FROM anthropic_sessions WHERE id = $1")
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
    fn test_session_creation() {
        // Test would require database connection
    }
}
