//! Claude 请求头构建器

#![allow(dead_code)]
use reqwest::header::HeaderMap;

/// Claude Code 请求头配置
#[derive(Debug, Clone)]
pub struct ClaudeHeaders {
    /// User-Agent
    pub user_agent: String,
    /// X-Stainless-Lang
    pub stainless_lang: String,
    /// X-Stainless-Package-Version
    pub stainless_package_version: String,
    /// X-Stainless-OS
    pub stainless_os: String,
    /// X-Stainless-Arch
    pub stainless_arch: String,
    /// X-Stainless-Runtime
    pub stainless_runtime: String,
    /// X-Stainless-Runtime-Version
    pub stainless_runtime_version: String,
    /// X-App
    pub x_app: String,
    /// anthropic-dangerous-direct-browser-access
    pub anthropic_dangerous_direct_browser_access: bool,
    /// anthropic-version
    pub anthropic_version: String,
}

impl Default for ClaudeHeaders {
    fn default() -> Self {
        Self {
            user_agent: "claude-cli/2.1.22 (external, cli)".to_string(),
            stainless_lang: "js".to_string(),
            stainless_package_version: "0.70.0".to_string(),
            stainless_os: "Linux".to_string(),
            stainless_arch: "arm64".to_string(),
            stainless_runtime: "node".to_string(),
            stainless_runtime_version: "v24.13.0".to_string(),
            x_app: "cli".to_string(),
            anthropic_dangerous_direct_browser_access: true,
            anthropic_version: "2023-06-01".to_string(),
        }
    }
}

impl ClaudeHeaders {
    /// 创建新的请求头配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 User-Agent
    pub fn user_agent(mut self, ua: &str) -> Self {
        self.user_agent = ua.to_string();
        self
    }

    /// 设置 OS
    pub fn os(mut self, os: &str) -> Self {
        self.stainless_os = os.to_string();
        self
    }

    /// 设置 Arch
    pub fn arch(mut self, arch: &str) -> Self {
        self.stainless_arch = arch.to_string();
        self
    }

    /// 构建请求头（按正确顺序）
    ///
    /// 顺序基于真实 Claude Code 客户端抓包
    pub fn build(&self, auth_token: &str, beta: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // 注意：HeaderMap 不能保证顺序，但我们需要尽量保持
        // 这里按照抓包顺序设置

        // 1. Accept
        headers.insert("Accept", "application/json".parse().unwrap());

        // 2. X-Stainless-Retry-Count
        headers.insert("X-Stainless-Retry-Count", "0".parse().unwrap());

        // 3. X-Stainless-Timeout
        headers.insert("X-Stainless-Timeout", "600".parse().unwrap());

        // 4. X-Stainless-Lang
        headers.insert("X-Stainless-Lang", self.stainless_lang.parse().unwrap());

        // 5. X-Stainless-Package-Version
        headers.insert(
            "X-Stainless-Package-Version",
            self.stainless_package_version.parse().unwrap(),
        );

        // 6. X-Stainless-OS (注意大小写：不是 X-Stainless-Os)
        headers.insert("X-Stainless-OS", self.stainless_os.parse().unwrap());

        // 7. X-Stainless-Arch
        headers.insert("X-Stainless-Arch", self.stainless_arch.parse().unwrap());

        // 8. X-Stainless-Runtime
        headers.insert(
            "X-Stainless-Runtime",
            self.stainless_runtime.parse().unwrap(),
        );

        // 9. X-Stainless-Runtime-Version
        headers.insert(
            "X-Stainless-Runtime-Version",
            self.stainless_runtime_version.parse().unwrap(),
        );

        // 10. anthropic-dangerous-direct-browser-access (全小写)
        headers.insert(
            "anthropic-dangerous-direct-browser-access",
            "true".parse().unwrap(),
        );

        // 11. anthropic-version (全小写)
        headers.insert("anthropic-version", self.anthropic_version.parse().unwrap());

        // 12. authorization
        headers.insert(
            "authorization",
            format!("Bearer {auth_token}").parse().unwrap(),
        );

        // 13. x-app (全小写，不是 X-App)
        headers.insert("x-app", self.x_app.parse().unwrap());

        // 14. User-Agent
        headers.insert("User-Agent", self.user_agent.parse().unwrap());

        // 15. content-type (全小写)
        headers.insert("content-type", "application/json".parse().unwrap());

        // 16. anthropic-beta (全小写)
        headers.insert("anthropic-beta", beta.parse().unwrap());

        // 17. accept-language
        headers.insert("accept-language", "*".parse().unwrap());

        // 18. sec-fetch-mode
        headers.insert("sec-fetch-mode", "cors".parse().unwrap());

        // 19. accept-encoding
        headers.insert("accept-encoding", "gzip, deflate".parse().unwrap());

        headers
    }

    /// 构建请求头为 Vec（保证顺序）
    pub fn build_ordered(&self, auth_token: &str, beta: &str) -> Vec<(&'static str, String)> {
        vec![
            ("Accept", "application/json".to_string()),
            ("X-Stainless-Retry-Count", "0".to_string()),
            ("X-Stainless-Timeout", "600".to_string()),
            ("X-Stainless-Lang", self.stainless_lang.clone()),
            (
                "X-Stainless-Package-Version",
                self.stainless_package_version.clone(),
            ),
            ("X-Stainless-OS", self.stainless_os.clone()),
            ("X-Stainless-Arch", self.stainless_arch.clone()),
            ("X-Stainless-Runtime", self.stainless_runtime.clone()),
            (
                "X-Stainless-Runtime-Version",
                self.stainless_runtime_version.clone(),
            ),
            (
                "anthropic-dangerous-direct-browser-access",
                "true".to_string(),
            ),
            ("anthropic-version", self.anthropic_version.clone()),
            ("authorization", format!("Bearer {auth_token}")),
            ("x-app", self.x_app.clone()),
            ("User-Agent", self.user_agent.clone()),
            ("content-type", "application/json".to_string()),
            ("anthropic-beta", beta.to_string()),
            ("accept-language", "*".to_string()),
            ("sec-fetch-mode", "cors".to_string()),
            ("accept-encoding", "gzip, deflate".to_string()),
        ]
    }
}

/// Header 大小写映射（用于保持原始大小写）
pub fn header_wire_casing(key: &str) -> &str {
    let key_lower = key.to_lowercase();

    // 特殊处理的大小写
    match key_lower.as_str() {
        "x-stainless-os" => "X-Stainless-OS",
        "x-stainless-lang" => "X-Stainless-Lang",
        "x-stainless-package-version" => "X-Stainless-Package-Version",
        "x-stainless-arch" => "X-Stainless-Arch",
        "x-stainless-runtime" => "X-Stainless-Runtime",
        "x-stainless-runtime-version" => "X-Stainless-Runtime-Version",
        "x-stainless-retry-count" => "X-Stainless-Retry-Count",
        "x-stainless-timeout" => "X-Stainless-Timeout",
        "x-app" => "x-app",
        "anthropic-beta" => "anthropic-beta",
        "anthropic-version" => "anthropic-version",
        "anthropic-dangerous-direct-browser-access" => "anthropic-dangerous-direct-browser-access",
        "content-type" => "content-type",
        "accept-language" => "accept-language",
        "sec-fetch-mode" => "sec-fetch-mode",
        "accept-encoding" => "accept-encoding",
        "authorization" => "authorization",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_headers_default() {
        let headers = ClaudeHeaders::default();

        assert_eq!(headers.user_agent, "claude-cli/2.1.22 (external, cli)");
        assert_eq!(headers.stainless_lang, "js");
        assert_eq!(headers.stainless_os, "Linux");
        assert_eq!(headers.stainless_arch, "arm64");
    }

    #[test]
    fn test_claude_headers_build() {
        let headers = ClaudeHeaders::default();
        let map = headers.build("test-token", "test-beta");

        assert!(map.contains_key("authorization"));
        assert!(map.contains_key("x-app"));
        assert!(map.contains_key("anthropic-beta"));
        assert!(map.contains_key("User-Agent"));
    }

    #[test]
    fn test_claude_headers_build_ordered() {
        let headers = ClaudeHeaders::default();
        let ordered = headers.build_ordered("test-token", "test-beta");

        assert_eq!(ordered.len(), 19);

        // 验证第一个是 Accept
        assert_eq!(ordered[0].0, "Accept");

        // 验证第 6 个是 X-Stainless-OS（不是 X-Stainless-Os）
        assert_eq!(ordered[5].0, "X-Stainless-OS");

        // 验证 x-app 是小写
        let x_app = ordered.iter().find(|(k, _)| *k == "x-app");
        assert!(x_app.is_some());
    }

    #[test]
    fn test_header_wire_casing() {
        assert_eq!(header_wire_casing("X-Stainless-Os"), "X-Stainless-OS");
        assert_eq!(header_wire_casing("x-app"), "x-app");
        assert_eq!(header_wire_casing("X-App"), "x-app");
        assert_eq!(header_wire_casing("Anthropic-Beta"), "anthropic-beta");
    }
}
