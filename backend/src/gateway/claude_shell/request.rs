// 请求和响应结构

use serde::{Deserialize, Serialize};

/// 消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest {
    /// 模型名称
    pub model: String,

    /// 消息列表
    pub messages: Vec<Message>,

    /// 最大 token 数
    #[serde(rename = "max_tokens")]
    pub max_tokens: u32,

    /// 是否流式输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// 系统提示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// 温度 (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p 采样
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k 采样
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// 工具定义
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// 元数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 角色 (user/assistant)
    pub role: String,

    /// 内容
    pub content: MessageContent,
}

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 文本内容
    Text(String),

    /// 复合内容
    ContentBlocks(Vec<ContentBlock>),
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

    /// 图像源
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ImageSource>,

    /// 工具使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<ToolUse>,
}

/// 图像源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    /// 类型
    #[serde(rename = "type")]
    pub source_type: String,

    /// 媒体类型
    pub media_type: String,

    /// 数据
    pub data: String,
}

/// 工具使用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUse {
    /// 工具 ID
    pub id: String,

    /// 工具名称
    pub name: String,

    /// 输入
    pub input: serde_json::Value,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// 工具名称
    pub name: String,

    /// 工具描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 输入模式
    pub input_schema: serde_json::Value,
}

/// 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    /// 响应 ID
    pub id: String,

    /// 类型
    #[serde(rename = "type")]
    pub response_type: String,

    /// 角色
    pub role: String,

    /// 内容
    pub content: Vec<ContentBlock>,

    /// 模型
    pub model: String,

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
    pub output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_request() {
        let request = MessageRequest {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
            }],
            max_tokens: 4096,
            stream: Some(true),
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            tools: None,
            metadata: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("claude-3-5-sonnet-20241022"));
    }
}
