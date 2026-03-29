use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Sticky session compatibility layer for OpenAI requests
pub struct OpenAIStickyCompat {
    sessions: Arc<RwLock<HashMap<String, StickySession>>>,
    config: StickyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickySession {
    pub session_id: String,
    pub account_id: i64,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub request_count: u64,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickyConfig {
    pub enable_sticky_session: bool,
    pub session_ttl_seconds: u64,
    pub max_sessions_per_user: u32,
    pub cleanup_interval_seconds: u64,
}

impl Default for StickyConfig {
    fn default() -> Self {
        Self {
            enable_sticky_session: true,
            session_ttl_seconds: 300,
            max_sessions_per_user: 10,
            cleanup_interval_seconds: 60,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StickyError {
    #[error("Session not found")]
    SessionNotFound,
    #[error("Session expired")]
    SessionExpired,
    #[error("Max sessions exceeded")]
    MaxSessionsExceeded,
}

impl OpenAIStickyCompat {
    pub fn new(config: StickyConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create or get sticky session
    pub async fn get_or_create(
        &self,
        user_id: i64,
        account_id: i64,
        model: &str,
    ) -> Result<String, StickyError> {
        if !self.config.enable_sticky_session {
            return Ok(uuid::Uuid::new_v4().to_string());
        }

        let session_id = format!("sticky:{}:{}:{}", user_id, account_id, model);
        let mut sessions = self.sessions.write().await;

        // Check if session exists and is valid
        if let Some(session) = sessions.get(&session_id) {
            if self.is_session_valid(session) {
                // Update last used time
                let session = sessions.get_mut(&session_id).unwrap();
                session.last_used_at = Utc::now();
                session.request_count += 1;
                return Ok(session_id);
            }
        }

        // Create new session
        let session = StickySession {
            session_id: session_id.clone(),
            account_id,
            model: model.to_string(),
            created_at: Utc::now(),
            last_used_at: Utc::now(),
            request_count: 1,
            ttl_seconds: self.config.session_ttl_seconds,
        };

        sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Get session by ID
    pub async fn get(&self, session_id: &str) -> Option<StickySession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Check if session is valid
    fn is_session_valid(&self, session: &StickySession) -> bool {
        let now = Utc::now();
        let elapsed = (now - session.last_used_at).num_seconds();
        elapsed < self.config.session_ttl_seconds as i64
    }

    /// Delete session
    pub async fn delete(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Cleanup expired sessions
    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, session| self.is_session_valid(session));
    }

    /// Get session stats
    pub async fn get_stats(&self) -> StickyStats {
        let sessions = self.sessions.read().await;
        StickyStats {
            total_sessions: sessions.len(),
            active_sessions: sessions
                .values()
                .filter(|s| self.is_session_valid(s))
                .count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickyStats {
    pub total_sessions: usize,
    pub active_sessions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sticky_session() {
        let compat = OpenAIStickyCompat::new(StickyConfig::default());

        let session_id = compat.get_or_create(1, 100, "gpt-4").await.unwrap();
        assert!(session_id.starts_with("sticky:"));

        let session = compat.get(&session_id).await;
        assert!(session.is_some());
    }

    #[test]
    fn test_sticky_config_default() {
        let config = StickyConfig::default();
        assert!(config.enable_sticky_session);
        assert_eq!(config.session_ttl_seconds, 300);
    }
}
