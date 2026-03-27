//! 用户服务测试

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHasher};

    fn test_password_hash() -> String {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2.hash_password("testpassword".as_bytes(), &salt).unwrap().to_string()
    }

    #[test]
    fn test_password_hashing() {
        let password = "mysecretpassword123";
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap().to_string();
        
        assert!(!hash.is_empty());
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_password_verification() {
        let password = "mysecretpassword123";
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap().to_string();
        
        // 正确密码
        let parsed_hash = password_hash::PasswordHash::new(&hash).unwrap();
        let valid = argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok();
        assert!(valid);
        
        // 错误密码
        let invalid = argon2.verify_password("wrongpassword".as_bytes(), &parsed_hash).is_ok();
        assert!(!invalid);
    }

    #[test]
    fn test_jwt_claims() {
        let claims = Claims {
            sub: "user-123".to_string(),
            email: "test@example.com".to_string(),
            role: "user".to_string(),
            exp: 9999999999,
            iat: 1700000000,
        };
        
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "user");
    }
}
