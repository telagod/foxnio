//! 流式响应完整实现

use anyhow::Result;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use serde_json::Value;

/// SSE 事件
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
    pub retry: Option<u64>,
}

impl SseEvent {
    /// 解析 SSE 事件
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() || text == "[DONE]" {
            return None;
        }
        
        let mut event = Self {
            event_type: None,
            data: String::new(),
            id: None,
            retry: None,
        };
        
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            
            if let Some(event_type) = line.strip_prefix("event: ") {
                event.event_type = Some(event_type.to_string());
            } else if let Some(data) = line.strip_prefix("data: ") {
                if !event.data.is_empty() {
                    event.data.push('\n');
                }
                event.data.push_str(data);
            } else if let Some(id) = line.strip_prefix("id: ") {
                event.id = Some(id.to_string());
            } else if let Some(retry) = line.strip_prefix("retry: ") {
                event.retry = retry.parse().ok();
            }
        }
        
        if event.data.is_empty() {
            None
        } else {
            Some(event)
        }
    }
    
    /// 转换为 SSE 文本
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        
        if let Some(ref event_type) = self.event_type {
            result.push_str(&format!("event: {}\n", event_type));
        }
        
        if let Some(ref id) = self.id {
            result.push_str(&format!("id: {}\n", id));
        }
        
        if let Some(retry) = self.retry {
            result.push_str(&format!("retry: {}\n", retry));
        }
        
        result.push_str(&format!("data: {}\n\n", self.data));
        
        result
    }
}

/// OpenAI 流式响应解析器
pub struct OpenAIStreamParser;

impl OpenAIStreamParser {
    /// 解析 OpenAI 流式 chunk
    pub fn parse_chunk(data: &str) -> Option<OpenAIStreamChunk> {
        if data == "[DONE]" {
            return None;
        }
        
        serde_json::from_str(data).ok()
    }
    
    /// 提取内容增量
    pub fn extract_delta(chunk: &OpenAIStreamChunk) -> Option<String> {
        chunk.choices.first()
            .and_then(|c| c.delta.content.clone())
    }
    
    /// 检查是否完成
    pub fn is_finished(chunk: &OpenAIStreamChunk) -> bool {
        chunk.choices.first()
            .map(|c| c.finish_reason.is_some())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OpenAIStreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamChoice {
    pub index: i32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
}

/// Anthropic 流式响应解析器
pub struct AnthropicStreamParser;

impl AnthropicStreamParser {
    /// 解析 Anthropic 流式事件
    pub fn parse_event(data: &str) -> Option<AnthropicStreamEvent> {
        serde_json::from_str(data).ok()
    }
    
    /// 提取内容增量
    pub fn extract_text(event: &AnthropicStreamEvent) -> Option<String> {
        match event {
            AnthropicStreamEvent::ContentBlockDelta { delta, .. } => delta.text.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessage },
    
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: i32, content_block: ContentBlock },
    
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: i32, delta: DeltaContent },
    
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: i32 },
    
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDelta, usage: UsageDelta },
    
    #[serde(rename = "message_stop")]
    MessageStop,
    
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AnthropicMessage {
    pub id: String,
    pub model: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DeltaContent {
    #[serde(rename = "type")]
    pub delta_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UsageDelta {
    pub output_tokens: i64,
}

/// Token 使用量追踪器
pub struct UsageTracker {
    input_tokens: i64,
    output_tokens: i64,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
        }
    }
    
    /// 从 OpenAI 流中提取使用量
    pub fn track_openai_chunk(&mut self, chunk: &OpenAIStreamChunk) {
        // OpenAI 流式响应通常在最后一个 chunk 包含 usage
        // 这里简化处理，实际需要从 chunk 中提取
    }
    
    /// 从 Anthropic 流中提取使用量
    pub fn track_anthropic_event(&mut self, event: &AnthropicStreamEvent) {
        if let AnthropicStreamEvent::MessageDelta { usage, .. } = event {
            self.output_tokens += usage.output_tokens;
        }
    }
    
    pub fn get_usage(&self) -> (i64, i64) {
        (self.input_tokens, self.output_tokens)
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// 流式响应包装器
pub struct StreamingBody {
    receiver: mpsc::Receiver<Result<Bytes>>,
}

impl StreamingBody {
    pub fn new(receiver: mpsc::Receiver<Result<Bytes>>) -> Self {
        Self { receiver }
    }
}

impl Stream for StreamingBody {
    type Item = Result<Bytes>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sse_event_parse() {
        let text = "event: message\ndata: {\"text\":\"hello\"}\n\n";
        let event = SseEvent::parse(text);
        
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_type, Some("message".to_string()));
        assert_eq!(event.data, r#"{"text":"hello"}"#);
    }
    
    #[test]
    fn test_sse_event_parse_multiline() {
        let text = "data: line1\ndata: line2\n\n";
        let event = SseEvent::parse(text);
        
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.data, "line1\nline2");
    }
    
    #[test]
    fn test_sse_event_done() {
        let text = "[DONE]";
        let event = SseEvent::parse(text);
        
        assert!(event.is_none());
    }
    
    #[test]
    fn test_openai_chunk_parse() {
        let data = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk = OpenAIStreamParser::parse_chunk(data);
        
        assert!(chunk.is_some());
        let chunk = chunk.unwrap();
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(OpenAIStreamParser::extract_delta(&chunk), Some("Hello".to_string()));
    }
    
    #[test]
    fn test_sse_to_string() {
        let event = SseEvent {
            event_type: Some("message".to_string()),
            data: r#"{"text":"hello"}"#.to_string(),
            id: None,
            retry: None,
        };
        
        let text = event.to_string();
        assert!(text.contains("event: message"));
        assert!(text.contains("data: {\"text\":\"hello\"}"));
    }
}
