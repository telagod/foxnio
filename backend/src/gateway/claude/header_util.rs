//! Header 工具函数
//!
//! 处理 HTTP Header 的大小写和顺序

use std::collections::HashMap;

/// Header 大小写映射（真实 Claude CLI 抓包）
pub fn header_wire_casing(key: &str) -> &str {
    let key_lower = key.to_lowercase();
    
    match key_lower.as_str() {
        // Title case
        "accept" => "Accept",
        "user-agent" => "User-Agent",
        
        // X-Stainless-* 保持 SDK 原始大小写
        "x-stainless-retry-count" => "X-Stainless-Retry-Count",
        "x-stainless-timeout" => "X-Stainless-Timeout",
        "x-stainless-lang" => "X-Stainless-Lang",
        "x-stainless-package-version" => "X-Stainless-Package-Version",
        "x-stainless-os" => "X-Stainless-OS",
        "x-stainless-arch" => "X-Stainless-Arch",
        "x-stainless-runtime" => "X-Stainless-Runtime",
        "x-stainless-runtime-version" => "X-Stainless-Runtime-Version",
        "x-stainless-helper-method" => "x-stainless-helper-method",
        
        // Anthropic SDK 自身设置的 header，全小写
        "anthropic-dangerous-direct-browser-access" => "anthropic-dangerous-direct-browser-access",
        "anthropic-version" => "anthropic-version",
        "anthropic-beta" => "anthropic-beta",
        "x-app" => "x-app",
        "content-type" => "content-type",
        "accept-language" => "accept-language",
        "sec-fetch-mode" => "sec-fetch-mode",
        "accept-encoding" => "accept-encoding",
        "authorization" => "authorization",
        
        _ => key,
    }
}

/// Header 发送顺序（真实 Claude CLI 抓包）
pub const HEADER_WIRE_ORDER: &[&str] = &[
    "Accept",
    "X-Stainless-Retry-Count",
    "X-Stainless-Timeout",
    "X-Stainless-Lang",
    "X-Stainless-Package-Version",
    "X-Stainless-OS",
    "X-Stainless-Arch",
    "X-Stainless-Runtime",
    "X-Stainless-Runtime-Version",
    "anthropic-dangerous-direct-browser-access",
    "anthropic-version",
    "authorization",
    "x-app",
    "User-Agent",
    "content-type",
    "anthropic-beta",
    "accept-language",
    "sec-fetch-mode",
    "accept-encoding",
];

/// 按照真实 Claude CLI 的 header 顺序排序
pub fn sort_headers_by_wire_order(headers: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    
    // 先按 wire order 输出
    for wire_key in HEADER_WIRE_ORDER {
        let wire_lower = wire_key.to_lowercase();
        
        // 查找 header（忽略大小写）
        for (k, v) in headers {
            if k.to_lowercase() == wire_lower && !seen.contains(&wire_lower) {
                result.push((header_wire_casing(k).to_string(), v.clone()));
                seen.insert(wire_lower);
                break;
            }
        }
    }
    
    // 再追加不在 wire order 中的 header
    for (k, v) in headers {
        let k_lower = k.to_lowercase();
        if !seen.contains(&k_lower) {
            result.push((header_wire_casing(k).to_string(), v.clone()));
            seen.insert(k_lower);
        }
    }
    
    result
}

/// 构建完整的 Claude 请求头（按顺序）
pub fn build_claude_headers_ordered(
    auth_token: &str,
    beta: &str,
    user_agent: &str,
    version: &str,
) -> Vec<(String, String)> {
    vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("X-Stainless-Retry-Count".to_string(), "0".to_string()),
        ("X-Stainless-Timeout".to_string(), "600".to_string()),
        ("X-Stainless-Lang".to_string(), "js".to_string()),
        ("X-Stainless-Package-Version".to_string(), "0.70.0".to_string()),
        ("X-Stainless-OS".to_string(), "Linux".to_string()),
        ("X-Stainless-Arch".to_string(), "arm64".to_string()),
        ("X-Stainless-Runtime".to_string(), "node".to_string()),
        ("X-Stainless-Runtime-Version".to_string(), "v24.13.0".to_string()),
        ("anthropic-dangerous-direct-browser-access".to_string(), "true".to_string()),
        ("anthropic-version".to_string(), version.to_string()),
        ("authorization".to_string(), format!("Bearer {}", auth_token)),
        ("x-app".to_string(), "cli".to_string()),
        ("User-Agent".to_string(), user_agent.to_string()),
        ("content-type".to_string(), "application/json".to_string()),
        ("anthropic-beta".to_string(), beta.to_string()),
        ("accept-language".to_string(), "*".to_string()),
        ("sec-fetch-mode".to_string(), "cors".to_string()),
        ("accept-encoding".to_string(), "gzip, deflate".to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_header_wire_casing() {
        // X-Stainless-OS（注意：不是 X-Stainless-Os）
        assert_eq!(header_wire_casing("X-Stainless-Os"), "X-Stainless-OS");
        assert_eq!(header_wire_casing("x-stainless-os"), "X-Stainless-OS");
        
        // 全小写 headers
        assert_eq!(header_wire_casing("X-App"), "x-app");
        assert_eq!(header_wire_casing("x-app"), "x-app");
        assert_eq!(header_wire_casing("Anthropic-Beta"), "anthropic-beta");
        
        // 其他保持不变
        assert_eq!(header_wire_casing("Custom-Header"), "Custom-Header");
    }
    
    #[test]
    fn test_sort_headers_by_wire_order() {
        let mut headers = HashMap::new();
        headers.insert("anthropic-beta".to_string(), "test-beta".to_string());
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("x-app".to_string(), "cli".to_string());
        
        let sorted = sort_headers_by_wire_order(&headers);
        
        // Accept 应该在第一位
        assert_eq!(sorted[0].0, "Accept");
        
        // x-app 应该在 anthropic-beta 前面
        let x_app_pos = sorted.iter().position(|(k, _)| k == "x-app").unwrap();
        let beta_pos = sorted.iter().position(|(k, _)| k == "anthropic-beta").unwrap();
        assert!(x_app_pos < beta_pos);
    }
    
    #[test]
    fn test_build_claude_headers_ordered() {
        let headers = build_claude_headers_ordered(
            "test-token",
            "test-beta",
            "claude-cli/2.1.22",
            "2023-06-01"
        );
        
        assert_eq!(headers.len(), 19);
        
        // 验证顺序
        assert_eq!(headers[0].0, "Accept");
        assert_eq!(headers[5].0, "X-Stainless-OS");
        assert_eq!(headers[12].0, "x-app");
        
        // 验证值
        assert!(headers[11].1.starts_with("Bearer"));
        assert_eq!(headers[15].1, "test-beta");
    }
}
