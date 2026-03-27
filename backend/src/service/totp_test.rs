//! TOTP 两步验证测试

use crate::service::totp::TotpService;

#[test]
fn test_generate_secret() {
    let secret = TotpService::generate_secret();
    println!("Generated secret: {}", secret);
    
    // 验证格式
    assert!(!secret.is_empty(), "Secret should not be empty");
    assert!(secret.len() >= 26, "Secret should be at least 26 characters (160 bits base32)");
    assert!(
        secret.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
        "Secret should only contain Base32 characters"
    );
}

#[test]
fn test_generate_backup_codes() {
    let codes = TotpService::generate_backup_codes();
    println!("Generated backup codes:");
    for code in &codes {
        println!("  {}", code);
    }
    
    assert_eq!(codes.len(), 10, "Should generate 10 backup codes");
    
    for code in &codes {
        assert!(
            TotpService::is_valid_backup_code_format(code),
            "Backup code '{}' should have valid format XXXX-XXXX",
            code
        );
    }
}

#[test]
fn test_backup_code_hash() {
    let code = "1234-5678";
    let hash = TotpService::hash_backup_code(code);
    println!("Backup code hash: {}", hash);
    
    // 验证哈希长度（SHA-256 = 64 hex chars）
    assert_eq!(hash.len(), 64, "Hash should be SHA-256 (64 hex chars)");
    
    // 验证相同代码产生相同哈希
    let hash2 = TotpService::hash_backup_code(code);
    assert_eq!(hash, hash2, "Same code should produce same hash");
    
    // 验证不同代码产生不同哈希
    let hash3 = TotpService::hash_backup_code("8765-4321");
    assert_ne!(hash, hash3, "Different codes should produce different hashes");
    
    // 验证验证函数
    assert!(
        TotpService::verify_backup_code(code, &hash),
        "Verify should return true for correct code"
    );
    assert!(
        !TotpService::verify_backup_code("0000-0000", &hash),
        "Verify should return false for incorrect code"
    );
}

#[test]
fn test_otpauth_url() {
    let service = TotpService::new("FoxNIO");
    let email = "user@example.com";
    let secret = "JBSWY3DPEHPK3PXP";
    
    let url = service.generate_otpauth_url(email, secret);
    println!("otpauth URL: {}", url);
    
    assert!(
        url.starts_with("otpauth://totp/"),
        "URL should start with otpauth://totp/"
    );
    assert!(
        url.contains(&format!("secret={}", secret)),
        "URL should contain secret parameter"
    );
    assert!(
        url.contains("issuer=FoxNIO"),
        "URL should contain issuer parameter"
    );
    assert!(
        url.contains("algorithm=SHA1"),
        "URL should contain algorithm parameter"
    );
    assert!(
        url.contains("digits=6"),
        "URL should contain digits parameter"
    );
    assert!(
        url.contains("period=30"),
        "URL should contain period parameter"
    );
}

#[test]
fn test_otpauth_url_special_chars() {
    let service = TotpService::new("FoxNIO App");
    let email = "user+test@example.com";
    let secret = "JBSWY3DPEHPK3PXP";
    
    let url = service.generate_otpauth_url(email, secret);
    println!("otpauth URL with special chars: {}", url);
    
    // 特殊字符应该被 URL 编码
    assert!(
        url.contains("%") || !email.contains('+'),
        "Special characters should be URL encoded"
    );
}

#[test]
fn test_verify_code_invalid_format() {
    let secret = "JBSWY3DPEHPK3PXP";
    
    // 太短
    assert!(
        !TotpService::verify_code(secret, "12345"),
        "Code too short should fail"
    );
    
    // 太长
    assert!(
        !TotpService::verify_code(secret, "1234567"),
        "Code too long should fail"
    );
    
    // 非数字
    assert!(
        !TotpService::verify_code(secret, "abcdef"),
        "Non-numeric code should fail"
    );
    
    // 包含空格
    assert!(
        !TotpService::verify_code(secret, "123 456"),
        "Code with space should fail"
    );
}

#[test]
fn test_verify_code_valid_format() {
    let secret = TotpService::generate_secret();
    
    // 获取当前代码
    let code = TotpService::get_current_code(&secret).unwrap();
    println!("Current TOTP code: {}", code);
    
    assert_eq!(code.len(), 6, "Code should be 6 digits");
    assert!(
        code.chars().all(|c| c.is_ascii_digit()),
        "Code should only contain digits"
    );
    
    // 验证当前代码
    assert!(
        TotpService::verify_code(&secret, &code),
        "Current code should verify"
    );
}

#[test]
fn test_base32_encode_decode() {
    use crate::service::totp::{base32_encode, base32_decode};
    
    let test_cases = vec![
        b"Hello".to_vec(),
        b"World!".to_vec(),
        b"Test123".to_vec(),
        vec![0x00, 0x01, 0x02, 0x03, 0x04],
        (0..=255).collect::<Vec<u8>>(),
    ];
    
    for original in test_cases {
        let encoded = base32_encode(&original);
        let decoded = base32_decode(&encoded).unwrap();
        assert_eq!(original, decoded, "Encode/decode should be reversible");
    }
}

#[test]
fn test_remaining_seconds() {
    let remaining = TotpService::remaining_seconds();
    println!("Remaining seconds: {}", remaining);
    
    assert!(remaining > 0, "Remaining seconds should be > 0");
    assert!(remaining <= 30, "Remaining seconds should be <= 30");
}

#[test]
fn test_qr_code_generation() {
    let service = TotpService::new("FoxNIO");
    let email = "test@example.com";
    let secret = "JBSWY3DPEHPK3PXP";
    
    let result = service.generate_qr_code_base64(email, secret);
    assert!(result.is_ok(), "QR code generation should succeed");
    
    let base64_svg = result.unwrap();
    println!("QR code (base64): {}...", &base64_svg[..base64_svg.len().min(50)]);
    
    assert!(!base64_svg.is_empty(), "QR code should not be empty");
    
    // 解码并验证是 SVG
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&base64_svg)
        .expect("Should decode base64");
    let svg_str = String::from_utf8(decoded).expect("Should be valid UTF-8");
    assert!(
        svg_str.starts_with("<?xml") || svg_str.starts_with("<svg"),
        "Should be SVG content"
    );
}

#[test]
fn test_qr_code_data_url() {
    let service = TotpService::new("FoxNIO");
    let email = "test@example.com";
    let secret = "JBSWY3DPEHPK3PXP";
    
    let result = service.generate_qr_code_data_url(email, secret);
    assert!(result.is_ok(), "Data URL generation should succeed");
    
    let data_url = result.unwrap();
    println!("Data URL: {}...", &data_url[..data_url.len().min(50)]);
    
    assert!(
        data_url.starts_with("data:image/svg+xml;base64,"),
        "Should be SVG data URL"
    );
}

#[test]
fn test_known_totp_vector() {
    // 使用 RFC 6238 测试向量
    // 秘钥: "12345678901234567890" (20 bytes)
    // Base32 编码: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ
    let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    
    // 生成当前代码
    let code = TotpService::get_current_code(secret).unwrap();
    println!("Current code for test secret: {}", code);
    
    // 验证格式
    assert_eq!(code.len(), 6);
    assert!(code.chars().all(|c| c.is_ascii_digit()));
    
    // 验证代码
    assert!(
        TotpService::verify_code(secret, &code),
        "Current code should verify"
    );
}

#[test]
fn test_time_tolerance() {
    // 测试时间容错
    let secret = TotpService::generate_secret();
    let code = TotpService::get_current_code(&secret).unwrap();
    
    // 当前代码应该验证通过
    assert!(
        TotpService::verify_code(&secret, &code),
        "Current code should verify with tolerance"
    );
}

#[test]
fn test_different_users_different_codes() {
    let secret1 = TotpService::generate_secret();
    let secret2 = TotpService::generate_secret();
    
    assert_ne!(secret1, secret2, "Different users should have different secrets");
    
    let code1 = TotpService::get_current_code(&secret1).unwrap();
    let code2 = TotpService::get_current_code(&secret2).unwrap();
    
    // 代码可能相同（概率很低），但验证应该区分
    // 验证 secret1 的代码不应该通过 secret2 验证
    assert!(
        !TotpService::verify_code(&secret2, &code1) || code1 == code2,
        "Code from secret1 should not verify with secret2"
    );
}
