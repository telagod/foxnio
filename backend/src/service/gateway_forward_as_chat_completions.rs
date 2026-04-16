//! Gateway Chat Completions 转发服务
//!
//! 将 OpenAI Chat Completions API 请求转换为 Anthropic Messages 格式，
//! 并将响应转换回 Chat Completions 格式。

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Chat Completions 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(flatten)]
    pub other: HashMap<String, JsonValue>,
}

/// Chat 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 消息内容（字符串或多部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<MessagePart>),
}

/// 消息部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 流式选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

/// 工具函数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: JsonValue,
}

/// 工具选择
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    None(String),
    Auto(String),
    Required(String),
    Function {
        r#type: String,
        function: FunctionName,
    },
}

/// 函数名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionName {
    pub name: String,
}

/// 响应格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Chat Completions 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Chat 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: Option<ChatMessage>,
    pub delta: Option<ChatDelta>,
    pub finish_reason: Option<String>,
}

/// Chat 增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// 工具调用增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// 函数调用增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// 使用量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Chat Completions 流式事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
}

/// 转发服务
pub struct GatewayForwardChatCompletions;

impl GatewayForwardChatCompletions {
    /// 将 Chat Completions 请求转换为 Responses 请求
    pub fn to_responses(
        req: &ChatCompletionsRequest,
    ) -> Result<super::gateway_forward_as_responses::ResponsesRequest> {
        // 转换消息格式
        let mut responses_messages = Vec::new();

        for msg in &req.messages {
            let role = match msg.role.as_str() {
                "system" => "system",
                "user" => "user",
                "assistant" => "assistant",
                "tool" => "user", // 工具结果作为用户消息
                _ => &msg.role,
            };

            let content = match &msg.content {
                Some(MessageContent::Text(text)) => {
                    if msg.role == "tool" {
                        vec![
                            super::gateway_forward_as_responses::ResponsesContent::ToolResult {
                                tool_use_id: msg.tool_call_id.clone().unwrap_or_default(),
                                content: text.clone(),
                            },
                        ]
                    } else {
                        vec![
                            super::gateway_forward_as_responses::ResponsesContent::Text {
                                text: text.clone(),
                            },
                        ]
                    }
                }
                Some(MessageContent::Parts(parts)) => parts
                    .iter()
                    .map(|p| match p.part_type.as_str() {
                        "text" => super::gateway_forward_as_responses::ResponsesContent::Text {
                            text: p.text.clone().unwrap_or_default(),
                        },
                        "image_url" => {
                            super::gateway_forward_as_responses::ResponsesContent::Image {
                                source: super::gateway_forward_as_responses::ImageSource {
                                    source_type: "url".to_string(),
                                    url: Some(p.image_url.clone().unwrap().url),
                                    media_type: None,
                                    data: None,
                                },
                            }
                        }
                        _ => super::gateway_forward_as_responses::ResponsesContent::Text {
                            text: p.text.clone().unwrap_or_default(),
                        },
                    })
                    .collect(),
                None => vec![],
            };

            // 处理工具调用
            if let Some(tool_calls) = &msg.tool_calls {
                for tc in tool_calls {
                    responses_messages.push(
                        super::gateway_forward_as_responses::ResponsesMessage {
                            role: "assistant".to_string(),
                            content: vec![
                                super::gateway_forward_as_responses::ResponsesContent::ToolUse {
                                    id: tc.id.clone(),
                                    name: tc.function.name.clone(),
                                    input: serde_json::from_str(&tc.function.arguments)
                                        .unwrap_or(JsonValue::Null),
                                },
                            ],
                        },
                    );
                }
            }

            if !content.is_empty() {
                responses_messages.push(super::gateway_forward_as_responses::ResponsesMessage {
                    role: role.to_string(),
                    content,
                });
            }
        }

        // 转换工具定义
        let tools = req.tools.as_ref().map(|t| {
            t.iter()
                .map(|tool| super::gateway_forward_as_responses::ResponsesTool {
                    name: tool.function.name.clone(),
                    description: tool.function.description.clone(),
                    input_schema: tool.function.parameters.clone(),
                })
                .collect()
        });

        Ok(super::gateway_forward_as_responses::ResponsesRequest {
            model: req.model.clone(),
            input: responses_messages,
            instructions: None,
            max_output_tokens: req.max_tokens,
            temperature: req.temperature,
            top_p: req.top_p,
            tools,
            stream: req.stream,
            include_usage: req.stream_options.as_ref().and_then(|o| o.include_usage),
            previous_response_id: None,
            reasoning: None,
            other: HashMap::new(),
        })
    }

    /// 将 Responses 响应转换为 Chat Completions 响应
    pub fn from_responses(
        resp: &super::gateway_forward_as_responses::ResponsesResponse,
        model: &str,
    ) -> Result<ChatCompletionsResponse> {
        let mut choices = Vec::new();

        // 转换输出内容
        let mut message_content = String::new();
        let mut tool_calls = Vec::new();

        for content in &resp.output {
            match content {
                super::gateway_forward_as_responses::ResponsesContent::Text { text, .. } => {
                    message_content.push_str(text);
                }
                super::gateway_forward_as_responses::ResponsesContent::ToolUse {
                    id,
                    name,
                    input,
                    ..
                } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: name.clone(),
                            arguments: serde_json::to_string(input).unwrap_or_default(),
                        },
                    });
                }
                _ => {}
            }
        }

        let message = ChatMessage {
            role: "assistant".to_string(),
            content: Some(MessageContent::Text(message_content)),
            name: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tool_call_id: None,
        };

        choices.push(ChatChoice {
            index: 0,
            message: Some(message),
            delta: None,
            finish_reason: Some(resp.status.clone()),
        });

        Ok(ChatCompletionsResponse {
            id: resp
                .id
                .clone()
                .unwrap_or_else(|| format!("chatcmpl-{}", uuid::Uuid::new_v4())),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices,
            usage: resp.usage.clone().map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            system_fingerprint: None,
        })
    }

    /// 处理 Chat Completions 请求
    pub async fn forward(
        _req: ChatCompletionsRequest,
        _account_id: i64,
    ) -> Result<ChatCompletionsResponse> {
        // 实际转发逻辑已在 chat_completions_forwarder.rs 实现
        // 1. 获取账号信息
        // 2. 转换为 Responses 格式
        // 3. 调用网关转发
        // 4. 转换响应

        Err(anyhow!("Not implemented yet"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_completions_request_deserialize() {
        let json = r#"{
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "temperature": 0.7
        }"#;

        let req: ChatCompletionsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_message_content_parts() {
        let json = r#"{
            "model": "gpt-4-vision",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "What is this?"},
                        {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                    ]
                }
            ]
        }"#;

        let req: ChatCompletionsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req.messages[0].content,
            Some(MessageContent::Parts(_))
        ));
    }
}
