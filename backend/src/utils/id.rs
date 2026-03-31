//! ID 生成工具
//!
//! 注意：部分工具函数暂未使用，保留供未来扩展

#![allow(dead_code)]

use rand::Rng;
use uuid::Uuid;

/// 生成 UUID v4
pub fn uuid() -> String {
    Uuid::new_v4().to_string()
}

/// 生成 UUID v4 (别名)
pub fn generate_id() -> String {
    uuid()
}

/// 生成简短 ID（8 字符）
pub fn short_id() -> String {
    let mut rng = rand::thread_rng();
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..chars.len());
            chars.chars().nth(idx).unwrap()
        })
        .collect()
}

/// 生成简短 ID (别名)
pub fn generate_short_id() -> String {
    short_id()
}

/// 生成请求 ID
pub fn request_id() -> String {
    format!("req_{}", short_id())
}

/// 生成 API Key
pub fn api_key(prefix: &str) -> String {
    let mut rng = rand::thread_rng();
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let random_part: String = (0..48)
        .map(|_| {
            let idx = rng.gen_range(0..chars.len());
            chars.chars().nth(idx).unwrap()
        })
        .collect();

    format!("{prefix}-{random_part}")
}

/// 生成 API Key (别名)
pub fn generate_api_key(prefix: &str) -> String {
    api_key(prefix)
}

/// 生成密钥
pub fn secret_key() -> String {
    let mut rng = rand::thread_rng();
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..chars.len());
            chars.chars().nth(idx).unwrap()
        })
        .collect()
}

/// 掩码字符串
pub fn mask_string(s: &str, visible_len: usize) -> String {
    if s.len() <= visible_len {
        return format!("{s}...");
    }
    format!("{}...", &s[..visible_len])
}
