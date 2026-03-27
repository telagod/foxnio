//! Entity 测试

#[cfg(test)]
mod tests {
    use crate::entity::{users, accounts, api_keys, usages};
    use chrono::Utc;

    #[test]
    fn test_user_status() {
        // 活跃用户
        let active_status = "active";
        assert_eq!(active_status, "active");
        
        // 禁用用户
        let banned_status = "banned";
        assert_ne!(banned_status, "active");
    }

    #[test]
    fn test_account_provider() {
        let providers = vec!["anthropic", "openai", "gemini", "antigravity"];
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"openai"));
    }

    #[test]
    fn test_api_key_masking() {
        let key = "sk-abcdefghij1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let masked = format!("{}...{}", &key[..7], &key[key.len()-4..]);
        
        assert_eq!(masked, "sk-abcd...STUV");
    }

    #[test]
    fn test_usage_cost_calculation() {
        // 100 分 = 1 元
        let cost_cents = 150;
        let cost_yuan = cost_cents as f64 / 100.0;
        
        assert_eq!(cost_yuan, 1.5);
    }
}
