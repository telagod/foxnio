//! Gateway Responses API 转发服务
//!
//! 处理 OpenAI Responses API 格式的请求和响应

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Responses API 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<ResponsesMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponsesTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(flatten)]
    pub other: HashMap<String, JsonValue>,
}

/// Responses 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesMessage {
    pub role: String,
    pub content: Vec<ResponsesContent>,
}

/// Responses 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponsesContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "reasoning")]
    Reasoning { summary: Vec<ReasoningSummary> },
}

/// 图片源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// 推理摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningSummary {
    #[serde(rename = "type")]
    pub summary_type: String,
    pub text: String,
}

/// Responses 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: JsonValue,
}

/// 推理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    #[serde(rename = "type")]
    pub config_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

/// Responses API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub object: String,
    pub created_at: i64,
    pub status: String,
    pub output: Vec<ResponsesContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

/// 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Responses 流式事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ResponsesResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<ResponsesDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<ResponsesContent>,
}

/// Responses 增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// 转发服务
pub struct GatewayForwardResponses;

impl GatewayForwardResponses {
    /// 将 Responses 请求转换为 Anthropic 请求
    pub fn to_anthropic(
        req: &ResponsesRequest,
    ) -> Result<crate::gateway::responses::AnthropicRequest> {
        // 分离系统消息和对话消息
        let mut system_prompt = req.instructions.clone().unwrap_or_default();
        let mut messages = Vec::new();

        for msg in &req.input {
            if msg.role == "system" {
                if let Some(ResponsesContent::Text { text }) = msg.content.first() {
                    if !system_prompt.is_empty() {
                        system_prompt.push('\n');
                    }
                    system_prompt.push_str(text);
                }
                continue;
            }

            // 转换内容
            let anthropic_content: Vec<crate::gateway::responses::AnthropicContentBlock> = msg
                .content
                .iter()
                .filter_map(Self::convert_content)
                .collect();

            if !anthropic_content.is_empty() {
                messages.push(crate::gateway::responses::AnthropicMessage {
                    role: msg.role.clone(),
                    content: serde_json::to_value(&anthropic_content)
                        .unwrap_or(serde_json::json!([])),
                });
            }
        }

        // 转换工具定义
        let tools = req.tools.as_ref().map(|t| {
            t.iter()
                .map(|tool| crate::gateway::responses::AnthropicTool {
                    tool_type: Some("function".to_string()),
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    input_schema: tool.input_schema.clone(),
                })
                .collect()
        });

        // 将消息内容转换为 JsonValue
        let messages_json: Vec<crate::gateway::responses::AnthropicMessage> = messages
            .into_iter()
            .map(|m| crate::gateway::responses::AnthropicMessage {
                role: m.role,
                content: serde_json::to_value(m.content).unwrap_or(serde_json::json!([])),
            })
            .collect();

        Ok(crate::gateway::responses::AnthropicRequest {
            model: req.model.clone(),
            messages: messages_json,
            system: if system_prompt.is_empty() {
                None
            } else {
                Some(serde_json::json!(system_prompt))
            },
            max_tokens: req.max_output_tokens.unwrap_or(4096) as i32,
            temperature: req.temperature.map(|t| t as f64),
            top_p: req.top_p.map(|p| p as f64),
            tools,
            stream: req.stream.unwrap_or(false),
            stop_sequences: None,
            thinking: None,
            tool_choice: None,
        })
    }

    /// 转换内容块
    fn convert_content(
        content: &ResponsesContent,
    ) -> Option<crate::gateway::responses::AnthropicContentBlock> {
        match content {
            ResponsesContent::Text { text } => {
                Some(crate::gateway::responses::AnthropicContentBlock {
                    block_type: "text".to_string(),
                    text: Some(text.clone()),
                    thinking: None,
                    source: None,
                    id: None,
                    name: None,
                    input: None,
                    tool_use_id: None,
                    is_error: None,
                })
            }
            ResponsesContent::Image { source } => {
                Some(crate::gateway::responses::AnthropicContentBlock {
                    block_type: "image".to_string(),
                    text: None,
                    thinking: None,
                    source: Some(crate::gateway::responses::AnthropicImageSource {
                        source_type: source.source_type.clone(),
                        media_type: source.media_type.clone().unwrap_or_default(),
                        data: source.data.clone().unwrap_or_default(),
                    }),
                    id: None,
                    name: None,
                    input: None,
                    tool_use_id: None,
                    is_error: None,
                })
            }
            ResponsesContent::ToolUse { id, name, input } => {
                Some(crate::gateway::responses::AnthropicContentBlock {
                    block_type: "tool_use".to_string(),
                    text: None,
                    thinking: None,
                    source: None,
                    id: Some(id.clone()),
                    name: Some(name.clone()),
                    input: Some(input.clone()),
                    tool_use_id: None,
                    is_error: None,
                })
            }
            ResponsesContent::ToolResult {
                tool_use_id,
                content,
            } => Some(crate::gateway::responses::AnthropicContentBlock {
                block_type: "tool_result".to_string(),
                text: Some(content.clone()),
                thinking: None,
                source: None,
                id: None,
                name: None,
                input: None,
                tool_use_id: Some(tool_use_id.clone()),
                is_error: None,
            }),
            ResponsesContent::Reasoning { summary } => {
                Some(crate::gateway::responses::AnthropicContentBlock {
                    block_type: "text".to_string(),
                    text: Some(
                        summary
                            .iter()
                            .map(|s| s.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    thinking: None,
                    source: None,
                    id: None,
                    name: None,
                    input: None,
                    tool_use_id: None,
                    is_error: None,
                })
            }
        }
    }

    /// 将 Anthropic 响应转换为 Responses 响应
    pub fn from_anthropic(
        resp: &crate::gateway::responses::AnthropicResponse,
        model: &str,
    ) -> Result<ResponsesResponse> {
        let mut output = Vec::new();

        // 转换内容
        for content in &resp.content {
            match content.block_type.as_str() {
                "text" => {
                    if let Some(text) = &content.text {
                        output.push(ResponsesContent::Text { text: text.clone() });
                    }
                }
                "tool_use" => {
                    if let (Some(id), Some(name), Some(input)) =
                        (&content.id, &content.name, &content.input)
                    {
                        output.push(ResponsesContent::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(ResponsesResponse {
            id: Some(format!("resp-{}", uuid::Uuid::new_v4())),
            object: "response".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            status: resp.stop_reason.clone(),
            output,
            usage: Some(Usage {
                prompt_tokens: resp.usage.input_tokens as u32,
                completion_tokens: resp.usage.output_tokens as u32,
                total_tokens: (resp.usage.input_tokens + resp.usage.output_tokens) as u32,
            }),
            model: Some(model.to_string()),
            previous_response_id: None,
        })
    }

    /// 处理 Responses 请求
    pub async fn forward(_req: ResponsesRequest, _account_id: i64) -> Result<ResponsesResponse> {
        // 实际转发逻辑已在 responses_handler.rs 实现
        Err(anyhow::anyhow!("Not implemented yet"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responses_request_deserialize() {
        let json = r#"{
            "model": "claude-3-opus",
            "input": [
                {"role": "user", "content": [{"type": "text", "text": "Hello"}]}
            ],
            "max_output_tokens": 1024
        }"#;

        let req: ResponsesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "claude-3-opus");
        assert_eq!(req.input.len(), 1);
    }

    #[test]
    fn test_responses_content_variants() {
        let text_content = ResponsesContent::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&text_content).unwrap();
        assert!(json.contains(r#""type":"text"#));
    }
}
