use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Service for managing OpenAI previous response IDs (for context continuity)
pub struct OpenAIPreviousResponseId {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ResponseIdRecord {
    pub id: String,
    pub user_id: i64,
    pub conversation_id: String,
    pub response_id: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ResponseIdError {
    #[error("Response ID not found")]
    NotFound,
    #[error("Response ID expired")]
    Expired,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl OpenAIPreviousResponseId {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Store response ID
    pub async fn store(
        &self,
        user_id: i64,
        conversation_id: String,
        response_id: String,
        model: String,
        ttl_hours: i64,
    ) -> Result<ResponseIdRecord, ResponseIdError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(ttl_hours);

        let record = query_as::<_, ResponseIdRecord>(
            r#"
            INSERT INTO response_ids (
                user_id, conversation_id, response_id, model, created_at, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, user_id, conversation_id, response_id, model, created_at, expires_at
            "#,
        )
        .bind(user_id)
        .bind(&conversation_id)
        .bind(&response_id)
        .bind(&model)
        .bind(now)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    /// Get latest response ID for conversation
    pub async fn get_latest(
        &self,
        user_id: i64,
        conversation_id: &str,
    ) -> Result<Option<ResponseIdRecord>, ResponseIdError> {
        let record = query_as::<_, ResponseIdRecord>(
            r#"
            SELECT id, user_id, conversation_id, response_id, model, created_at, expires_at
            FROM response_ids
            WHERE user_id = $1 AND conversation_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    /// Validate response ID is still valid
    pub async fn validate(&self, response_id: &str) -> Result<ResponseIdRecord, ResponseIdError> {
        let record = query_as::<_, ResponseIdRecord>(
            r#"
            SELECT id, user_id, conversation_id, response_id, model, created_at, expires_at
            FROM response_ids
            WHERE response_id = $1
            "#,
        )
        .bind(response_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ResponseIdError::NotFound)?;

        if record.expires_at < Utc::now() {
            return Err(ResponseIdError::Expired);
        }

        Ok(record)
    }

    /// Delete expired response IDs
    pub async fn cleanup_expired(&self) -> Result<u64, ResponseIdError> {
        let result = query("DELETE FROM response_ids WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_id_creation() {
        // Test would require database connection
    }
}
