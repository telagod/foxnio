use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, FromRow, PgPool};

/// Video generation service for Sora API
pub struct SoraGenerationService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub user_id: i64,
    pub prompt: String,
    pub duration_seconds: Option<u32>,
    pub resolution: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Generation {
    pub id: String,
    pub user_id: i64,
    pub status: String,
    pub prompt: String,
    pub video_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub duration_seconds: i32,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GenerationStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error("Generation not found")]
    NotFound,
    #[error("Generation failed: {0}")]
    Failed(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl SoraGenerationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new generation
    pub async fn create(&self, req: GenerationRequest) -> Result<Generation, GenerationError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let generation = query_as::<_, Generation>(
            r#"
            INSERT INTO sora_generations (id, user_id, status, prompt, duration_seconds, created_at)
            VALUES ($1, $2, 'pending', $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(req.user_id)
        .bind(&req.prompt)
        .bind(req.duration_seconds.unwrap_or(10) as i32)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(generation)
    }

    /// Get generation by ID
    pub async fn get(&self, id: &str) -> Result<Generation, GenerationError> {
        let generation = query_as::<_, Generation>("SELECT * FROM sora_generations WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(GenerationError::NotFound)?;

        Ok(generation)
    }

    /// Update generation status
    pub async fn update_status(
        &self,
        id: &str,
        status: GenerationStatus,
        video_url: Option<String>,
    ) -> Result<(), GenerationError> {
        let status_str = serde_json::to_string(&status).unwrap();

        query(r#"
            UPDATE sora_generations
            SET status = $1, video_url = $2, completed_at = CASE WHEN $1 IN ('completed', 'failed') THEN NOW() ELSE NULL END
            WHERE id = $3
            "#)
            .bind(&status_str)
            .bind(&video_url)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Cancel generation
    pub async fn cancel(&self, id: &str) -> Result<(), GenerationError> {
        self.update_status(id, GenerationStatus::Cancelled, None)
            .await
    }

    /// Get generations for user
    pub async fn list_for_user(
        &self,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<Generation>, GenerationError> {
        let generations = query_as::<_, Generation>(
            r#"
            SELECT * FROM sora_generations
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(generations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_creation() {
        // Test would require database connection
    }
}
