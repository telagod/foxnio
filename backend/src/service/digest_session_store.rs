//! Digest session store

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Digest session
#[derive(Debug, Clone)]
pub struct DigestSession {
    pub id: String,
    pub user_id: i64,
    pub data: Vec<u8>,
    pub created_at: Instant,
    pub ttl: Duration,
}

impl DigestSession {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Digest session store
pub struct DigestSessionStore {
    sessions: Arc<RwLock<HashMap<String, DigestSession>>>,
    default_ttl: Duration,
}

impl Default for DigestSessionStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

impl DigestSessionStore {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    pub async fn store(&self, id: String, user_id: i64, data: Vec<u8>) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            id.clone(),
            DigestSession {
                id,
                user_id,
                data,
                created_at: Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    pub async fn retrieve(&self, id: &str) -> Option<DigestSession> {
        let sessions = self.sessions.read().await;
        sessions.get(id).filter(|s| !s.is_expired()).cloned()
    }

    pub async fn remove(&self, id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id);
    }

    pub async fn clear_expired(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, s| !s.is_expired());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_store() {
        let store = DigestSessionStore::new(Duration::from_secs(60));

        store
            .store("session-1".to_string(), 123, b"data".to_vec())
            .await;
        let session = store.retrieve("session-1").await.unwrap();

        assert_eq!(session.user_id, 123);
        assert_eq!(session.data, b"data".to_vec());
    }
}
