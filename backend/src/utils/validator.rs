//! 验证工具

use once_cell::sync::Lazy;
use regex::Regex;

/// 邮箱正则
static EMAIL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap());

/// 手机号正则（中国）
static PHONE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^1[3-9]\d{9}$").unwrap());

/// API Key 正则
static API_KEY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_-]+-[a-zA-Z0-9]{32,}$").unwrap());

/// 验证邮箱
pub fn is_valid_email(email: &str) -> bool {
    EMAIL_REGEX.is_match(email)
}

/// 验证手机号
pub fn is_valid_phone(phone: &str) -> bool {
    PHONE_REGEX.is_match(phone)
}

/// 验证 API Key
pub fn is_valid_api_key(key: &str) -> bool {
    API_KEY_REGEX.is_match(key)
}

/// 验证密码强度
pub fn is_strong_password(password: &str) -> bool {
    // 至少 8 字符
    if password.len() < 8 {
        return false;
    }

    // 包含数字
    let has_digit = password.chars().any(|c| c.is_ascii_digit());

    // 包含小写字母
    let has_lowercase = password.chars().any(|c| c.is_lowercase());

    // 包含大写字母
    let has_uppercase = password.chars().any(|c| c.is_uppercase());

    has_digit && has_lowercase && has_uppercase
}

/// 验证用户名
pub fn is_valid_username(username: &str) -> bool {
    // 3-20 字符，只允许字母、数字、下划线
    username.len() >= 3
        && username.len() <= 20
        && username.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// 验证 URL
pub fn is_valid_url(url: &str) -> bool {
    url::Url::parse(url).is_ok()
}

/// 验证 JSON
pub fn is_valid_json(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json).is_ok()
}

/// 验证模型名称
pub fn is_valid_model_name(model: &str) -> bool {
    // 非空，且只允许字母、数字、连字符、点、斜杠
    !model.is_empty()
        && model
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '/' || c == ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name+tag@example.org"));
        assert!(!is_valid_email("invalid-email"));
        assert!(!is_valid_email("@example.com"));
    }

    #[test]
    fn test_is_valid_phone() {
        assert!(is_valid_phone("13812345678"));
        assert!(is_valid_phone("15912345678"));
        assert!(!is_valid_phone("12345678901"));
        assert!(!is_valid_phone("1381234567"));
    }

    #[test]
    fn test_is_strong_password() {
        assert!(is_strong_password("Password123"));
        assert!(is_strong_password("StrongPass1"));
        assert!(!is_strong_password("weak"));
        assert!(!is_strong_password("noDigits"));
        assert!(!is_strong_password("nocaps123"));
    }

    #[test]
    fn test_is_valid_username() {
        assert!(is_valid_username("user123"));
        assert!(is_valid_username("user_name"));
        assert!(!is_valid_username("ab")); // 太短
        assert!(!is_valid_username("user@name")); // 非法字符
    }

    #[test]
    fn test_is_valid_model_name() {
        assert!(is_valid_model_name("gpt-4"));
        assert!(is_valid_model_name("claude-3-opus"));
        assert!(is_valid_model_name("gemini-1.5-pro"));
        assert!(!is_valid_model_name(""));
    }

    #[test]
    fn test_is_valid_json() {
        assert!(is_valid_json(r#"{"key": "value"}"#));
        assert!(is_valid_json(r#"[1, 2, 3]"#));
        assert!(!is_valid_json(r#"invalid json"#));
    }
}
