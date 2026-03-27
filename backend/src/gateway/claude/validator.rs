//! Claude Code 客户端验证器
//!
//! 验证请求是否来自真实的 Claude Code CLI

use regex::Regex;
use std::collections::HashMap;

/// Claude Code 官方 System Prompt 模板
pub const CLAUDE_CODE_SYSTEM_PROMPTS: &[&str] = &[
    // Primary
    "You are Claude Code, Anthropic's official CLI for Claude.",
    // Agent SDK
    "You are a Claude agent, built on Anthropic's Claude Agent SDK.",
    // Compact Agent SDK
    "You are Claude Code, Anthropic's official CLI for Claude, running within the Claude Agent SDK.",
    // Explore Agent
    "You are a file search specialist for Claude Code, Anthropic's official CLI for Claude.",
    // Compact (对话摘要)
    "You are a helpful AI assistant tasked with summarizing conversations.",
    // Secondary (长提示词关键部分)
    "You are an interactive CLI tool that helps users",
];

/// System prompt 相似度阈值
pub const SYSTEM_PROMPT_THRESHOLD: f64 = 0.5;

/// Claude Code 验证器
#[derive(Debug, Clone)]
pub struct ClaudeCodeValidator {
    /// User-Agent 匹配正则
    ua_pattern: Regex,
    /// 版本提取正则
    version_pattern: Regex,
}

impl Default for ClaudeCodeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeValidator {
    /// 创建新的验证器
    pub fn new() -> Self {
        Self {
            // 匹配: claude-cli/x.x.x (大小写不敏感)
            ua_pattern: Regex::new(r"(?i)^claude-cli/\d+\.\d+\.\d+").unwrap(),
            // 提取版本号
            version_pattern: Regex::new(r"(?i)^claude-cli/(\d+\.\d+\.\d+)").unwrap(),
        }
    }
    
    /// 验证 User-Agent 是否来自 Claude Code
    pub fn validate_user_agent(&self, ua: &str) -> bool {
        self.ua_pattern.is_match(ua)
    }
    
    /// 从 User-Agent 提取版本号
    pub fn extract_version(&self, ua: &str) -> Option<String> {
        self.version_pattern
            .captures(ua)
            .map(|caps| caps[1].to_string())
    }
    
    /// 比较两个版本号
    /// 返回: -1 (a < b), 0 (a == b), 1 (a > b)
    pub fn compare_versions(a: &str, b: &str) -> i32 {
        let parse_version = |v: &str| -> Vec<u32> {
            v.trim_start_matches('v')
                .split('.')
                .filter_map(|s| s.parse().ok())
                .collect()
        };
        
        let a_parts = parse_version(a);
        let b_parts = parse_version(b);
        
        for i in 0..3 {
            let a_val = a_parts.get(i).unwrap_or(&0);
            let b_val = b_parts.get(i).unwrap_or(&0);
            
            if a_val < b_val {
                return -1;
            }
            if a_val > b_val {
                return 1;
            }
        }
        
        0
    }
    
    /// 检查请求体是否包含 Claude Code System Prompt
    pub fn has_claude_code_system_prompt(&self, body: &serde_json::Value) -> bool {
        let system = body.get("system").and_then(|s| s.as_array());
        
        if let Some(system_entries) = system {
            for entry in system_entries {
                if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                    let score = self.best_similarity_score(text);
                    if score >= SYSTEM_PROMPT_THRESHOLD {
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// 计算文本与所有模板的最佳相似度
    fn best_similarity_score(&self, text: &str) -> f64 {
        let normalized_text = normalize_prompt(text);
        
        CLAUDE_CODE_SYSTEM_PROMPTS
            .iter()
            .map(|template| {
                let normalized_template = normalize_prompt(template);
                dice_coefficient(&normalized_text, &normalized_template)
            })
            .fold(0.0, f64::max)
    }
}

/// 标准化提示词（去除多余空白）
fn normalize_prompt(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 计算 Dice 系数（Sørensen–Dice coefficient）
/// 公式: 2 * |intersection| / (|bigrams(a)| + |bigrams(b)|)
fn dice_coefficient(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    
    if a.len() < 2 || b.len() < 2 {
        return 0.0;
    }
    
    let bigrams_a = get_bigrams(a);
    let bigrams_b = get_bigrams(b);
    
    if bigrams_a.is_empty() || bigrams_b.is_empty() {
        return 0.0;
    }
    
    // 计算交集大小
    let mut intersection = 0;
    for (bigram, count_a) in &bigrams_a {
        if let Some(count_b) = bigrams_b.get(bigram) {
            intersection += count_a.min(count_b);
        }
    }
    
    // 计算总 bigram 数量
    let total_a: usize = bigrams_a.values().sum();
    let total_b: usize = bigrams_b.values().sum();
    
    (2 * intersection) as f64 / (total_a + total_b) as f64
}

/// 获取字符串的所有 bigrams
fn get_bigrams(s: &str) -> HashMap<String, usize> {
    let mut bigrams = HashMap::new();
    let chars: Vec<char> = s.to_lowercase().chars().collect();
    
    for i in 0..chars.len().saturating_sub(1) {
        let bigram: String = chars[i..i+2].iter().collect();
        *bigrams.entry(bigram).or_insert(0) += 1;
    }
    
    bigrams
}

/// 解析 metadata.user_id
/// 格式: device_type:device_id:session_id (例如 "cli:abc123:def456")
pub fn parse_metadata_user_id(user_id: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = user_id.split(':').collect();
    
    if parts.len() != 3 {
        return None;
    }
    
    // 验证各部分非空
    if parts[0].is_empty() || parts[1].is_empty() || parts[2].is_empty() {
        return None;
    }
    
    Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_user_agent() {
        let validator = ClaudeCodeValidator::new();
        
        // 有效 User-Agent
        assert!(validator.validate_user_agent("claude-cli/1.0.0"));
        assert!(validator.validate_user_agent("claude-cli/2.1.22 (darwin; arm64)"));
        assert!(validator.validate_user_agent("Claude-CLI/3.10.5 (linux; x86_64)"));
        
        // 无效 User-Agent
        assert!(!validator.validate_user_agent("curl/8.0.0"));
        assert!(!validator.validate_user_agent("Mozilla/5.0"));
        assert!(!validator.validate_user_agent("claude-cli/"));
    }
    
    #[test]
    fn test_extract_version() {
        let validator = ClaudeCodeValidator::new();
        
        assert_eq!(validator.extract_version("claude-cli/2.1.22"), Some("2.1.22".to_string()));
        assert_eq!(validator.extract_version("claude-cli/1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(validator.extract_version("curl/8.0.0"), None);
        assert_eq!(validator.extract_version(""), None);
    }
    
    #[test]
    fn test_compare_versions() {
        assert_eq!(ClaudeCodeValidator::compare_versions("2.1.0", "2.1.0"), 0);
        assert_eq!(ClaudeCodeValidator::compare_versions("2.1.1", "2.1.0"), 1);
        assert_eq!(ClaudeCodeValidator::compare_versions("2.0.0", "2.1.0"), -1);
        assert_eq!(ClaudeCodeValidator::compare_versions("3.0.0", "2.99.99"), 1);
        assert_eq!(ClaudeCodeValidator::compare_versions("v2.1.0", "2.1.0"), 0);
    }
    
    #[test]
    fn test_dice_coefficient() {
        // 相同字符串
        assert_eq!(dice_coefficient("hello", "hello"), 1.0);
        
        // 完全不同
        assert_eq!(dice_coefficient("abc", "xyz"), 0.0);
        
        // 部分相似
        let score = dice_coefficient("hello world", "hello there");
        assert!(score > 0.0 && score < 1.0);
    }
    
    #[test]
    fn test_normalize_prompt() {
        assert_eq!(
            normalize_prompt("  hello   world  "),
            "hello world"
        );
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
    }
    
    #[test]
    fn test_has_claude_code_system_prompt() {
        let validator = ClaudeCodeValidator::new();
        
        // 包含 Claude Code system prompt
        let body = serde_json::json!({
            "system": [
                {"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}
            ]
        });
        assert!(validator.has_claude_code_system_prompt(&body));
        
        // 不包含
        let body = serde_json::json!({
            "system": [
                {"type": "text", "text": "You are a helpful assistant."}
            ]
        });
        assert!(!validator.has_claude_code_system_prompt(&body));
    }
}
