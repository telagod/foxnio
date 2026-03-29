use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// State store for OpenAI WebSocket sessions
pub struct OpenAIWsStateStore {
    states: Arc<RwLock<HashMap<String, SessionState>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub conversation_items: Vec<ConversationItem>,
    pub audio_buffer: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationItem {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Session not found")]
    SessionNotFound,
    #[error("State corrupted")]
    StateCorrupted,
}

impl OpenAIWsStateStore {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create new session state
    pub async fn create(&self, session_id: String) -> Result<(), StateError> {
        let now = chrono::Utc::now().timestamp();
        let state = SessionState {
            session_id: session_id.clone(),
            conversation_items: Vec::new(),
            audio_buffer: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        };

        let mut states = self.states.write().await;
        states.insert(session_id, state);
        Ok(())
    }

    /// Get session state
    pub async fn get(&self, session_id: &str) -> Option<SessionState> {
        let states = self.states.read().await;
        states.get(session_id).cloned()
    }

    /// Add conversation item
    pub async fn add_item(
        &self,
        session_id: &str,
        item: ConversationItem,
    ) -> Result<(), StateError> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(session_id)
            .ok_or(StateError::SessionNotFound)?;
        state.conversation_items.push(item);
        state.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Append audio buffer
    pub async fn append_audio(&self, session_id: &str, audio: &[u8]) -> Result<(), StateError> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(session_id)
            .ok_or(StateError::SessionNotFound)?;
        state.audio_buffer.extend_from_slice(audio);
        state.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Clear audio buffer
    pub async fn clear_audio(&self, session_id: &str) -> Result<(), StateError> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(session_id)
            .ok_or(StateError::SessionNotFound)?;
        state.audio_buffer.clear();
        state.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Delete session
    pub async fn delete(&self, session_id: &str) {
        let mut states = self.states.write().await;
        states.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_operations() {
        let store = OpenAIWsStateStore::new();
        let session_id = "test_session".to_string();

        store.create(session_id.clone()).await.unwrap();

        let state = store.get(&session_id).await;
        assert!(state.is_some());

        store.delete(&session_id).await;
        let state = store.get(&session_id).await;
        assert!(state.is_none());
    }
}
