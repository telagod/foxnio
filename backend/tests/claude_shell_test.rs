// Claude Code Shell 集成测试

use foxnio_gateway::claude_shell::{
    parse_sse_line, parse_sse_stream,
    request::{Message, MessageContent, MessageRequest},
    ClaudeShell, ClaudeShellConfig,
};

/// 测试默认配置
#[test]
fn test_default_config() {
    let config = ClaudeShellConfig::default();
    assert_eq!(config.base_url, "https://api.anthropic.com");
    assert_eq!(config.api_version, "2023-06-01");
    assert!(config.stream);
}

/// 测试配置构建
#[test]
fn test_custom_config() {
    let config = ClaudeShellConfig {
        api_key: "test-key".to_string(),
        base_url: "https://custom.api.com".to_string(),
        api_version: "2024-01-01".to_string(),
        stream: false,
    };

    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.base_url, "https://custom.api.com");
    assert_eq!(config.api_version, "2024-01-01");
    assert!(!config.stream);
}

/// 测试请求构建
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
        system: Some("You are helpful".to_string()),
        temperature: Some(0.7),
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: None,
        metadata: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("claude-3-5-sonnet-20241022"));
    assert!(json.contains("Hello"));
    assert!(json.contains("You are helpful"));
    assert!(json.contains("0.7"));
}

/// 测试 SSE 解析
#[test]
fn test_sse_parsing() {
    let line = r#"data: {"type": "message_start", "message": {"id": "msg_xxx", "type": "message", "role": "assistant", "model": "claude-3-5-sonnet-20241022", "content": [], "usage": {"input_tokens": 10}}}"#;

    let event = parse_sse_line(line);
    assert!(event.is_some());

    let event = event.unwrap();
    assert_eq!(event.event_type, "message_start");
    assert!(event.message.is_some());
}

/// 测试 SSE 流解析
#[test]
fn test_sse_stream_parsing() {
    let stream = r#"data: {"type": "message_start", "message": {"id": "msg_xxx", "type": "message", "role": "assistant", "model": "claude-3-5-sonnet-20241022", "content": [], "usage": {"input_tokens": 10}}}
data: {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}
data: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}
data: [DONE]"#;

    let events = parse_sse_stream(stream);
    assert_eq!(events.len(), 3);

    assert_eq!(events[0].event_type, "message_start");
    assert_eq!(events[1].event_type, "content_block_start");
    assert_eq!(events[2].event_type, "content_block_delta");
}

/// 测试错误响应解析
#[test]
fn test_error_parsing() {
    use foxnio_gateway::claude_shell::AnthropicError;

    let json = r#"{
        "error": {
            "type": "invalid_request_error",
            "message": "Invalid request: missing required field"
        }
    }"#;

    let error: AnthropicError = serde_json::from_str(json).unwrap();
    assert_eq!(error.error.error_type, "invalid_request_error");
    assert_eq!(
        error.error.message,
        "Invalid request: missing required field"
    );
}

/// 测试错误类型判断
#[test]
fn test_error_types() {
    use foxnio_gateway::claude_shell::{error_types, AnthropicError, ErrorDetail};

    let rate_limit_error = AnthropicError {
        error: ErrorDetail {
            error_type: error_types::RATE_LIMIT_ERROR.to_string(),
            message: "Rate limit exceeded".to_string(),
        },
    };

    assert!(rate_limit_error.is_rate_limit_error());
    assert!(rate_limit_error.is_retryable());

    let auth_error = AnthropicError {
        error: ErrorDetail {
            error_type: error_types::AUTHENTICATION_ERROR.to_string(),
            message: "Invalid API key".to_string(),
        },
    };

    assert!(auth_error.is_authentication_error());
    assert!(!auth_error.is_retryable());
}

/// 测试客户端创建（无 API key）
#[test]
fn test_client_creation() {
    let config = ClaudeShellConfig::default();
    let result = ClaudeShell::new(config);

    // 应该能创建客户端（即使没有 API key）
    assert!(result.is_ok());
}

/// 真实 API 测试（需要 API key，默认忽略）
#[tokio::test]
#[ignore]
async fn test_real_api() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let config = ClaudeShellConfig {
        api_key,
        ..Default::default()
    };

    let shell = ClaudeShell::new(config).unwrap();

    let request = MessageRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Say 'Hello'".to_string()),
        }],
        max_tokens: 100,
        stream: None,
        system: None,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: None,
        metadata: None,
    };

    let response = shell.send_message(request).await.unwrap();

    assert!(!response.content.is_empty());
    println!("Response: {:?}", response.content);
}

/// 测试流式 API（需要 API key，默认忽略）
#[tokio::test]
#[ignore]
async fn test_real_streaming_api() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let config = ClaudeShellConfig {
        api_key,
        ..Default::default()
    };

    let shell = ClaudeShell::new(config).unwrap();

    let request = MessageRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Count from 1 to 10".to_string()),
        }],
        max_tokens: 100,
        stream: Some(true),
        system: None,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: None,
        metadata: None,
    };

    let response = shell.send_message_stream(request).await.unwrap();

    assert!(response.status().is_success());

    // 这里可以进一步解析 SSE 流
    // use futures::StreamExt;
    // let mut stream = response.bytes_stream();
    // while let Some(chunk) = stream.next().await { ... }
}

/// 测试连接（需要 API key，默认忽略）
#[tokio::test]
#[ignore]
async fn test_connection() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    let config = ClaudeShellConfig {
        api_key,
        ..Default::default()
    };

    let shell = ClaudeShell::new(config).unwrap();

    let is_connected = shell.test_connection().await.unwrap();
    assert!(is_connected);
}

/// 测试无效 API key
#[tokio::test]
#[ignore]
async fn test_invalid_api_key() {
    let config = ClaudeShellConfig {
        api_key: "invalid-key".to_string(),
        ..Default::default()
    };

    let shell = ClaudeShell::new(config).unwrap();

    let is_connected = shell.test_connection().await.unwrap();
    assert!(!is_connected);
}
