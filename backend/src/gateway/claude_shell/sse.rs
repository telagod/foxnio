// SSE (Server-Sent Events) 流式解析

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// SSE 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    /// 事件类型
    #[serde(rename = "type")]
    pub event_type: String,

    /// 索引（用于内容块）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,

    /// 增量内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,

    /// 消息内容（用于 message_start）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<MessageStart>,

    /// 使用情况（用于 message_delta）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// 内容块（用于 content_block_start）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_block: Option<ContentBlock>,
}

/// 增量内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    /// 停止原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// 文本内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// 类型
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub delta_type: Option<String>,
}

/// 消息开始
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStart {
    /// 消息 ID
    pub id: String,

    /// 类型
    #[serde(rename = "type")]
    pub message_type: String,

    /// 角色
    pub role: String,

    /// 模型
    pub model: String,

    /// 内容
    pub content: Vec<serde_json::Value>,

    /// 停止原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,

    /// 使用情况
    pub usage: Usage,
}

/// 使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// 输入 token 数
    pub input_tokens: u32,

    /// 输出 token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
}

/// 内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    /// 类型
    #[serde(rename = "type")]
    pub block_type: String,

    /// 文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// 解析 SSE 行
pub fn parse_sse_line(line: &str) -> Option<SseEvent> {
    // SSE 格式: data: {...}
    if !line.starts_with("data: ") {
        return None;
    }

    let json_str = line.strip_prefix("data: ")?;

    // 跳过空行和结束标记
    if json_str.is_empty() || json_str == "[DONE]" {
        return None;
    }

    // 解析 JSON
    serde_json::from_str(json_str).ok()
}

/// 解析 SSE 事件流
pub fn parse_sse_stream(stream: &str) -> Vec<SseEvent> {
    stream
        .lines()
        .filter_map(|line| parse_sse_line(line))
        .collect()
}

/// 事件类型常量
pub mod event_types {
    pub const MESSAGE_START: &str = "message_start";
    pub const CONTENT_BLOCK_START: &str = "content_block_start";
    pub const CONTENT_BLOCK_DELTA: &str = "content_block_delta";
    pub const CONTENT_BLOCK_STOP: &str = "content_block_stop";
    pub const MESSAGE_DELTA: &str = "message_delta";
    pub const MESSAGE_STOP: &str = "message_stop";
    pub const PING: &str = "ping";
    pub const ERROR: &str = "error";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_line() {
        let line = r#"data: {"type": "message_start", "message": {"id": "msg_xxx", "type": "message", "role": "assistant", "model": "claude-3-5-sonnet-20241022", "content": [], "usage": {"input_tokens": 10}}}"#;

        let event = parse_sse_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, "message_start");
    }

    #[test]
    fn test_parse_sse_line_invalid() {
        let line = "invalid line";
        assert!(parse_sse_line(line).is_none());
    }

    #[test]
    fn test_parse_sse_line_done() {
        let line = "data: [DONE]";
        assert!(parse_sse_line(line).is_none());
    }

    #[test]
    fn test_parse_sse_stream() {
        let stream = r#"data: {"type": "message_start", "message": {"id": "msg_xxx", "type": "message", "role": "assistant", "model": "claude-3-5-sonnet-20241022", "content": [], "usage": {"input_tokens": 10}}}
data: {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}
data: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}
data: [DONE]"#;

        let events = parse_sse_stream(stream);
        assert_eq!(events.len(), 3);
    }
}
