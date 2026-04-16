//! Responses API 格式转换器
//!
//! 实现 Responses ↔ Anthropic 双向格式转换

#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value as JsonValue};

use super::responses::*;

// ---------------------------------------------------------------------------
// Responses → Anthropic 转换
// ---------------------------------------------------------------------------

/// 将 Responses API 请求转换为 Anthropic Messages 请求
pub fn responses_to_anthropic(req: &ResponsesRequest) -> Result<AnthropicRequest> {
    let mut messages = Vec::new();
    let mut system = None;

    // 解析 input 字段
    if let JsonValue::String(text) = &req.input {
        // 简单字符串输入，作为单条用户消息
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: json!(text),
        });
    } else if let JsonValue::Array(items) = &req.input {
        // 输入项数组
        for item_value in items {
            if let Ok(item) = serde_json::from_value::<ResponsesInputItem>(item_value.clone()) {
                if let Some(role) = &item.role {
                    // 角色消息
                    let content = item.content.clone().unwrap_or(json!(""));

                    if role == "system" {
                        // 系统消息
                        if let JsonValue::String(sys_text) = content {
                            system = Some(json!(sys_text));
                        } else {
                            system = Some(content);
                        }
                    } else {
                        // 用户/助手消息
                        messages.push(AnthropicMessage {
                            role: role.clone(),
                            content,
                        });
                    }
                } else if let Some(item_type) = &item.item_type {
                    // 类型化输入项
                    match item_type.as_str() {
                        "function_call" => {
                            // 转换为助手工具调用消息
                            let tool_call_id = item.call_id.clone().unwrap_or_default();
                            let tool_name = item.name.clone().unwrap_or_default();
                            let args = item.arguments.clone().unwrap_or_default();

                            messages.push(AnthropicMessage {
                                role: "assistant".to_string(),
                                content: json!([{
                                    "type": "tool_use",
                                    "id": tool_call_id,
                                    "name": tool_name,
                                    "input": serde_json::from_str::<JsonValue>(&args).unwrap_or(json!({}))
                                }]),
                            });
                        }
                        "function_call_output" => {
                            // 转换为工具结果消息
                            let call_id = item.call_id.clone().unwrap_or_default();
                            let output = item.output.clone().unwrap_or_default();

                            messages.push(AnthropicMessage {
                                role: "user".to_string(),
                                content: json!([{
                                    "type": "tool_result",
                                    "tool_use_id": call_id,
                                    "content": output
                                }]),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 如果没有消息，添加一个空的用户消息
    if messages.is_empty() {
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: json!(""),
        });
    }

    // 转换工具
    let tools = req.tools.as_ref().map(|tools| {
        tools
            .iter()
            .filter_map(|tool| {
                if tool.tool_type == "function" {
                    Some(AnthropicTool {
                        tool_type: Some(tool.tool_type.clone()),
                        name: tool.name.clone().unwrap_or_default(),
                        description: tool.description.clone(),
                        input_schema: tool.parameters.clone().unwrap_or(json!({})),
                    })
                } else {
                    None
                }
            })
            .collect()
    });

    // 处理 reasoning 配置
    let thinking = req.reasoning.as_ref().map(|r| {
        let budget_tokens = match r.effort.as_str() {
            "high" => Some(10000),
            "medium" => Some(5000),
            "low" => Some(1000),
            _ => None,
        };
        AnthropicThinking {
            thinking_type: "enabled".to_string(),
            budget_tokens,
        }
    });

    Ok(AnthropicRequest {
        model: req.model.clone(),
        max_tokens: req.max_output_tokens.unwrap_or(4096),
        system,
        messages,
        tools,
        stream: true, // 强制使用流式
        temperature: req.temperature,
        top_p: req.top_p,
        stop_sequences: None,
        thinking,
        tool_choice: req.tool_choice.clone(),
    })
}

// ---------------------------------------------------------------------------
// Anthropic → Responses 转换
// ---------------------------------------------------------------------------

/// 将 Anthropic 响应转换为 Responses API 响应
pub fn anthropic_to_responses(resp: &AnthropicResponse, model: &str) -> ResponsesResponse {
    let mut output = Vec::new();

    // 转换内容块
    for block in &resp.content {
        match block.block_type.as_str() {
            "thinking" => {
                // 思考块 → reasoning 输出
                if let Some(thinking) = &block.thinking {
                    output.push(ResponsesOutput {
                        output_type: "reasoning".to_string(),
                        id: Some(format!("reasoning_{}", Utc::now().timestamp_millis())),
                        encrypted_content: None,
                        summary: Some(vec![ResponsesSummary {
                            summary_type: "summary_text".to_string(),
                            text: thinking.clone(),
                        }]),
                        role: None,
                        content: None,
                        status: None,
                        call_id: None,
                        name: None,
                        arguments: None,
                    });
                }
            }
            "text" => {
                // 文本块 → message 输出
                if let Some(text) = &block.text {
                    output.push(ResponsesOutput {
                        output_type: "message".to_string(),
                        id: Some(format!("msg_{}", Utc::now().timestamp_millis())),
                        role: Some("assistant".to_string()),
                        content: Some(vec![ResponsesContentPart {
                            part_type: "output_text".to_string(),
                            text: Some(text.clone()),
                            image_url: None,
                        }]),
                        status: Some("completed".to_string()),
                        encrypted_content: None,
                        summary: None,
                        call_id: None,
                        name: None,
                        arguments: None,
                    });
                }
            }
            "tool_use" => {
                // 工具使用 → function_call 输出
                output.push(ResponsesOutput {
                    output_type: "function_call".to_string(),
                    call_id: Some(block.id.clone().unwrap_or_default()),
                    name: block.name.clone(),
                    arguments: Some(
                        block
                            .input
                            .as_ref()
                            .map(|i| i.to_string())
                            .unwrap_or_default(),
                    ),
                    id: Some(format!("fc_{}", Utc::now().timestamp_millis())),
                    role: None,
                    content: None,
                    status: None,
                    encrypted_content: None,
                    summary: None,
                });
            }
            _ => {}
        }
    }

    // 如果没有输出，添加一个空的文本输出
    if output.is_empty() {
        output.push(ResponsesOutput {
            output_type: "message".to_string(),
            id: Some(format!("msg_{}", Utc::now().timestamp_millis())),
            role: Some("assistant".to_string()),
            content: Some(vec![ResponsesContentPart {
                part_type: "output_text".to_string(),
                text: Some("".to_string()),
                image_url: None,
            }]),
            status: Some("completed".to_string()),
            encrypted_content: None,
            summary: None,
            call_id: None,
            name: None,
            arguments: None,
        });
    }

    // 转换状态
    let status = match resp.stop_reason.as_str() {
        "max_tokens" => "incomplete",
        _ => "completed",
    };

    // 转换使用量
    let usage = ResponsesUsage {
        input_tokens: resp.usage.input_tokens,
        output_tokens: resp.usage.output_tokens,
        total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
        input_tokens_details: Some(ResponsesInputTokensDetails {
            cached_tokens: Some(resp.usage.cache_read_input_tokens),
        }),
        output_tokens_details: None,
    };

    ResponsesResponse {
        id: resp.id.clone(),
        object: "response".to_string(),
        model: model.to_string(),
        status: status.to_string(),
        output,
        usage: Some(usage),
        incomplete_details: if status == "incomplete" {
            Some(ResponsesIncompleteDetails {
                reason: "max_output_tokens".to_string(),
            })
        } else {
            None
        },
        error: None,
    }
}

/// 将 Anthropic 流式事件转换为 Responses 流式事件列表
pub fn anthropic_event_to_responses_events(
    event: &AnthropicStreamEvent,
    state: &mut ResponsesConverterState,
) -> Vec<ResponsesStreamEvent> {
    let mut events = Vec::new();

    match event.event_type.as_str() {
        "message_start" => {
            if let Some(message) = &event.message {
                state.message_id = Some(message.id.clone());
                state.model = Some(message.model.clone());

                events.push(ResponsesStreamEvent {
                    event_type: "response.created".to_string(),
                    response: Some(ResponsesResponse {
                        id: message.id.clone(),
                        object: "response".to_string(),
                        model: message.model.clone(),
                        status: "in_progress".to_string(),
                        output: vec![],
                        usage: None,
                        incomplete_details: None,
                        error: None,
                    }),
                    item: None,
                    output_index: 0,
                    content_index: 0,
                    delta: None,
                    text: None,
                    item_id: None,
                    call_id: None,
                    name: None,
                    arguments: None,
                    summary_index: 0,
                    code: None,
                    param: None,
                    sequence_number: state.next_sequence(),
                });
            }
        }
        "content_block_start" => {
            if let Some(block) = &event.content_block {
                let output_index = state.current_output_index;

                let item = match block.block_type.as_str() {
                    "thinking" => ResponsesOutput {
                        output_type: "reasoning".to_string(),
                        id: Some(format!("reasoning_{}", Utc::now().timestamp_millis())),
                        encrypted_content: None,
                        summary: Some(vec![]),
                        role: None,
                        content: None,
                        status: Some("in_progress".to_string()),
                        call_id: None,
                        name: None,
                        arguments: None,
                    },
                    "text" => ResponsesOutput {
                        output_type: "message".to_string(),
                        id: Some(format!("msg_{}", Utc::now().timestamp_millis())),
                        role: Some("assistant".to_string()),
                        content: Some(vec![]),
                        status: Some("in_progress".to_string()),
                        encrypted_content: None,
                        summary: None,
                        call_id: None,
                        name: None,
                        arguments: None,
                    },
                    "tool_use" => ResponsesOutput {
                        output_type: "function_call".to_string(),
                        call_id: block.id.clone(),
                        name: block.name.clone(),
                        arguments: Some(String::new()),
                        id: Some(format!("fc_{}", Utc::now().timestamp_millis())),
                        role: None,
                        content: None,
                        status: Some("in_progress".to_string()),
                        encrypted_content: None,
                        summary: None,
                    },
                    _ => return events,
                };

                state.current_block_type = Some(block.block_type.clone());

                events.push(ResponsesStreamEvent {
                    event_type: "response.output_item.added".to_string(),
                    response: None,
                    item: Some(item),
                    output_index,
                    content_index: 0,
                    delta: None,
                    text: None,
                    item_id: None,
                    call_id: None,
                    name: None,
                    arguments: None,
                    summary_index: 0,
                    code: None,
                    param: None,
                    sequence_number: state.next_sequence(),
                });
            }
        }
        "content_block_delta" => {
            if let Some(delta) = &event.delta {
                let output_index = state.current_output_index;

                match delta.delta_type.as_deref() {
                    Some("text_delta") => {
                        if let Some(text) = &delta.text {
                            events.push(ResponsesStreamEvent {
                                event_type: "response.output_text.delta".to_string(),
                                response: None,
                                item: None,
                                output_index,
                                content_index: 0,
                                delta: Some(text.clone()),
                                text: None,
                                item_id: None,
                                call_id: None,
                                name: None,
                                arguments: None,
                                summary_index: 0,
                                code: None,
                                param: None,
                                sequence_number: state.next_sequence(),
                            });
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(thinking) = &delta.thinking {
                            events.push(ResponsesStreamEvent {
                                event_type: "response.reasoning_summary_text.delta".to_string(),
                                response: None,
                                item: None,
                                output_index,
                                content_index: 0,
                                delta: Some(thinking.clone()),
                                text: None,
                                item_id: None,
                                call_id: None,
                                name: None,
                                arguments: None,
                                summary_index: 0,
                                code: None,
                                param: None,
                                sequence_number: state.next_sequence(),
                            });
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(partial_json) = &delta.partial_json {
                            events.push(ResponsesStreamEvent {
                                event_type: "response.function_call_arguments.delta".to_string(),
                                response: None,
                                item: None,
                                output_index,
                                content_index: 0,
                                delta: Some(partial_json.clone()),
                                text: None,
                                item_id: None,
                                call_id: None,
                                name: None,
                                arguments: None,
                                summary_index: 0,
                                code: None,
                                param: None,
                                sequence_number: state.next_sequence(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_stop" => {
            state.current_output_index += 1;
        }
        "message_delta" => {
            if let Some(usage) = &event.usage {
                state.usage = Some(usage.clone());
            }

            if let Some(delta) = &event.delta {
                if let Some(stop_reason) = &delta.stop_reason {
                    let status = match stop_reason.as_str() {
                        "max_tokens" => "incomplete",
                        _ => "completed",
                    };

                    events.push(ResponsesStreamEvent {
                        event_type: format!("response.{status}"),
                        response: Some(ResponsesResponse {
                            id: state.message_id.clone().unwrap_or_default(),
                            object: "response".to_string(),
                            model: state.model.clone().unwrap_or_default(),
                            status: status.to_string(),
                            output: vec![],
                            usage: state.usage.clone().map(|u| ResponsesUsage {
                                input_tokens: u.input_tokens,
                                output_tokens: u.output_tokens,
                                total_tokens: u.input_tokens + u.output_tokens,
                                input_tokens_details: Some(ResponsesInputTokensDetails {
                                    cached_tokens: Some(u.cache_read_input_tokens),
                                }),
                                output_tokens_details: None,
                            }),
                            incomplete_details: if status == "incomplete" {
                                Some(ResponsesIncompleteDetails {
                                    reason: "max_output_tokens".to_string(),
                                })
                            } else {
                                None
                            },
                            error: None,
                        }),
                        item: None,
                        output_index: 0,
                        content_index: 0,
                        delta: None,
                        text: None,
                        item_id: None,
                        call_id: None,
                        name: None,
                        arguments: None,
                        summary_index: 0,
                        code: None,
                        param: None,
                        sequence_number: state.next_sequence(),
                    });
                }
            }
        }
        _ => {}
    }

    events
}

/// Responses 转换器状态
#[derive(Debug, Default)]
pub struct ResponsesConverterState {
    pub message_id: Option<String>,
    pub model: Option<String>,
    pub current_output_index: i32,
    pub current_block_type: Option<String>,
    pub usage: Option<AnthropicUsage>,
    sequence: i32,
}

impl ResponsesConverterState {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_sequence(&mut self) -> i32 {
        self.sequence += 1;
        self.sequence
    }
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/// 将 Responses 事件转换为 SSE 格式
pub fn responses_event_to_sse(event: &ResponsesStreamEvent) -> Result<String> {
    let json = serde_json::to_string(event).context("Failed to serialize Responses event")?;
    Ok(format!("event: {}\ndata: {}\n\n", event.event_type, json))
}

/// 从请求体中提取 reasoning.effort
pub fn extract_reasoning_effort(body: &[u8]) -> Option<String> {
    if let Ok(json) = serde_json::from_slice::<JsonValue>(body) {
        if let Some(effort) = json
            .get("reasoning")
            .and_then(|r| r.get("effort"))
            .and_then(|e| e.as_str())
        {
            return Some(effort.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responses_to_anthropic_simple() {
        let req = ResponsesRequest {
            model: "gpt-4".to_string(),
            input: json!("Hello"),
            max_output_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: false,
            tools: None,
            include: None,
            store: None,
            reasoning: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = responses_to_anthropic(&req).unwrap();
        assert_eq!(anthropic.model, "gpt-4");
        assert_eq!(anthropic.max_tokens, 100);
        assert_eq!(anthropic.messages.len(), 1);
    }

    #[test]
    fn test_anthropic_to_responses() {
        let resp = AnthropicResponse {
            id: "msg_123".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![AnthropicContentBlock {
                block_type: "text".to_string(),
                text: Some("Hello".to_string()),
                thinking: None,
                source: None,
                id: None,
                name: None,
                input: None,
                tool_use_id: None,
                is_error: None,
            }],
            model: "claude-3".to_string(),
            stop_reason: "end_turn".to_string(),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };

        let responses = anthropic_to_responses(&resp, "gpt-4");
        assert_eq!(responses.id, "msg_123");
        assert_eq!(responses.status, "completed");
        assert_eq!(responses.output.len(), 1);
    }

    #[test]
    fn test_responses_with_tools() {
        use super::super::responses::ResponsesTool;

        let req = ResponsesRequest {
            model: "gpt-4".to_string(),
            input: json!("Test with tools"),
            max_output_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: false,
            tools: Some(vec![ResponsesTool {
                tool_type: "function".to_string(),
                name: Some("get_weather".to_string()),
                description: Some("Get weather info".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                })),
                strict: None,
            }]),
            include: None,
            store: None,
            reasoning: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = responses_to_anthropic(&req).unwrap();
        assert!(anthropic.tools.is_some());
        assert_eq!(anthropic.tools.unwrap().len(), 1);
    }

    #[test]
    fn test_responses_with_reasoning() {
        use super::super::responses::ResponsesReasoning;

        let req = ResponsesRequest {
            model: "gpt-4".to_string(),
            input: json!("Test reasoning"),
            max_output_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: false,
            tools: None,
            include: None,
            store: None,
            reasoning: Some(ResponsesReasoning {
                effort: "high".to_string(),
                summary: None,
            }),
            tool_choice: None,
            user: None,
        };

        let anthropic = responses_to_anthropic(&req).unwrap();
        assert_eq!(anthropic.model, "gpt-4");
    }

    #[test]
    fn test_responses_with_system_message() {
        let req = ResponsesRequest {
            model: "gpt-4".to_string(),
            input: json!([
                {"type": "message", "role": "system", "content": "You are a helpful assistant"},
                {"type": "message", "role": "user", "content": "Hello"}
            ]),
            max_output_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: false,
            tools: None,
            include: None,
            store: None,
            reasoning: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = responses_to_anthropic(&req).unwrap();
        assert!(anthropic.system.is_some());
        assert_eq!(anthropic.messages.len(), 1);
        assert_eq!(anthropic.messages[0].role, "user");
    }

    #[test]
    fn test_anthropic_with_thinking() {
        let resp = AnthropicResponse {
            id: "msg_thinking".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![
                AnthropicContentBlock {
                    block_type: "thinking".to_string(),
                    thinking: Some("Let me think...".to_string()),
                    text: None,
                    source: None,
                    id: Some("thinking_1".to_string()),
                    name: None,
                    input: None,
                    tool_use_id: None,
                    is_error: None,
                },
                AnthropicContentBlock {
                    block_type: "text".to_string(),
                    text: Some("Final answer".to_string()),
                    thinking: None,
                    source: None,
                    id: Some("text_1".to_string()),
                    name: None,
                    input: None,
                    tool_use_id: None,
                    is_error: None,
                },
            ],
            model: "claude-3".to_string(),
            stop_reason: "end_turn".to_string(),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 20,
                output_tokens: 10,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };

        let responses = anthropic_to_responses(&resp, "gpt-4");
        assert_eq!(responses.output.len(), 2);
    }

    #[test]
    fn test_extract_reasoning_effort() {
        let body = br#"{"model":"gpt-4","reasoning":{"effort":"high"}}"#;
        let effort = extract_reasoning_effort(body);
        assert_eq!(effort, Some("high".to_string()));

        let body_no_reasoning = br#"{"model":"gpt-4"}"#;
        let effort = extract_reasoning_effort(body_no_reasoning);
        assert_eq!(effort, None);
    }

    #[test]
    fn test_empty_input() {
        let req = ResponsesRequest {
            model: "gpt-4".to_string(),
            input: json!(""),
            max_output_tokens: Some(100),
            temperature: None,
            top_p: None,
            stream: false,
            tools: None,
            include: None,
            store: None,
            reasoning: None,
            tool_choice: None,
            user: None,
        };

        let anthropic = responses_to_anthropic(&req).unwrap();
        assert_eq!(anthropic.messages.len(), 1);
    }
}
