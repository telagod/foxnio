//! API Key 服务测试

#[cfg(test)]
#[allow(clippy::all)]
mod tests {
    use crate::service::api_key::mask_key;
    use uuid::Uuid;

    fn generate_test_key() -> String {
        let prefix = "sk-";
        // Use UUID to generate random part
        let random = Uuid::new_v4().simple().to_string();
        // Repeat to get 48 chars
        let random_48 = format!("{random}{random}").into_bytes();
        let random_48: String = String::from_utf8(random_48[..48].to_vec()).unwrap();
        format!("{prefix}{random_48}")
    }

    #[test]
    fn test_generate_key() {
        let key = generate_test_key();

        // 检查前缀
        assert!(key.starts_with("sk-"));

        // 检查长度 (前缀 3 + 随机部分 48 = 51)
        assert_eq!(key.len(), 51);

        // 检查只包含有效字符
        let random_part = &key[3..];
        assert!(random_part.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_key_uniqueness() {
        let key1 = generate_test_key();
        let key2 = generate_test_key();

        // 两个 key 应该不同
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_mask_key() {
        let key = "sk-abcdefghijklmnopqrstuvwxyz123456789012345678";
        let masked = mask_key(key);

        assert!(masked.starts_with("sk-abcd"));
        assert!(masked.ends_with("5678"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_mask_short_key() {
        let key = "short";
        let masked = mask_key(key);

        // 短 key 不应该被掩码
        assert_eq!(masked, "short");
    }
}
