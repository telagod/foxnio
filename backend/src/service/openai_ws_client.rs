use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info};

/// WebSocket client for OpenAI Realtime API
pub struct OpenAIWsClient {
    url: String,
    api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsConfig {
    pub url: String,
    pub reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
    pub ping_interval_seconds: u64,
    pub max_message_size: usize,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            url: "wss://api.openai.com/v1/realtime".to_string(),
            reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
            ping_interval_seconds: 30,
            max_message_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "session.update")]
    SessionUpdate { session: SessionConfig },
    #[serde(rename = "session.created")]
    SessionCreated { session: SessionConfig },
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend { audio: String },
    #[serde(rename = "input_audio_buffer.commit")]
    InputAudioBufferCommit,
    #[serde(rename = "input_audio_buffer.clear")]
    InputAudioBufferClear,
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate { item: ConversationItem },
    #[serde(rename = "response.create")]
    ResponseCreate { response: ResponseConfig },
    #[serde(rename = "response.audio.delta")]
    ResponseAudioDelta { delta: String },
    #[serde(rename = "response.audio.done")]
    ResponseAudioDone,
    #[serde(rename = "error")]
    Error { error: ErrorDetail },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub modalities: Vec<String>,
    pub instructions: Option<String>,
    pub voice: Option<String>,
    pub input_audio_format: Option<String>,
    pub output_audio_format: Option<String>,
    pub turn_detection: Option<TurnDetection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnDetection {
    #[serde(rename = "type")]
    pub detection_type: String,
    pub threshold: f32,
    pub prefix_padding_ms: u32,
    pub silence_duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub role: String,
    pub content: Vec<ContentPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
    pub audio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseConfig {
    pub modalities: Vec<String>,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Max reconnect attempts exceeded")]
    MaxReconnectExceeded,
}

impl OpenAIWsClient {
    pub fn new(url: String, api_key: String) -> Self {
        Self { url, api_key }
    }

    /// Connect to WebSocket
    pub async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let url = format!("{}?model=gpt-4o-realtime-preview-2024-12-17", self.url);

        let (ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

        info!("WebSocket connected to {}", url);
        Ok(ws_stream)
    }

    /// Send message
    pub async fn send_message(
        &mut self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        message: &WsMessage,
    ) -> Result<(), WsError> {
        let json = serde_json::to_string(message)?;
        ws.send(Message::Text(json)).await?;
        debug!("Sent WebSocket message: {:?}", message);
        Ok(())
    }

    /// Receive message
    pub async fn receive_message(
        &mut self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<Option<WsMessage>, WsError> {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => {
                let message: WsMessage = serde_json::from_str(&text)?;
                debug!("Received WebSocket message: {:?}", message);
                Ok(Some(message))
            }
            Some(Ok(Message::Binary(data))) => {
                debug!("Received binary message: {} bytes", data.len());
                Ok(None)
            }
            Some(Ok(Message::Ping(_))) => {
                debug!("Received ping");
                Ok(None)
            }
            Some(Ok(Message::Pong(_))) => {
                debug!("Received pong");
                Ok(None)
            }
            Some(Ok(Message::Close(_))) => {
                info!("WebSocket connection closed");
                Ok(None)
            }
            Some(Ok(Message::Frame(_))) => {
                // Frame messages are raw frames, usually handled internally
                Ok(None)
            }
            Some(Err(e)) => {
                error!("WebSocket error: {}", e);
                Err(WsError::from(e))
            }
            None => Ok(None),
        }
    }

    /// Close connection
    pub async fn close(
        &mut self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<(), WsError> {
        ws.close(None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_config_default() {
        let config = WsConfig::default();
        assert_eq!(config.reconnect_attempts, 5);
    }

    #[test]
    fn test_message_serialization() {
        let message = WsMessage::SessionCreated {
            session: SessionConfig {
                model: "gpt-4o-realtime".to_string(),
                modalities: vec!["text".to_string(), "audio".to_string()],
                instructions: None,
                voice: None,
                input_audio_format: None,
                output_audio_format: None,
                turn_detection: None,
            },
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("session.created"));
    }
}
