//! Gemini OAuth 服务 - Gemini OAuth Service
//!
//! 管理 Gemini API 的 OAuth 认证

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth Token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl OAuthToken {
    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        let expires_at = self.created_at + Duration::seconds(self.expires_in);
        expires_at < Utc::now()
    }

    /// 检查是否即将过期（提前5分钟）
    pub fn is_expiring_soon(&self) -> bool {
        let expires_at = self.created_at + Duration::seconds(self.expires_in);
        expires_at < Utc::now() + Duration::minutes(5)
    }
}

/// OAuth 配置
#[derive(Debug, Clone)]
pub struct GeminiOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
}

impl Default for GeminiOAuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            scopes: vec!["https://www.googleapis.com/auth/cloud-platform".to_string()],
        }
    }
}

/// Gemini OAuth 服务
pub struct GeminiOAuthService {
    db: sea_orm::DatabaseConnection,
    config: GeminiOAuthConfig,
    http_client: reqwest::Client,
    token_cache: Arc<RwLock<std::collections::HashMap<String, OAuthToken>>>,
}

impl GeminiOAuthService {
    /// 创建新的 OAuth 服务
    pub fn new(db: sea_orm::DatabaseConnection, config: GeminiOAuthConfig) -> Self {
        let http_client = reqwest::Client::new();

        Self {
            db,
            config,
            http_client,
            token_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 获取授权 URL
    pub fn get_authorization_url(&self, state: &str) -> String {
        let scope_str = self.config.scopes.join(" ");
        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", &scope_str),
            ("state", state),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ];

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", self.config.auth_url, query)
    }

    /// 用授权码换取 Token
    pub async fn exchange_code(&self, code: &str) -> Result<OAuthToken> {
        let params = vec![
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ];

        let response = self
            .http_client
            .post(&self.config.token_url)
            .form(&params)
            .send()
            .await?;

        let token: OAuthToken = response.json().await?;

        Ok(token)
    }

    /// 刷新 Token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        let params = vec![
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = self
            .http_client
            .post(&self.config.token_url)
            .form(&params)
            .send()
            .await?;

        let mut token: OAuthToken = response.json().await?;
        token.refresh_token = Some(refresh_token.to_string());
        token.created_at = Utc::now();

        Ok(token)
    }

    /// 存储 Token
    pub async fn store_token(&self, account_id: i64, token: &OAuthToken) -> Result<()> {
        // 更新缓存
        {
            let mut cache = self.token_cache.write().await;
            cache.insert(account_id.to_string(), token.clone());
        }

        // 通过 oauth_tokens entity 存储

        Ok(())
    }

    /// 获取 Token
    pub async fn get_token(&self, account_id: i64) -> Result<Option<OAuthToken>> {
        // 先检查缓存
        {
            let cache = self.token_cache.read().await;
            if let Some(token) = cache.get(&account_id.to_string()) {
                if !token.is_expired() {
                    return Ok(Some(token.clone()));
                }
            }
        }

        // NOTE: 从数据库加载

        Ok(None)
    }

    /// 获取有效的 Token（自动刷新）
    pub async fn get_valid_token(&self, account_id: i64) -> Result<Option<String>> {
        let token = self.get_token(account_id).await?;

        if let Some(token) = token {
            // 检查是否需要刷新
            if token.is_expiring_soon() {
                if let Some(refresh_token) = &token.refresh_token {
                    let new_token = self.refresh_token(refresh_token).await?;
                    self.store_token(account_id, &new_token).await?;
                    return Ok(Some(new_token.access_token));
                }
            } else {
                return Ok(Some(token.access_token));
            }
        }

        Ok(None)
    }

    /// 撤销 Token
    pub async fn revoke_token(&self, token: &str) -> Result<bool> {
        let revoke_url = "https://oauth2.googleapis.com/revoke";

        let response = self
            .http_client
            .post(revoke_url)
            .form(&[("token", token)])
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// 删除 Token
    pub async fn delete_token(&self, account_id: i64) -> Result<()> {
        // 从缓存移除
        {
            let mut cache = self.token_cache.write().await;
            cache.remove(&account_id.to_string());
        }

        // NOTE: 从数据库删除

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_token_expiry() {
        let token = OAuthToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: None,
            scope: None,
            created_at: Utc::now(),
        };

        assert!(!token.is_expired());
        assert!(!token.is_expiring_soon());
    }

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_authorization_url() {
        let config = GeminiOAuthConfig {
            client_id: "test-client-id".to_string(),
            ..Default::default()
        };

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let service = GeminiOAuthService::new(db, config);

        let url = service.get_authorization_url("test-state");
        assert!(url.contains("test-client-id"));
        assert!(url.contains("test-state"));
    }
}
