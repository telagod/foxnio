//! 网关核心测试

#[cfg(test)]
mod tests {
    use crate::gateway::{UpstreamEndpoints, SseEvent, failover::FailoverConfig};

    #[test]
    fn test_upstream_endpoints() {
        assert_eq!(
            UpstreamEndpoints::anthropic(),
            "https://api.anthropic.com"
        );
        assert_eq!(
            UpstreamEndpoints::openai(),
            "https://api.openai.com"
        );
        assert_eq!(
            UpstreamEndpoints::gemini(),
            "https://generativelanguage.googleapis.com"
        );
    }

    #[test]
    fn test_parse_sse_event() {
        let data = "event: message\ndata: {\"text\":\"hello\"}\n\n";
        let event = SseEvent::parse(data);
        
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_type, Some("message".to_string()));
        assert_eq!(event.data, r#"{"text":"hello"}"#);
    }

    #[test]
    fn test_parse_sse_event_multiline() {
        let data = "data: line1\ndata: line2\n\n";
        let event = SseEvent::parse(data);
        
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.data, "line1\nline2");
    }

    #[test]
    fn test_parse_sse_event_done() {
        let data = "[DONE]";
        let event = SseEvent::parse(data);
        
        assert!(event.is_none());
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

    #[test]
    fn test_failover_config_default() {
        let config = FailoverConfig::default();
        
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 100);
        assert_eq!(config.backoff_base, 2.0);
        assert_eq!(config.max_backoff_ms, 5000);
    }

    #[test]
    fn test_openai_stream_chunk() {
        use crate::gateway::stream::{OpenAIStreamParser, OpenAIStreamChunk};
        
        let data = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk = OpenAIStreamParser::parse_chunk(data);
        
        assert!(chunk.is_some());
        let chunk = chunk.unwrap();
        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(OpenAIStreamParser::extract_delta(&chunk), Some("Hello".to_string()));
        assert!(!OpenAIStreamParser::is_finished(&chunk));
    }

    #[test]
    fn test_openai_stream_finished() {
        use crate::gateway::stream::{OpenAIStreamParser, OpenAIStreamChunk};
        
        let data = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let chunk = OpenAIStreamParser::parse_chunk(data);
        
        assert!(chunk.is_some());
        let chunk = chunk.unwrap();
        assert!(OpenAIStreamParser::is_finished(&chunk));
    }

    #[test]
    fn test_usage_tracker() {
        use crate::gateway::stream::UsageTracker;
        
        let mut tracker = UsageTracker::new();
        let (input, output) = tracker.get_usage();
        
        assert_eq!(input, 0);
        assert_eq!(output, 0);
    }
}
