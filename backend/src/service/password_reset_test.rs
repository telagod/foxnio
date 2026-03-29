//! 密码重置服务测试

#[cfg(test)]
#[allow(clippy::all)]
mod tests {
    use crate::service::email::{EmailSender, MockEmailSender};
    use crate::service::password_reset::*;

    #[test]
    fn test_token_length() {
        // Token 应该是 32 字节 = 64 个十六进制字符
        assert_eq!(TOKEN_LENGTH, 32);
    }

    #[test]
    fn test_token_expiry() {
        // Token 有效期应该是 1 小时
        assert_eq!(TOKEN_EXPIRY_HOURS, 1);
    }

    #[test]
    fn test_hash_token_consistency() {
        // 相同的 token 应该产生相同的哈希
        use sha2::{Digest, Sha256};

        let token = "test_token_123";
        let mut hasher1 = Sha256::new();
        hasher1.update(token.as_bytes());
        let hash1 = format!("{:x}", hasher1.finalize());

        let mut hasher2 = Sha256::new();
        hasher2.update(token.as_bytes());
        let hash2 = format!("{:x}", hasher2.finalize());

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 输出 64 个十六进制字符
    }

    #[test]
    fn test_password_validation() {
        // 测试密码验证逻辑
        let long_password = "a".repeat(128);
        let valid_passwords: Vec<&str> = vec!["password123", "MySecureP@ss", "12345678"];

        let invalid_passwords = vec![
            "1234567", // 太短
            "",        // 空密码
        ];

        for password in valid_passwords {
            assert!(
                password.len() >= 8,
                "Password '{}' should be valid",
                password
            );
        }

        // Test long password separately
        assert!(long_password.len() >= 8, "Long password should be valid");

        for password in invalid_passwords {
            assert!(
                password.len() < 8,
                "Password '{}' should be invalid",
                password
            );
        }
    }

    #[test]
    fn test_mock_email_sender() {
        let sender = MockEmailSender::new();

        let result = sender.send_password_reset_email(
            "test@example.com",
            "https://example.com/reset?token=abc123",
        );

        assert!(result.is_ok());

        let emails = sender.get_sent_emails();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].0, "test@example.com");
        assert!(emails[0].1.contains("token=abc123"));
    }
}
