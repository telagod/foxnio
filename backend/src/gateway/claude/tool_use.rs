//! Claude Tool Use 支持
//!
//! 提供 Anthropic Claude 工具调用功能的解析、处理和响应
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 输入参数 schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

/// 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseRequest {
    /// 工具调用 ID
    pub id: String,
    /// 工具类型（通常是 "function"）
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 工具名称
    pub name: String,
    /// 输入参数
    pub input: serde_json::Value,
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseResult {
    /// 工具调用 ID
    pub tool_use_id: String,
    /// 内容类型
    #[serde(rename = "type")]
    pub content_type: String,
    /// 结果内容
    pub content: String,
    /// 是否出错
    pub is_error: Option<bool>,
}

/// 工具调用处理器
pub struct ToolUseHandler {
    /// 已注册的工具
    tools: HashMap<String, ToolDefinition>,
}

impl ToolUseHandler {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册工具
    pub fn register_tool(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// 批量注册工具
    pub fn register_tools(&mut self, tools: Vec<ToolDefinition>) {
        for tool in tools {
            self.register_tool(tool);
        }
    }

    /// 获取所有工具定义
    pub fn get_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    /// 获取工具定义（Claude API 格式）
    pub fn get_tools_for_claude(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema
                })
            })
            .collect()
    }

    /// 解析 Claude 响应中的工具调用
    pub fn parse_tool_uses(&self, content: &[serde_json::Value]) -> Result<Vec<ToolUseRequest>> {
        let mut tool_uses = Vec::new();

        for item in content {
            if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                if item_type == "tool_use" {
                    let tool_use = ToolUseRequest {
                        id: item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        tool_type: "function".to_string(),
                        name: item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        input: item
                            .get("input")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    };
                    tool_uses.push(tool_use);
                }
            }
        }

        Ok(tool_uses)
    }

    /// 验证工具调用参数
    pub fn validate_tool_use(&self, tool_use: &ToolUseRequest) -> Result<(), ToolUseError> {
        // 检查工具是否存在
        let tool = self
            .tools
            .get(&tool_use.name)
            .ok_or_else(|| ToolUseError::ToolNotFound(tool_use.name.clone()))?;

        // 简单验证：检查必需字段
        if let Some(schema) = tool.input_schema.get("required") {
            if let Some(required) = schema.as_array() {
                for field in required {
                    if let Some(field_name) = field.as_str() {
                        if tool_use.input.get(field_name).is_none() {
                            return Err(ToolUseError::MissingRequiredField(
                                tool_use.name.clone(),
                                field_name.to_string(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 构建工具结果消息
    pub fn build_tool_result_message(&self, results: Vec<ToolUseResult>) -> serde_json::Value {
        let content: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": r.tool_use_id,
                    "content": r.content,
                    "is_error": r.is_error.unwrap_or(false)
                })
            })
            .collect();

        serde_json::json!({
            "role": "user",
            "content": content
        })
    }

    /// 构建 OpenAI 兼容的工具结果
    pub fn build_openai_tool_result_message(
        &self,
        results: Vec<ToolUseResult>,
    ) -> serde_json::Value {
        let content: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "tool_call_id": r.tool_use_id,
                    "role": "tool",
                    "content": r.content
                })
            })
            .collect();

        serde_json::json!({
            "role": "tool",
            "content": content
        })
    }

    /// 转换 OpenAI 工具定义为 Claude 格式
    pub fn convert_openai_tools_to_claude(
        openai_tools: &[serde_json::Value],
    ) -> Vec<ToolDefinition> {
        openai_tools
            .iter()
            .filter_map(|t| {
                t.get("function").map(|function| ToolDefinition {
                    name: function
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    description: function
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    input_schema: function
                        .get("parameters")
                        .cloned()
                        .unwrap_or(serde_json::json!({"type": "object"})),
                })
            })
            .collect()
    }

    /// 转换 Claude 工具调用为 OpenAI 格式
    pub fn convert_claude_tool_uses_to_openai(
        tool_uses: &[ToolUseRequest],
    ) -> Vec<serde_json::Value> {
        tool_uses
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "arguments": serde_json::to_string(&t.input).unwrap_or_default()
                    }
                })
            })
            .collect()
    }
}

impl Default for ToolUseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具调用错误
#[derive(Debug, Clone)]
pub enum ToolUseError {
    /// 工具未找到
    ToolNotFound(String),
    /// 缺少必需字段
    MissingRequiredField(String, String),
    /// 参数验证失败
    ValidationError(String),
    /// 执行错误
    ExecutionError(String),
}

impl std::fmt::Display for ToolUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ToolNotFound(name) => write!(f, "Tool not found: {name}"),
            Self::MissingRequiredField(tool, field) => {
                write!(f, "Missing required field '{field}' for tool '{tool}'")
            }
            Self::ValidationError(msg) => write!(f, "Validation error: {msg}"),
            Self::ExecutionError(msg) => write!(f, "Execution error: {msg}"),
        }
    }
}

impl std::error::Error for ToolUseError {}

/// 工具调用助手函数
pub mod helpers {
    use super::*;

    /// 创建简单的工具定义
    pub fn create_simple_tool(
        name: &str,
        description: &str,
        parameters: Option<serde_json::Value>,
    ) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: parameters.unwrap_or(serde_json::json!({"type": "object"})),
        }
    }

    /// 创建带参数的工具定义
    pub fn create_tool_with_params(
        name: &str,
        description: &str,
        params: Vec<(&str, &str, bool)>, // (name, type, required)
    ) -> ToolDefinition {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (param_name, param_type, is_required) in params {
            properties.insert(
                param_name.to_string(),
                serde_json::json!({
                    "type": param_type
                }),
            );
            if is_required {
                required.push(param_name);
            }
        }

        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": properties,
                "required": required
            }),
        }
    }

    /// 创建成功的工具结果
    pub fn create_success_result(tool_use_id: &str, content: &str) -> ToolUseResult {
        ToolUseResult {
            tool_use_id: tool_use_id.to_string(),
            content_type: "tool_result".to_string(),
            content: content.to_string(),
            is_error: Some(false),
        }
    }

    /// 创建错误的工具结果
    pub fn create_error_result(tool_use_id: &str, error: &str) -> ToolUseResult {
        ToolUseResult {
            tool_use_id: tool_use_id.to_string(),
            content_type: "tool_result".to_string(),
            content: error.to_string(),
            is_error: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registration() {
        let mut handler = ToolUseHandler::new();

        handler.register_tool(ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather info".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        });

        assert_eq!(handler.tools.len(), 1);
    }

    #[test]
    fn test_parse_tool_uses() {
        let handler = ToolUseHandler::new();

        let content = vec![
            serde_json::json!({
                "type": "tool_use",
                "id": "tool_123",
                "name": "get_weather",
                "input": {"location": "Beijing"}
            }),
            serde_json::json!({
                "type": "text",
                "text": "Let me check the weather."
            }),
        ];

        let tool_uses = handler.parse_tool_uses(&content).unwrap();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].name, "get_weather");
    }

    #[test]
    fn test_validate_tool_use() {
        let mut handler = ToolUseHandler::new();

        handler.register_tool(ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["location"]
            }),
        });

        // 有效的调用
        let valid = ToolUseRequest {
            id: "tool_1".to_string(),
            tool_type: "function".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({"location": "Beijing"}),
        };
        assert!(handler.validate_tool_use(&valid).is_ok());

        // 无效的调用（缺少必需字段）
        let invalid = ToolUseRequest {
            id: "tool_2".to_string(),
            tool_type: "function".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({}),
        };
        assert!(handler.validate_tool_use(&invalid).is_err());
    }

    #[test]
    fn test_convert_openai_tools() {
        let openai_tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "search",
                "description": "Search the web",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    }
                }
            }
        })];

        let claude_tools = ToolUseHandler::convert_openai_tools_to_claude(&openai_tools);
        assert_eq!(claude_tools.len(), 1);
        assert_eq!(claude_tools[0].name, "search");
    }

    #[test]
    fn test_helpers() {
        let tool = helpers::create_simple_tool("test", "A test tool", None);
        assert_eq!(tool.name, "test");

        let tool = helpers::create_tool_with_params(
            "weather",
            "Get weather",
            vec![("city", "string", true)],
        );
        assert!(tool.input_schema.get("required").is_some());

        let result = helpers::create_success_result("tool_1", "Success");
        assert_eq!(result.is_error, Some(false));
    }
}
