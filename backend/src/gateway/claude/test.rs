//! Claude 渠道集成测试

#[cfg(test)]
mod tests {
    use crate::gateway::claude::{
        get_beta_header, normalize_model_id, denormalize_model_id,
        ClaudeHeaders, TLSFingerprint,
        DEFAULT_CIPHER_SUITES, DEFAULT_CURVES,
    };
    
    #[test]
    fn test_full_headers_build() {
        let headers = ClaudeHeaders::default();
        let auth_token = "test-api-key";
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        
        let header_map = headers.build(auth_token, &beta);
        
        // 验证关键 header 存在
        assert!(header_map.contains_key("authorization"));
        assert!(header_map.contains_key("x-app"));
        assert!(header_map.contains_key("anthropic-beta"));
        assert!(header_map.contains_key("User-Agent"));
        
        // 验证 User-Agent
        let ua = header_map.get("User-Agent").unwrap();
        assert_eq!(ua, "claude-cli/2.1.22 (external, cli)");
    }
    
    #[test]
    fn test_ordered_headers() {
        let headers = ClaudeHeaders::default();
        let auth_token = "test-api-key";
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        
        let ordered = headers.build_ordered(auth_token, &beta);
        
        // 验证顺序
        assert_eq!(ordered[0].0, "Accept");
        assert_eq!(ordered[1].0, "X-Stainless-Retry-Count");
        assert_eq!(ordered[5].0, "X-Stainless-OS");
        
        // 验证大小写
        let x_app = ordered.iter().find(|(k, _)| *k == "x-app");
        assert!(x_app.is_some());
        
        let anthropic_beta = ordered.iter().find(|(k, _)| *k == "anthropic-beta");
        assert!(anthropic_beta.is_some());
    }
    
    #[test]
    fn test_beta_header_scenarios() {
        // OAuth + Sonnet
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        assert!(beta.contains("claude-code"));
        assert!(beta.contains("oauth"));
        
        // API Key + Sonnet
        let beta = get_beta_header(false, "claude-sonnet-4-5");
        assert!(beta.contains("claude-code"));
        assert!(!beta.contains("oauth"));
        
        // OAuth + Haiku
        let beta = get_beta_header(true, "claude-haiku-4-5");
        assert!(beta.contains("oauth"));
        assert!(!beta.contains("claude-code"));
        
        // API Key + Haiku
        let beta = get_beta_header(false, "claude-haiku-4-5");
        assert!(!beta.contains("oauth"));
        assert!(!beta.contains("claude-code"));
    }
    
    #[test]
    fn test_model_id_normalization() {
        // 短名转完整名
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5-20250929"
        );
        assert_eq!(
            normalize_model_id("claude-opus-4-5"),
            "claude-opus-4-5-20251101"
        );
        
        // 已是完整名则不变
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5-20250929"
        );
    }
    
    #[test]
    fn test_model_id_denormalization() {
        // 完整名转短名
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5"
        );
        
        // 已是短名则不变
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5"
        );
    }
    
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
    }
    
    #[test]
    fn test_curve_order() {
        // 验证顺序：X25519, P256, P384
        assert_eq!(DEFAULT_CURVES[0], 0x001d); // X25519
        assert_eq!(DEFAULT_CURVES[1], 0x0017); // P256
        assert_eq!(DEFAULT_CURVES[2], 0x0018); // P384
    }
}
