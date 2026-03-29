//! Token 缓存键服务
//!
//! 定义和管理 Token 缓存的键

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Token 类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    AccessToken,
    RefreshToken,
    ApiKey,
    SessionToken,
    BearerToken,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessToken => write!(f, "access_token"),
            Self::RefreshToken => write!(f, "refresh_token"),
            Self::ApiKey => write!(f, "api_key"),
            Self::SessionToken => write!(f, "session_token"),
            Self::BearerToken => write!(f, "bearer_token"),
        }
    }
}

/// Token 缓存键
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub struct TokenCacheKey {
    pub provider: String,
    pub account_id: String,
    pub token_type: TokenType,
    pub scope: Option<String>,
    pub context: Option<String>,
}

impl TokenCacheKey {
    /// 创建新的缓存键
    pub fn new(provider: &str, account_id: &str) -> Self {
        Self {
            provider: provider.to_string(),
            account_id: account_id.to_string(),
            token_type: TokenType::AccessToken,
            scope: None,
            context: None,
        }
    }

    /// 设置 Token 类型
    pub fn with_token_type(mut self, token_type: TokenType) -> Self {
        self.token_type = token_type;
        self
    }

    /// 设置作用域
    pub fn with_scope(mut self, scope: &str) -> Self {
        self.scope = Some(scope.to_string());
        self
    }

    /// 设置上下文
    pub fn with_context(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());
        self
    }

    /// 生成缓存键字符串
    pub fn to_cache_string(&self) -> String {
        let mut key = format!(
            "token:{}:{}:{}",
            self.provider, self.account_id, self.token_type
        );

        if let Some(scope) = &self.scope {
            key.push_str(&format!(":{}", scope));
        }

        if let Some(context) = &self.context {
            key.push_str(&format!(":{}", context));
        }

        key
    }

    /// 从缓存键字符串解析
    pub fn from_cache_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() < 4 || parts[0] != "token" {
            return None;
        }

        let token_type = match parts[3] {
            "access_token" => TokenType::AccessToken,
            "refresh_token" => TokenType::RefreshToken,
            "api_key" => TokenType::ApiKey,
            "session_token" => TokenType::SessionToken,
            "bearer_token" => TokenType::BearerToken,
            _ => return None,
        };

        Some(Self {
            provider: parts[1].to_string(),
            account_id: parts[2].to_string(),
            token_type,
            scope: parts.get(4).map(|s| s.to_string()),
            context: parts.get(5).map(|s| s.to_string()),
        })
    }

    /// 生成 Redis 键
    pub fn to_redis_key(&self) -> String {
        format!("foxnio:{}", self.to_cache_string())
    }

    /// 生成哈希键（用于分片）
    pub fn hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.to_cache_string().hash(&mut hasher);
        hasher.finish()
    }
}

impl fmt::Display for TokenCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_cache_string())
    }
}

/// 缓存键构建器
pub struct TokenCacheKeyBuilder {
    provider: String,
    account_id: String,
    token_type: TokenType,
    scope: Option<String>,
    context: Option<String>,
}

impl TokenCacheKeyBuilder {
    /// 创建新的构建器
    pub fn new(provider: &str, account_id: &str) -> Self {
        Self {
            provider: provider.to_string(),
            account_id: account_id.to_string(),
            token_type: TokenType::AccessToken,
            scope: None,
            context: None,
        }
    }

    /// 设置 Token 类型
    pub fn token_type(mut self, token_type: TokenType) -> Self {
        self.token_type = token_type;
        self
    }

    /// 设置作用域
    pub fn scope(mut self, scope: &str) -> Self {
        self.scope = Some(scope.to_string());
        self
    }

    /// 设置上下文
    pub fn context(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());
        self
    }

    /// 构建缓存键
    pub fn build(self) -> TokenCacheKey {
        TokenCacheKey {
            provider: self.provider,
            account_id: self.account_id,
            token_type: self.token_type,
            scope: self.scope,
            context: self.context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_type_display() {
        assert_eq!(TokenType::AccessToken.to_string(), "access_token");
        assert_eq!(TokenType::RefreshToken.to_string(), "refresh_token");
        assert_eq!(TokenType::ApiKey.to_string(), "api_key");
    }

    #[test]
    fn test_cache_key_creation() {
        let key = TokenCacheKey::new("openai", "account123");
        assert_eq!(key.provider, "openai");
        assert_eq!(key.account_id, "account123");
        assert_eq!(key.token_type, TokenType::AccessToken);
    }

    #[test]
    fn test_cache_key_with_options() {
        let key = TokenCacheKey::new("openai", "account123")
            .with_token_type(TokenType::RefreshToken)
            .with_scope("read write")
            .with_context("production");

        assert_eq!(key.token_type, TokenType::RefreshToken);
        assert_eq!(key.scope, Some("read write".to_string()));
        assert_eq!(key.context, Some("production".to_string()));
    }

    #[test]
    fn test_to_cache_string() {
        let key = TokenCacheKey::new("openai", "account123");
        let cache_str = key.to_cache_string();
        assert_eq!(cache_str, "token:openai:account123:access_token");

        let key_with_scope = TokenCacheKey::new("openai", "account123").with_scope("read");
        assert_eq!(
            key_with_scope.to_cache_string(),
            "token:openai:account123:access_token:read"
        );
    }

    #[test]
    fn test_from_cache_string() {
        let key_str = "token:openai:account123:access_token";
        let key = TokenCacheKey::from_cache_string(key_str).unwrap();

        assert_eq!(key.provider, "openai");
        assert_eq!(key.account_id, "account123");
        assert_eq!(key.token_type, TokenType::AccessToken);
    }

    #[test]
    fn test_roundtrip() {
        let original = TokenCacheKey::new("anthropic", "acc456")
            .with_token_type(TokenType::RefreshToken)
            .with_scope("full");

        let cache_str = original.to_cache_string();
        let parsed = TokenCacheKey::from_cache_string(&cache_str).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_redis_key() {
        let key = TokenCacheKey::new("openai", "account123");
        let redis_key = key.to_redis_key();
        assert_eq!(redis_key, "foxnio:token:openai:account123:access_token");
    }

    #[test]
    fn test_hash() {
        let key1 = TokenCacheKey::new("openai", "account123");
        let key2 = TokenCacheKey::new("openai", "account123");
        let key3 = TokenCacheKey::new("anthropic", "account123");

        assert_eq!(key1.hash(), key2.hash());
        assert_ne!(key1.hash(), key3.hash());
    }

    #[test]
    fn test_builder() {
        let key = TokenCacheKeyBuilder::new("openai", "account123")
            .token_type(TokenType::RefreshToken)
            .scope("read")
            .context("production")
            .build();

        assert_eq!(key.provider, "openai");
        assert_eq!(key.token_type, TokenType::RefreshToken);
        assert_eq!(key.scope, Some("read".to_string()));
    }
}
