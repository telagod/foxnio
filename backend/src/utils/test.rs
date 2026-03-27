//! 工具函数测试

#[cfg(test)]
mod tests {
    use crate::utils::{crypto, id};
    
    #[test]
    fn test_hash_password() {
        let password = "test_password_123";
        let hash = crypto::hash_password(password);
        
        assert!(hash.is_ok());
        let hash = hash.unwrap();
        
        // 哈希应该不为空且不等于原密码
        assert!(!hash.is_empty());
        assert_ne!(hash, password);
    }
    
    #[test]
    fn test_verify_password() {
        let password = "test_password_123";
        let hash = crypto::hash_password(password).unwrap();
        
        // 正确密码应该验证通过
        assert!(crypto::verify_password(password, &hash));
        
        // 错误密码应该验证失败
        assert!(!crypto::verify_password("wrong_password", &hash));
    }
    
    #[test]
    fn test_generate_id() {
        let id1 = id::generate_id();
        let id2 = id::generate_id();
        
        // 两个 ID 应该不同
        assert_ne!(id1, id2);
        
        // ID 长度应该固定
        assert!(!id1.is_empty());
    }
    
    #[test]
    fn test_generate_short_id() {
        let id = id::generate_short_id();
        
        // 短 ID 应该是 8 字符
        assert_eq!(id.len(), 8);
    }
    
    #[test]
    fn test_mask_string() {
        let original = "sk-test-1234567890abcdef";
        let masked = id::mask_string(original, 8);
        
        // 掩码后应该隐藏中间部分
        assert!(masked.starts_with("sk-test-"));
        assert!(masked.ends_with("..."));
    }
    
    #[test]
    fn test_generate_api_key() {
        let key = id::generate_api_key("foxnio");
        
        // API Key 应该有前缀
        assert!(key.starts_with("foxnio-"));
        
        // API Key 长度应该足够
        assert!(key.len() > 20);
    }
}
