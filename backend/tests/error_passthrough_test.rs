#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! Tests for error passthrough rule service

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use foxnio::entity::error_passthrough_rules::Model;
    use serde_json::json;

    fn create_test_rule() -> Model {
        Model {
            id: 1,
            name: "Test Rule".to_string(),
            enabled: true,
            priority: 0,
            error_codes: Some(json!([429, 500])),
            keywords: Some(json!(["rate limit", "timeout"])),
            match_mode: "any".to_string(),
            platforms: Some(json!(["openai", "anthropic"])),
            passthrough_code: true,
            response_code: None,
            passthrough_body: true,
            custom_message: None,
            skip_monitoring: false,
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_matches_error_code() {
        let rule = create_test_rule();

        // Should match error code
        assert!(rule.matches(Some(429), None, Some("openai")));

        // Should match error code
        assert!(rule.matches(Some(500), None, Some("anthropic")));

        // Should not match different error code
        assert!(!rule.matches(Some(400), None, Some("openai")));
    }

    #[test]
    fn test_matches_keyword() {
        let rule = create_test_rule();

        // Should match keyword
        assert!(rule.matches(None, Some("rate limit exceeded"), Some("openai")));

        // Should match keyword
        assert!(rule.matches(None, Some("request timeout"), Some("anthropic")));

        // Should not match different keyword
        assert!(!rule.matches(None, Some("invalid request"), Some("openai")));
    }

    #[test]
    fn test_matches_platform() {
        let rule = create_test_rule();

        // Should match platform
        assert!(rule.matches(Some(429), None, Some("openai")));

        // Should not match different platform
        assert!(!rule.matches(Some(429), None, Some("google")));
    }

    #[test]
    fn test_disabled_rule() {
        let mut rule = create_test_rule();
        rule.enabled = false;

        // Should not match when disabled
        assert!(!rule.matches(Some(429), None, Some("openai")));
    }
}
