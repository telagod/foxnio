//! Token 刷新服务
//!
//! 自动刷新即将过期的 Token

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::token_cache_key::TokenCacheKey;
use super::token_refresher::TokenRefresher;

/// Token 刷新配置
#[derive(Debug, Clone)]
pub struct TokenRefreshConfig {
    pub refresh_before_expiry_seconds: u64,
    pub max_concurrent_refreshes: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub enable_background_refresh: bool,
}

impl Default for TokenRefreshConfig {
    fn default() -> Self {
        Self {
            refresh_before_expiry_seconds: 300, // 5 分钟前刷新
            max_concurrent_refreshes: 10,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            enable_background_refresh: true,
        }
    }
}

/// Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub token_type: String,
    pub scope: Option<String>,
    pub created_at: DateTime<Utc>,
    pub refreshed_at: Option<DateTime<Utc>>,
}

impl TokenInfo {
    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// 检查是否即将过期
    pub fn is_expiring_soon(&self, within_seconds: u64) -> bool {
        let threshold = Utc::now() + chrono::Duration::seconds(within_seconds as i64);
        self.expires_at <= threshold
    }

    /// 获取剩余有效时间（秒）
    pub fn remaining_seconds(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds()
    }
}

/// 刷新任务
#[derive(Debug, Clone)]
pub struct RefreshTask {
    pub key: TokenCacheKey,
    pub token_info: TokenInfo,
    pub attempts: u32,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// 刷新服务统计
#[derive(Debug, Clone, Default)]
pub struct RefreshServiceStats {
    pub total_refreshes: u64,
    pub successful_refreshes: u64,
    pub failed_refreshes: u64,
    pub skipped_refreshes: u64,
}

/// Token 刷新服务
pub struct TokenRefreshService {
    config: TokenRefreshConfig,
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,
    pending_refreshes: Arc<RwLock<HashMap<String, RefreshTask>>>,
    refresher: Arc<RwLock<TokenRefresher>>,
    stats: Arc<RwLock<RefreshServiceStats>>,
}

impl Default for TokenRefreshService {
    fn default() -> Self {
        Self::new(TokenRefreshConfig::default())
    }
}

impl TokenRefreshService {
    /// 创建新的刷新服务
    pub fn new(config: TokenRefreshConfig) -> Self {
        Self {
            config,
            tokens: Arc::new(RwLock::new(HashMap::new())),
            pending_refreshes: Arc::new(RwLock::new(HashMap::new())),
            refresher: Arc::new(RwLock::new(TokenRefresher::default())),
            stats: Arc::new(RwLock::new(RefreshServiceStats::default())),
        }
    }

    /// 注册 Token
    pub async fn register_token(&self, key: TokenCacheKey, token_info: TokenInfo) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(key.to_string(), token_info);
    }

    /// 移除 Token
    pub async fn remove_token(&self, key: &TokenCacheKey) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(&key.to_string());
    }

    /// 获取 Token
    pub async fn get_token(&self, key: &TokenCacheKey) -> Option<TokenInfo> {
        let tokens = self.tokens.read().await;
        tokens.get(&key.to_string()).cloned()
    }

    /// 刷新 Token
    pub async fn refresh_token(&self, key: &TokenCacheKey) -> Result<TokenInfo> {
        // 检查是否有待处理的刷新
        {
            let pending = self.pending_refreshes.read().await;
            if pending.contains_key(&key.to_string()) {
                // 等待现有的刷新完成
                // TODO: 实现等待机制
            }
        }

        // 获取当前 Token
        let token_info = self
            .get_token(key)
            .await
            .ok_or_else(|| anyhow::anyhow!("Token not found"))?;

        // 添加到待刷新列表
        {
            let mut pending = self.pending_refreshes.write().await;
            pending.insert(
                key.to_string(),
                RefreshTask {
                    key: key.clone(),
                    token_info: token_info.clone(),
                    attempts: 0,
                    last_attempt: None,
                    last_error: None,
                },
            );
        }

        // 执行刷新
        let result = self.do_refresh(key, &token_info).await;

        // 移除待刷新记录
        {
            let mut pending = self.pending_refreshes.write().await;
            pending.remove(&key.to_string());
        }

        result
    }

    /// 执行刷新
    async fn do_refresh(&self, key: &TokenCacheKey, token_info: &TokenInfo) -> Result<TokenInfo> {
        let provider = key.provider.to_lowercase();

        let new_token = match provider.as_str() {
            // API key providers - keys don't expire, just verify they still work
            "openai" => {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()?;

                let resp = client
                    .get("https://api.openai.com/v1/models")
                    .header(
                        "Authorization",
                        format!("Bearer {}", token_info.access_token),
                    )
                    .send()
                    .await
                    .context("OpenAI verification request failed")?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    anyhow::bail!("OpenAI token invalid: {} - {}", status, body);
                }

                // Token is valid, return as-is with refreshed timestamp
                TokenInfo {
                    refreshed_at: Some(Utc::now()),
                    ..token_info.clone()
                }
            }

            "anthropic" => {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()?;

                let resp = client
                    .get("https://api.anthropic.com/v1/models")
                    .header("x-api-key", &token_info.access_token)
                    .header("anthropic-version", "2023-06-01")
                    .send()
                    .await
                    .context("Anthropic verification request failed")?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    anyhow::bail!("Anthropic token invalid: {} - {}", status, body);
                }

                TokenInfo {
                    refreshed_at: Some(Utc::now()),
                    ..token_info.clone()
                }
            }

            // OAuth providers - use refresh_token grant
            "google" | "gemini" => {
                let refresh_token = token_info.refresh_token.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("No refresh_token available for {}", provider)
                })?;

                let refresher = self.refresher.read().await;
                let result = refresher
                    .refresh(&provider, refresh_token, token_info.scope.as_deref())
                    .await
                    .with_context(|| format!("OAuth refresh failed for {}", provider))?;

                TokenInfo {
                    access_token: result.access_token,
                    refresh_token: result.refresh_token.or(token_info.refresh_token.clone()),
                    expires_at: result.expires_at,
                    token_type: result.token_type,
                    scope: result.scope.or(token_info.scope.clone()),
                    created_at: token_info.created_at,
                    refreshed_at: Some(Utc::now()),
                }
            }

            other => {
                anyhow::bail!("Refresh not supported for provider: {}", other);
            }
        };

        // 更新 Token
        {
            let mut tokens = self.tokens.write().await;
            tokens.insert(key.to_string(), new_token.clone());
        }

        // 更新统计
        {
            let mut stats = self.stats.write().await;
            stats.total_refreshes += 1;
            stats.successful_refreshes += 1;
        }

        Ok(new_token)
    }

    /// 检查并刷新需要更新的 Token
    pub async fn check_and_refresh(&self) -> Vec<Result<TokenInfo>> {
        let mut results = Vec::new();
        let tokens = self.tokens.read().await;

        for (key_str, token_info) in tokens.iter() {
            if token_info.is_expiring_soon(self.config.refresh_before_expiry_seconds) {
                if let Some(key) = super::token_cache_key::TokenCacheKey::from_cache_string(key_str)
                {
                    let result = self.refresh_token(&key).await;
                    results.push(result);
                }
            }
        }

        results
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> RefreshServiceStats {
        self.stats.read().await.clone()
    }

    /// 获取所有 Token 数量
    pub async fn token_count(&self) -> usize {
        self.tokens.read().await.len()
    }

    /// 获取需要刷新的 Token 数量
    pub async fn pending_refresh_count(&self) -> usize {
        let tokens = self.tokens.read().await;
        tokens
            .values()
            .filter(|t| t.is_expiring_soon(self.config.refresh_before_expiry_seconds))
            .count()
    }

    /// 启动后台刷新任务
    pub fn start_background_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if let Err(e) = self
                    .check_and_refresh()
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>>>()
                {
                    tracing::error!("Background refresh error: {}", e);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_info_is_expired() {
        let expired = TokenInfo {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() - chrono::Duration::hours(1),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };
        assert!(expired.is_expired());

        let valid = TokenInfo {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };
        assert!(!valid.is_expired());
    }

    #[test]
    fn test_token_info_expiring_soon() {
        let soon = TokenInfo {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + chrono::Duration::seconds(100),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };
        assert!(soon.is_expiring_soon(300));

        let later = TokenInfo {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };
        assert!(!later.is_expiring_soon(300));
    }

    #[tokio::test]
    async fn test_register_and_get_token() {
        let service = TokenRefreshService::default();
        let key = TokenCacheKey::new("openai", "account1");

        let token = TokenInfo {
            access_token: "test_token".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };

        service.register_token(key.clone(), token.clone()).await;

        let retrieved = service.get_token(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().access_token, "test_token");
    }

    #[tokio::test]
    async fn test_remove_token() {
        let service = TokenRefreshService::default();
        let key = TokenCacheKey::new("openai", "account1");

        let token = TokenInfo {
            access_token: "test_token".to_string(),
            refresh_token: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_type: "Bearer".to_string(),
            scope: None,
            created_at: Utc::now(),
            refreshed_at: None,
        };

        service.register_token(key.clone(), token).await;
        service.remove_token(&key).await;

        let retrieved = service.get_token(&key).await;
        assert!(retrieved.is_none());
    }
}
