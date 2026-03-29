use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool continuation service for multi-step tool calls
pub struct OpenAIToolContinuation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallContext {
    pub call_id: String,
    pub tool_name: String,
    pub tool_args: Value,
    pub status: ToolCallStatus,
    pub result: Option<Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationRequest {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: Value,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationResponse {
    pub call_id: String,
    pub status: ToolCallStatus,
    pub result: Option<Value>,
    pub requires_continuation: bool,
    pub next_action: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContinuationError {
    #[error("Tool call not found")]
    NotFound,
    #[error("Tool call timeout")]
    Timeout,
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid tool arguments: {0}")]
    InvalidArguments(String),
}

impl OpenAIToolContinuation {
    /// Create new tool call context
    pub fn create_context(tool_name: String, tool_args: Value) -> ToolCallContext {
        let now = chrono::Utc::now().timestamp();
        ToolCallContext {
            call_id: uuid::Uuid::new_v4().to_string(),
            tool_name,
            tool_args,
            status: ToolCallStatus::Pending,
            result: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if tool call requires continuation
    pub fn requires_continuation(context: &ToolCallContext) -> bool {
        matches!(
            context.status,
            ToolCallStatus::Pending | ToolCallStatus::Running
        )
    }

    /// Update tool call status
    pub fn update_status(
        context: &mut ToolCallContext,
        status: ToolCallStatus,
        result: Option<Value>,
    ) {
        context.status = status;
        context.result = result;
        context.updated_at = chrono::Utc::now().timestamp();
    }

    /// Build continuation response
    pub fn build_response(
        context: &ToolCallContext,
        requires_continuation: bool,
    ) -> ContinuationResponse {
        ContinuationResponse {
            call_id: context.call_id.clone(),
            status: context.status.clone(),
            result: context.result.clone(),
            requires_continuation,
            next_action: None,
        }
    }

    /// Validate tool arguments
    pub fn validate_args(
        _tool_name: &str,
        args: &Value,
        schema: &Value,
    ) -> Result<(), ContinuationError> {
        // Simple validation - check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for field in required {
                if let Some(field_name) = field.as_str() {
                    if args.get(field_name).is_none() {
                        return Err(ContinuationError::InvalidArguments(format!(
                            "Missing required field: {}",
                            field_name
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate tool call message for API
    pub fn generate_tool_message(tool_call_id: &str, tool_name: &str, result: &Value) -> Value {
        serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "name": tool_name,
            "content": serde_json::to_string(result).unwrap_or_default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_context() {
        let context = OpenAIToolContinuation::create_context(
            "test_tool".to_string(),
            serde_json::json!({"arg": "value"}),
        );

        assert_eq!(context.tool_name, "test_tool");
        assert_eq!(context.status, ToolCallStatus::Pending);
    }

    #[test]
    fn test_requires_continuation() {
        let context = ToolCallContext {
            call_id: "test".to_string(),
            tool_name: "test".to_string(),
            tool_args: serde_json::json!({}),
            status: ToolCallStatus::Pending,
            result: None,
            created_at: 0,
            updated_at: 0,
        };

        assert!(OpenAIToolContinuation::requires_continuation(&context));

        let completed_context = ToolCallContext {
            status: ToolCallStatus::Completed,
            ..context.clone()
        };

        assert!(!OpenAIToolContinuation::requires_continuation(
            &completed_context
        ));
    }
}
