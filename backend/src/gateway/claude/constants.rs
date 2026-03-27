//! Claude 渠道常量和配置

/// Beta Header 常量
pub const BETA_OAUTH: &str = "oauth-2025-04-20";
pub const BETA_CLAUDE_CODE: &str = "claude-code-20250219";
pub const BETA_INTERLEAVED_THINKING: &str = "interleaved-thinking-2025-05-14";
pub const BETA_FINE_GRAINED_TOOL_STREAMING: &str = "fine-grained-tool-streaming-2025-05-14";
pub const BETA_TOKEN_COUNTING: &str = "token-counting-2024-11-01";
pub const BETA_CONTEXT_1M: &str = "context-1m-2025-08-07";

/// 默认 Beta Header（OAuth 账号）
pub const DEFAULT_BETA_HEADER: &str = "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

/// API Key 账号 Beta Header（不包含 oauth）
pub const API_KEY_BETA_HEADER: &str = "claude-code-20250219,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

/// Haiku 模型 Beta Header（OAuth）
pub const HAIKU_OAUTH_BETA_HEADER: &str = "oauth-2025-04-20,interleaved-thinking-2025-05-14";

/// Haiku 模型 Beta Header（API Key）
pub const HAIKU_API_KEY_BETA_HEADER: &str = "interleaved-thinking-2025-05-14";

/// 默认请求头（来自 Claude Code 客户端抓包）
pub const DEFAULT_USER_AGENT: &str = "claude-cli/2.1.22 (external, cli)";
pub const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Claude 模型 ID 映射
pub const MODEL_ID_OVERRIDES: &[(&str, &str)] = &[
    ("claude-sonnet-4-5", "claude-sonnet-4-5-20250929"),
    ("claude-opus-4-5", "claude-opus-4-5-20251101"),
    ("claude-haiku-4-5", "claude-haiku-4-5-20251001"),
];

/// 获取 Beta Header
pub fn get_beta_header(is_oauth: bool, model: &str) -> String {
    let is_haiku = model.to_lowercase().contains("haiku");
    
    if is_oauth {
        if is_haiku {
            HAIKU_OAUTH_BETA_HEADER.to_string()
        } else {
            DEFAULT_BETA_HEADER.to_string()
        }
    } else {
        if is_haiku {
            HAIKU_API_KEY_BETA_HEADER.to_string()
        } else {
            API_KEY_BETA_HEADER.to_string()
        }
    }
}

/// 标准化模型 ID（短名转完整名）
pub fn normalize_model_id(id: &str) -> String {
    for (short, full) in MODEL_ID_OVERRIDES {
        if id == *short {
            return full.to_string();
        }
    }
    id.to_string()
}

/// 反标准化模型 ID（完整名转短名）
pub fn denormalize_model_id(id: &str) -> String {
    for (short, full) in MODEL_ID_OVERRIDES {
        if id == *full {
            return short.to_string();
        }
    }
    id.to_string()
}

/// 默认模型列表
pub const DEFAULT_MODELS: &[(&str, &str)] = &[
    ("claude-opus-4-5-20251101", "Claude Opus 4.5"),
    ("claude-opus-4-6", "Claude Opus 4.6"),
    ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
    ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5"),
    ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
];

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_beta_header_oauth() {
        let beta = get_beta_header(true, "claude-sonnet-4-5");
        assert!(beta.contains(BETA_CLAUDE_CODE));
        assert!(beta.contains(BETA_OAUTH));
    }
    
    #[test]
    fn test_get_beta_header_api_key() {
        let beta = get_beta_header(false, "claude-sonnet-4-5");
        assert!(beta.contains(BETA_CLAUDE_CODE));
        assert!(!beta.contains(BETA_OAUTH));
    }
    
    #[test]
    fn test_get_beta_header_haiku_oauth() {
        let beta = get_beta_header(true, "claude-haiku-4-5");
        assert!(beta.contains(BETA_OAUTH));
        assert!(!beta.contains(BETA_CLAUDE_CODE));
    }
    
    #[test]
    fn test_get_beta_header_haiku_api_key() {
        let beta = get_beta_header(false, "claude-haiku-4-5");
        assert!(!beta.contains(BETA_OAUTH));
        assert!(!beta.contains(BETA_CLAUDE_CODE));
    }
    
    #[test]
    fn test_normalize_model_id() {
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5-20250929"
        );
        assert_eq!(
            normalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5-20250929"
        );
    }
    
    #[test]
    fn test_denormalize_model_id() {
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5"
        );
        assert_eq!(
            denormalize_model_id("claude-sonnet-4-5"),
            "claude-sonnet-4-5"
        );
    }
}
