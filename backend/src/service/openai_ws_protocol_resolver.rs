use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol resolver for OpenAI WebSocket messages
pub struct OpenAIWsProtocolResolver;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub message_type: String,
    pub payload: Value,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("Invalid message format")]
    InvalidFormat,
    #[error("Unknown message type: {0}")]
    UnknownType(String),
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

impl OpenAIWsProtocolResolver {
    /// Parse raw message
    pub fn parse(raw: &str) -> Result<ProtocolMessage, ResolverError> {
        let value: Value = serde_json::from_str(raw)?;

        let message_type = value
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or(ResolverError::InvalidFormat)?
            .to_string();

        Ok(ProtocolMessage {
            message_type: message_type.clone(),
            payload: value,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Validate message
    pub fn validate(message: &ProtocolMessage) -> Result<(), ResolverError> {
        match message.message_type.as_str() {
            "session.update"
            | "session.created"
            | "input_audio_buffer.append"
            | "input_audio_buffer.commit"
            | "input_audio_buffer.clear"
            | "conversation.item.create"
            | "response.create"
            | "response.audio.delta"
            | "response.audio.done"
            | "error" => Ok(()),
            _ => Err(ResolverError::UnknownType(message.message_type.clone())),
        }
    }

    /// Extract session ID from message
    pub fn extract_session_id(message: &ProtocolMessage) -> Option<String> {
        message
            .payload
            .get("session_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
    }

    /// Build error response
    pub fn build_error(code: &str, message: &str) -> Value {
        serde_json::json!({
            "type": "error",
            "error": {
                "code": code,
                "message": message
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message() {
        let raw = r#"{"type": "session.created", "session_id": "test"}"#;
        let message = OpenAIWsProtocolResolver::parse(raw).unwrap();
        assert_eq!(message.message_type, "session.created");
    }
}
