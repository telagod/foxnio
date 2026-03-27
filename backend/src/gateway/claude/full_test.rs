//! Claude 指纹完整集成测试

#[cfg(test)]
mod tests {
    use crate::gateway::claude::{
        ClaudeCodeValidator, ClaudeHeaders, TLSFingerprint,
        get_beta_header, normalize_model_id, denormalize_model_id,
        header_wire_casing, sort_headers_by_wire_order,
        parse_metadata_user_id, build_claude_headers_ordered,
        DEFAULT_CIPHER_SUITES, DEFAULT_CURVES,
    };
    use std::collections::HashMap;
    
    // ============ Validator Tests ============
    
    #[test]
    fn test_validator_user_agent() {
        let validator = ClaudeCodeValidator::new();
        
        // 有效 UA
        assert!(validator.validate_user_agent("claude-cli/2.1.22 (external, cli)"));
        assert!(validator.validate_user_agent("claude-cli/1.0.0"));
        assert!(validator.validate_user_agent("Claude-CLI/3.0.0 (darwin; arm64)"));
        
        // 无效 UA
        assert!(!validator.validate_user_agent("curl/8.0.0"));
        assert!(!validator.validate_user_agent("Mozilla/5.0"));
    }
    
    #[test]
    fn test_validator_version_extraction() {
        let validator = ClaudeCodeValidator::new();
        
        assert_eq!(
            validator.extract_version("claude-cli/2.1.22 (darwin; arm64)"),
            Some("2.1.22".to_string())
        );
        assert_eq!(
            validator.extract_version("claude-cli/1.0.0"),
            Some("1.0.0".to_string())
        );
        assert_eq!(validator.extract_version("curl/8.0.0"), None);
    }
    
    #[test]
    fn test_validator_version_compare() {
        use crate::gateway::claude::ClaudeCodeValidator;
        
        assert_eq!(ClaudeCodeValidator::compare_versions("2.1.0", "2.1.0"), 0);
        assert_eq!(ClaudeCodeValidator::compare_versions("2.1.1", "2.1.0"), 1);
        assert_eq!(ClaudeCodeValidator::compare_versions("2.0.0", "2.1.0"), -1);
        assert_eq!(ClaudeCodeValidator::compare_versions("3.0.0", "2.99.99"), 1);
    }
    
    #[test]
    fn test_validator_system_prompt() {
        let validator = ClaudeCodeValidator::new();
        
        // 包含 Claude Code system prompt
        let body = serde_json::json!({
            "system": [
                {"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}
            ]
        });
        assert!(validator.has_claude_code_system_prompt(&body));
        
        // 部分匹配
        let body = serde_json::json!({
            "system": [
                {"type": "text", "text": "You are an interactive CLI tool that helps users write code and debug"}
            ]
        });
        assert!(validator.has_claude_code_system_prompt(&body));
        
        // 不匹配
        let body = serde_json::json!({
            "system": [
                {"type": "text", "text": "You are a helpful assistant."}
            ]
        });
        assert!(!validator.has_claude_code_system_prompt(&body));
    }
    
    #[test]
    fn test_parse_metadata_user_id() {
        // 有效格式
        let result = parse_metadata_user_id("cli:abc123:def456");
        assert_eq!(result, Some(("cli".to_string(), "abc123".to_string(), "def456".to_string())));
        
        // 无效格式
        assert_eq!(parse_metadata_user_id("invalid"), None);
        assert_eq!(parse_metadata_user_id("a:b"), None);
        assert_eq!(parse_metadata_user_id("::"), None);
        assert_eq!(parse_metadata_user_id(""), None);
    }
    
    // ============ Header Tests ============
    
    #[test]
    fn test_header_wire_casing() {
        // X-Stainless-OS（特别注意：不是 X-Stainless-Os）
        assert_eq!(header_wire_casing("X-Stainless-Os"), "X-Stainless-OS");
        assert_eq!(header_wire_casing("x-stainless-os"), "X-Stainless-OS");
        assert_eq!(header_wire_casing("X-STAINLESS-OS"), "X-Stainless-OS");
        
        // 全小写 headers
        assert_eq!(header_wire_casing("X-App"), "x-app");
        assert_eq!(header_wire_casing("x-app"), "x-app");
        assert_eq!(header_wire_casing("Anthropic-Beta"), "anthropic-beta");
        assert_eq!(header_wire_casing("Anthropic-Version"), "anthropic-version");
        
        // Title case
        assert_eq!(header_wire_casing("accept"), "Accept");
        assert_eq!(header_wire_casing("user-agent"), "User-Agent");
    }
    
    #[test]
    fn test_sort_headers_by_wire_order() {
        let mut headers = HashMap::new();
        headers.insert("anthropic-beta".to_string(), "beta".to_string());
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("x-app".to_string(), "cli".to_string());
        headers.insert("authorization".to_string(), "Bearer token".to_string());
        
        let sorted = sort_headers_by_wire_order(&headers);
        
        // Accept 应该在第一位
        assert_eq!(sorted[0].0, "Accept");
        
        // 验证顺序：x-app 在 authorization 后，anthropic-beta 前
        let x_app_pos = sorted.iter().position(|(k, _)| k == "x-app").unwrap();
        let auth_pos = sorted.iter().position(|(k, _)| k == "authorization").unwrap();
        let beta_pos = sorted.iter().position(|(k, _)| k == "anthropic-beta").unwrap();
        
        assert!(auth_pos < x_app_pos);
        assert!(x_app_pos < beta_pos);
    }
    
    #[test]
    fn test_build_claude_headers_ordered() {
        let headers = build_claude_headers_ordered(
            "test-api-key",
            "claude-code-20250219",
            "claude-cli/2.1.22 (external, cli)",
            "2023-06-01"
        );
        
        // 验证数量
        assert_eq!(headers.len(), 19);
        
        // 验证关键位置
        assert_eq!(headers[0].0, "Accept");
        assert_eq!(headers[5].0, "X-Stainless-OS");
        assert_eq!(headers[11].0, "authorization");
        assert_eq!(headers[12].0, "x-app");
        assert_eq!(headers[13].0, "User-Agent");
        assert_eq!(headers[15].0, "anthropic-beta");
        
        // 验证值
        assert!(headers[11].1.contains("test-api-key"));
        assert_eq!(headers[15].1, "claude-code-20250219");
    }
    
    // ============ ClaudeHeaders Tests ============
    
    #[test]
    fn test_claude_headers_build() {
        let headers = ClaudeHeaders::default();
        let auth_token = "test-token";
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        
        let header_map = headers.build(auth_token, &beta);
        
        // 验证关键 header 存在
        assert!(header_map.contains_key("authorization"));
        assert!(header_map.contains_key("x-app"));
        assert!(header_map.contains_key("anthropic-beta"));
        assert!(header_map.contains_key("User-Agent"));
        assert!(header_map.contains_key("X-Stainless-OS"));
    }
    
    #[test]
    fn test_claude_headers_build_ordered() {
        let headers = ClaudeHeaders::default();
        let auth_token = "test-token";
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        
        let ordered = headers.build_ordered(auth_token, &beta);
        
        // 验证顺序
        assert_eq!(ordered[0].0, "Accept");
        assert_eq!(ordered[5].0, "X-Stainless-OS");
        
        // 验证 x-app 是小写
        let x_app = ordered.iter().find(|(k, _)| *k == "x-app");
        assert!(x_app.is_some());
    }
    
    // ============ Beta Header Tests ============
    
    #[test]
    fn test_beta_header_oauth_sonnet() {
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        assert!(beta.contains("claude-code"));
        assert!(beta.contains("oauth"));
        assert!(beta.contains("interleaved-thinking"));
    }
    
    #[test]
    fn test_beta_header_api_key_sonnet() {
        let beta = get_beta_header(false, "claude-sonnet-4-5");
        assert!(beta.contains("claude-code"));
        assert!(!beta.contains("oauth"));
        assert!(beta.contains("interleaved-thinking"));
    }
    
    #[test]
    fn test_beta_header_oauth_haiku() {
        let beta = get_beta_header(true, "claude-haiku-4-5");
        assert!(beta.contains("oauth"));
        assert!(!beta.contains("claude-code"));
    }
    
    #[test]
    fn test_beta_header_api_key_haiku() {
        let beta = get_beta_header(false, "claude-haiku-4-5");
        assert!(!beta.contains("oauth"));
        assert!(!beta.contains("claude-code"));
    }
    
    // ============ Model ID Tests ============
    
    #[test]
    fn test_model_id_normalization() {
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5-20250929"
        );
        assert_eq!(
            normalize_model_id("claude-opus-4-5"),
            "claude-opus-4-5-20251101"
        );
        assert_eq!(
            normalize_model_id("claude-haiku-4-5"),
            "claude-haiku-4-5-20251001"
        );
        
        // 已是完整名则不变
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5-20250929"
        );
    }
    
    #[test]
    fn test_model_id_denormalization() {
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5"
        );
        assert_eq!(
            denormalize_model_id("claude-opus-4-5-20251101"),
            "claude-opus-4-5"
        );
        
        // 已是短名则不变
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5"
        );
    }
    
    // ============ TLS Fingerprint Tests ============
    
    #[test]
    fn test_tls_fingerprint() {
        let fp = TLSFingerprint::default();
        
        // 验证密码套件数量
        assert_eq!(fp.cipher_suites.len(), 17);
        
        // 验证曲线数量
        assert_eq!(fp.curves.len(), 3);
        
        // 验证 GREASE 启用
        assert!(fp.enable_grease);
        
        // 验证 ALPN
        assert_eq!(fp.alpn_protocols, vec!["http/1.1"]);
    }
    
    #[test]
    fn test_cipher_suite_order() {
        // 验证第一个是 TLS 1.3 AES-128
        assert_eq!(DEFAULT_CIPHER_SUITES[0], 0x1301);
        
        // 验证第 4 个是 ECDHE-ECDSA-AES128-GCM
        assert_eq!(DEFAULT_CIPHER_SUITES[3], 0xc02b);
        
        // 验证最后一个是 RSA-AES256-SHA
        assert_eq!(DEFAULT_CIPHER_SUITES[16], 0x0035);
    }
    
    #[test]
    fn test_curve_order() {
        // 验证顺序：X25519, P256, P384
        assert_eq!(DEFAULT_CURVES[0], 0x001d); // X25519
        assert_eq!(DEFAULT_CURVES[1], 0x0017); // P256
        assert_eq!(DEFAULT_CURVES[2], 0x0018); // P384
    }
    
    // ============ Integration Tests ============
    
    #[test]
    fn test_full_request_simulation() {
        // 模拟完整请求
        let validator = ClaudeCodeValidator::new();
        let headers = ClaudeHeaders::default();
        
        let ua = "claude-cli/2.1.22 (external, cli)";
        let model = "claude-sonnet-4-5";
        let auth_token = "test-token";
        
        // 1. 验证 User-Agent
        assert!(validator.validate_user_agent(ua));
        
        // 2. 提取版本
        let version = validator.extract_version(ua);
        assert_eq!(version, Some("2.1.22".to_string()));
        
        // 3. 获取 beta header
        let beta = get_beta_header(true, model);
        assert!(beta.contains("oauth"));
        
        // 4. 标准化模型 ID
        let normalized_model = normalize_model_id(model);
        assert_eq!(normalized_model, "claude-sonnet-4-5-20250929");
        
        // 5. 构建请求头
        let ordered_headers = headers.build_ordered(auth_token, &beta);
        
        // 验证关键 headers
        let auth = ordered_headers.iter().find(|(k, _)| k == "authorization");
        assert!(auth.is_some());
        assert!(auth.unwrap().1.contains(auth_token));
        
        let x_app = ordered_headers.iter().find(|(k, _)| k == "x-app");
        assert!(x_app.is_some());
        assert_eq!(x_app.unwrap().1, "cli");
        
        let anthropic_beta = ordered_headers.iter().find(|(k, _)| k == "anthropic-beta");
        assert!(anthropic_beta.is_some());
        assert_eq!(anthropic_beta.unwrap().1, beta);
    }
    
    #[test]
    fn test_request_validation_flow() {
        let validator = ClaudeCodeValidator::new();
        
        // 模拟 Claude Code 请求体
        let body = serde_json::json!({
            "model": "claude-sonnet-4-5",
            "system": [
                {"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}
            ],
            "metadata": {
                "user_id": "cli:device123:session456"
            }
        });
        
        // 验证 system prompt
        assert!(validator.has_claude_code_system_prompt(&body));
        
        // 验证 metadata.user_id
        let user_id = body["metadata"]["user_id"].as_str().unwrap();
        let parsed = parse_metadata_user_id(user_id);
        assert!(parsed.is_some());
        
        let (device_type, device_id, session_id) = parsed.unwrap();
        assert_eq!(device_type, "cli");
        assert_eq!(device_id, "device123");
        assert_eq!(session_id, "session456");
    }
}
