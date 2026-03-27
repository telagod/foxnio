//! TOTP 两步验证服务
//!
//! 兼容 Google Authenticator / Authy / Microsoft Authenticator

use anyhow::{Result, bail};
use base64::Engine;
use qrcode::QrCode;
use rand::RngCore;
use totp_lite::{totp_custom, Sha1};

/// TOTP 配置
const TOTP_DIGITS: u32 = 6;
const TOTP_TOLERANCE: u32 = 1; // 允许前后各1个时间窗口
const TOTP_PERIOD: u64 = 30; // 30秒时间窗口

/// TOTP 服务
#[derive(Debug, Clone)]
pub struct TotpService {
    issuer: String,
}

impl TotpService {
    /// 创建新的 TOTP 服务实例
    pub fn new(issuer: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
        }
    }

    /// 生成随机密钥 (Base32 编码)
    /// 返回兼容 Google Authenticator 的 Base32 密钥
    pub fn generate_secret() -> String {
        // 生成 20 字节的随机密钥（160位，推荐长度）
        let mut bytes = [0u8; 20];
        rand::thread_rng().fill_bytes(&mut bytes);
        
        // Base32 编码（Google Authenticator 标准）
        base32_encode(&bytes)
    }

    /// 生成备用码
    /// 返回 10 个 8 位数字的备用码
    pub fn generate_backup_codes() -> Vec<String> {
        let mut codes = Vec::with_capacity(10);
        let mut rng = rand::thread_rng();
        
        for _ in 0..10 {
            let code1 = rng.next_u32() % 10000;
            let code2 = rng.next_u32() % 10000;
            codes.push(format!("{:04}-{:04}", code1, code2));
        }
        
        codes
    }

    /// 验证备用码格式
    pub fn is_valid_backup_code_format(code: &str) -> bool {
        let parts: Vec<&str> = code.split('-').collect();
        if parts.len() != 2 {
            return false;
        }
        parts[0].len() == 4 && parts[1].len() == 4 
            && parts[0].chars().all(|c| c.is_ascii_digit())
            && parts[1].chars().all(|c| c.is_ascii_digit())
    }

    /// 生成 otpauth:// URL（用于 QR 码）
    /// 格式: otpauth://totp/ISSUER:EMAIL?secret=SECRET&issuer=ISSUER&algorithm=SHA1&digits=6&period=30
    pub fn generate_otpauth_url(&self, email: &str, secret: &str) -> String {
        let issuer_encoded = url_encode(&self.issuer);
        let email_encoded = url_encode(email);
        format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits={}&period={}",
            issuer_encoded,
            email_encoded,
            secret,
            issuer_encoded,
            TOTP_DIGITS,
            TOTP_PERIOD,
        )
    }

    /// 生成 QR 码（返回 Base64 编码的 SVG）
    pub fn generate_qr_code_base64(&self, email: &str, secret: &str) -> Result<String> {
        let otpauth_url = self.generate_otpauth_url(email, secret);
        
        // 生成 QR 码
        let code = QrCode::new(otpauth_url.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to generate QR code: {}", e))?;
        
        // 转换为 SVG 字符串
        let svg = code.render::<qrcode::render::svg::Color>().build();
        
        // Base64 编码
        Ok(base64::engine::general_purpose::STANDARD.encode(svg.as_bytes()))
    }

    /// 生成 QR 码 URL（Data URL 格式）
    pub fn generate_qr_code_data_url(&self, email: &str, secret: &str) -> Result<String> {
        let base64_svg = self.generate_qr_code_base64(email, secret)?;
        Ok(format!("data:image/svg+xml;base64,{}", base64_svg))
    }

    /// 验证 TOTP 代码
    /// 使用时间容错机制，允许前后 1 个时间窗口
    pub fn verify_code(secret: &str, code: &str) -> bool {
        // 清理代码（移除空格）
        let code = code.trim().replace(' ', "");
        
        // 验证格式
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }

        // 解码密钥
        let secret_bytes = match base32_decode(secret) {
            Some(bytes) => bytes,
            None => return false,
        };

        // 获取当前时间戳
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 当前时间窗口
        let current_window = now / TOTP_PERIOD;

        // 检查当前窗口及前后各一个窗口（容错）
        for offset in 0..=TOTP_TOLERANCE {
            for sign in [-1i64, 0, 1] {
                let window = current_window as i64 + sign * offset as i64;
                if window < 0 {
                    continue;
                }
                
                let expected_code = totp_custom::<Sha1>(
                    TOTP_PERIOD,
                    TOTP_DIGITS,
                    &secret_bytes,
                    window as u64,
                );
                
                if expected_code == code {
                    return true;
                }
            }
        }

        false
    }

    /// 获取当前 TOTP 代码（用于测试）
    pub fn get_current_code(secret: &str) -> Result<String> {
        let secret_bytes = base32_decode(secret)
            .ok_or_else(|| anyhow::anyhow!("Invalid secret format"))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let code = totp_custom::<Sha1>(TOTP_PERIOD, TOTP_DIGITS, &secret_bytes, now / TOTP_PERIOD);
        Ok(code)
    }

    /// 计算当前时间窗口剩余秒数
    pub fn remaining_seconds() -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        TOTP_PERIOD - (now % TOTP_PERIOD)
    }

    /// 哈希备用码（用于存储）
    pub fn hash_backup_code(code: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 验证备用码（对比哈希）
    pub fn verify_backup_code(code: &str, hash: &str) -> bool {
        Self::hash_backup_code(code) == hash
    }
}

// ============================================================================
// Base32 编码/解码（Google Authenticator 兼容）
// ============================================================================

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Base32 编码
fn base32_encode(data: &[u8]) -> String {
    let mut result = String::new();
    let mut bits = 0u32;
    let mut bits_count = 0;

    for &byte in data {
        bits = (bits << 8) | (byte as u32);
        bits_count += 8;

        while bits_count >= 5 {
            bits_count -= 5;
            let index = ((bits >> bits_count) & 0x1F) as usize;
            result.push(BASE32_ALPHABET[index] as char);
        }
    }

    if bits_count > 0 {
        let index = ((bits << (5 - bits_count)) & 0x1F) as usize;
        result.push(BASE32_ALPHABET[index] as char);
    }

    result
}

/// Base32 解码
fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.to_uppercase().replace('=', "");
    let mut result = Vec::new();
    let mut bits = 0u32;
    let mut bits_count = 0;

    for c in s.chars() {
        let val = if c >= 'A' && c <= 'Z' {
            (c as u8) - b'A'
        } else if c >= '2' && c <= '7' {
            (c as u8) - b'2' + 26
        } else {
            return None;
        };

        bits = (bits << 5) | (val as u32);
        bits_count += 5;

        while bits_count >= 8 {
            bits_count -= 8;
            result.push(((bits >> bits_count) & 0xFF) as u8);
        }
    }

    Some(result)
}

// ============================================================================
// URL 编码辅助
// ============================================================================

/// URL 编码（百分号编码）
fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_secret() {
        let secret = TotpService::generate_secret();
        assert!(!secret.is_empty());
        assert!(secret.len() >= 32); // 至少 32 个字符
        assert!(secret.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_backup_codes() {
        let codes = TotpService::generate_backup_codes();
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert!(TotpService::is_valid_backup_code_format(code));
        }
    }

    #[test]
    fn test_backup_code_hash() {
        let code = "1234-5678";
        let hash = TotpService::hash_backup_code(code);
        assert!(TotpService::verify_backup_code(code, &hash));
        assert!(!TotpService::verify_backup_code("0000-0000", &hash));
    }

    #[test]
    fn test_otpauth_url() {
        let service = TotpService::new("FoxNIO");
        let url = service.generate_otpauth_url("test@example.com", "JBSWY3DPEHPK3PXP");
        
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("secret=JBSWY3DPEHPK3PXP"));
        assert!(url.contains("issuer=FoxNIO"));
        assert!(url.contains("algorithm=SHA1"));
        assert!(url.contains("digits=6"));
        assert!(url.contains("period=30"));
    }

    #[test]
    fn test_verify_code_format() {
        // 无效格式
        assert!(!TotpService::verify_code("SECRET", "12345")); // 太短
        assert!(!TotpService::verify_code("SECRET", "1234567")); // 太长
        assert!(!TotpService::verify_code("SECRET", "abcdef")); // 非数字
    }

    #[test]
    fn test_base32_encode_decode() {
        let original = b"Hello, World!";
        let encoded = base32_encode(original);
        let decoded = base32_decode(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_known_totp() {
        // 使用已知的测试向量
        // RFC 6238 测试密钥: "12345678901234567890" (20 bytes)
        // Base32 编码后: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        
        // 这个测试需要固定时间戳才能精确验证
        // 这里只验证能生成 6 位数字
        let code = TotpService::get_current_code(secret).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_remaining_seconds() {
        let remaining = TotpService::remaining_seconds();
        assert!(remaining > 0);
        assert!(remaining <= TOTP_PERIOD);
    }

    #[test]
    fn test_qr_code_generation() {
        let service = TotpService::new("FoxNIO");
        let result = service.generate_qr_code_base64("test@example.com", "JBSWY3DPEHPK3PXP");
        assert!(result.is_ok());
        
        let base64_svg = result.unwrap();
        assert!(!base64_svg.is_empty());
        
        // 解码并验证是 SVG
        let decoded = base64::engine::general_purpose::STANDARD.decode(&base64_svg).unwrap();
        let svg_str = String::from_utf8(decoded).unwrap();
        assert!(svg_str.starts_with("<?xml") || svg_str.starts_with("<svg"));
    }

    #[test]
    fn test_qr_code_data_url() {
        let service = TotpService::new("FoxNIO");
        let result = service.generate_qr_code_data_url("test@example.com", "JBSWY3DPEHPK3PXP");
        assert!(result.is_ok());
        
        let data_url = result.unwrap();
        assert!(data_url.starts_with("data:image/svg+xml;base64,"));
    }
}
