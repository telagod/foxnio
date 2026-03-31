//! Wire protocol support service

use serde::{Deserialize, Serialize};

/// Wire protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessageType {
    /// Handshake message
    Handshake,
    /// Data message
    Data,
    /// Control message
    Control,
    /// Heartbeat message
    Heartbeat,
}

/// Wire protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMessage {
    /// Message type
    pub msg_type: WireMessageType,
    /// Message ID
    pub msg_id: u64,
    /// Message payload
    pub payload: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
}

/// Wire protocol service
pub struct WireService {
    /// Current message ID counter
    msg_counter: std::sync::atomic::AtomicU64,
}

impl Default for WireService {
    fn default() -> Self {
        Self::new()
    }
}

impl WireService {
    /// Create a new wire service
    pub fn new() -> Self {
        Self {
            msg_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create a new message
    pub fn create_message(&self, msg_type: WireMessageType, payload: Vec<u8>) -> WireMessage {
        let msg_id = self
            .msg_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        WireMessage {
            msg_type,
            msg_id,
            payload,
            timestamp,
        }
    }

    /// Serialize message to bytes
    pub fn serialize_message(&self, msg: &WireMessage) -> Result<Vec<u8>, String> {
        bincode::serialize(msg).map_err(|e| format!("Failed to serialize: {e}"))
    }

    /// Deserialize message from bytes
    pub fn deserialize_message(&self, data: &[u8]) -> Result<WireMessage, String> {
        bincode::deserialize(data).map_err(|e| format!("Failed to deserialize: {e}"))
    }

    /// Create handshake message
    pub fn create_handshake(&self, version: &str) -> WireMessage {
        let payload = version.as_bytes().to_vec();
        self.create_message(WireMessageType::Handshake, payload)
    }

    /// Create heartbeat message
    pub fn create_heartbeat(&self) -> WireMessage {
        self.create_message(WireMessageType::Heartbeat, vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_service() {
        let service = WireService::new();
        let msg = service.create_message(WireMessageType::Data, vec![1, 2, 3]);
        assert_eq!(msg.msg_id, 1);
        assert_eq!(msg.payload, vec![1, 2, 3]);
    }

    #[test]
    fn test_serialize_deserialize() {
        let service = WireService::new();
        let msg = service.create_message(WireMessageType::Data, vec![1, 2, 3]);
        let serialized = service.serialize_message(&msg).unwrap();
        let deserialized = service.deserialize_message(&serialized).unwrap();
        assert_eq!(msg.msg_id, deserialized.msg_id);
    }
}
