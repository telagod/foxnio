use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::debug;

/// WebSocket forwarder for OpenAI Realtime API
pub struct OpenAIWsForwarder {
    config: ForwarderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwarderConfig {
    pub buffer_size: usize,
    pub max_message_size: usize,
    pub enable_compression: bool,
    pub timeout_seconds: u64,
}

impl Default for ForwarderConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            max_message_size: 10 * 1024 * 1024,
            enable_compression: true,
            timeout_seconds: 300,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForwardContext {
    pub session_id: String,
    pub user_id: i64,
    pub account_id: i64,
    pub model: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ForwarderError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Message too large")]
    MessageTooLarge,
    #[error("Timeout")]
    Timeout,
    #[error("Channel closed")]
    ChannelClosed,
}

impl OpenAIWsForwarder {
    pub fn new(config: ForwarderConfig) -> Self {
        Self { config }
    }

    /// Forward incoming stream to upstream
    pub async fn forward_upstream(
        &self,
        ctx: ForwardContext,
        mut inbound: impl Stream<Item = Result<Bytes, axum::Error>> + Unpin,
        outbound: mpsc::Sender<String>,
    ) -> Result<(), ForwarderError> {
        let mut buffer = Vec::new();

        while let Some(msg) = inbound.next().await {
            let data = msg.map_err(|e| ForwarderError::ConnectionError(e.to_string()))?;

            // Check message size
            if data.len() > self.config.max_message_size {
                return Err(ForwarderError::MessageTooLarge);
            }

            buffer.extend_from_slice(&data);

            // Try to parse complete message
            if let Ok(text) = std::str::from_utf8(&buffer) {
                debug!(
                    "Forwarding message for session {}: {} bytes",
                    ctx.session_id,
                    buffer.len()
                );
                outbound
                    .send(text.to_string())
                    .await
                    .map_err(|_| ForwarderError::ChannelClosed)?;
                buffer.clear();
            }
        }

        Ok(())
    }

    /// Forward downstream messages
    pub async fn forward_downstream(
        &self,
        ctx: ForwardContext,
        mut inbound: mpsc::Receiver<String>,
    ) -> Result<(), ForwarderError> {
        while let Some(msg) = inbound.recv().await {
            debug!(
                "Forwarding downstream for session {}: {} bytes",
                ctx.session_id,
                msg.len()
            );
            // In real implementation, send to client
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forwarder_config_default() {
        let config = ForwarderConfig::default();
        assert_eq!(config.buffer_size, 1000);
    }
}
