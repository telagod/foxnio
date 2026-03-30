// 错误处理 - Anthropic API 错误响应

use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};

/// Anthropic API 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicError {
    /// 错误详情
    pub error: ErrorDetail,
}

/// 错误详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// 错误类型
    #[serde(rename = "type")]
    pub error_type: String,
    
    /// 错误消息
    pub message: String,
}

/// 解析 Anthropic 错误响应
pub fn parse_error(response_body: &str) -> Result<AnthropicError> {
    let error: AnthropicError = serde_json::from_str(response_body)
        .map_err(|e| anyhow!("Failed to parse error response: {}", e))?;
    Ok(error)
}

/// 错误类型常量
pub mod error_types {
    pub const INVALID_REQUEST_ERROR: &str = "invalid_request_error";
    pub const AUTHENTICATION_ERROR: &str = "authentication_error";
    pub const PERMISSION_ERROR: &str = "permission_error";
    pub const NOT_FOUND_ERROR: &str = "not_found_error";
    pub const RATE_LIMIT_ERROR: &str = "rate_limit_error";
    pub const API_ERROR: &str = "api_error";
    pub const OVERLOADED_ERROR: &str = "overloaded_error";
}

impl AnthropicError {
    /// 是否为认证错误
    pub fn is_authentication_error(&self) -> bool {
        self.error.error_type == error_types::AUTHENTICATION_ERROR
    }
    
    /// 是否为限流错误
    pub fn is_rate_limit_error(&self) -> bool {
        self.error.error_type == error_types::RATE_LIMIT_ERROR
    }
    
    /// 是否为过载错误（需要重试）
    pub fn is_overloaded_error(&self) -> bool {
        self.error.error_type == error_types::OVERLOADED_ERROR
    }
    
    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.error.error_type.as_str(),
            error_types::RATE_LIMIT_ERROR | 
            error_types::API_ERROR | 
            error_types::OVERLOADED_ERROR
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let json = r#"{
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid request"
            }
        }"#;
        
        let error = parse_error(json).unwrap();
        assert_eq!(error.error.error_type, "invalid_request_error");
        assert_eq!(error.error.message, "Invalid request");
    }

    #[test]
    fn test_error_types() {
        let error = AnthropicError {
            error: ErrorDetail {
                error_type: error_types::RATE_LIMIT_ERROR.to_string(),
                message: "Rate limit exceeded".to_string(),
            },
        };
        
        assert!(error.is_rate_limit_error());
        assert!(error.is_retryable());
    }
}
