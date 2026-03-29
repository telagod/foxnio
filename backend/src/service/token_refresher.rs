//! Token 刷新器
//!
//! 负责实际的 Token 刷新操作

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 刷新请求
#[derive(Debug, Clone, Serialize)]
pub struct RefreshRequest {
    pub grant_type: String,
    pub refresh_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// 刷新响应
#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Provider 配置
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub token_url: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub default_scope: Option<String>,
    pub timeout_seconds: u64,
}

impl ProviderConfig {
    /// OpenAI 配置
    pub fn openai() -> Self {
        Self {
            name: "openai".to_string(),
            token_url: "https://auth0.openai.com/oauth/token".to_string(),
            client_id: None,
            client_secret: None,
            default_scope: None,
            timeout_seconds: 30,
        }
    }

    /// Anthropic 配置
    pub fn anthropic() -> Self {
        Self {
            name: "anthropic".to_string(),
            token_url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            client_id: None,
            client_secret: None,
            default_scope: None,
            timeout_seconds: 30,
        }
    }

    /// Gemini 配置
    pub fn gemini() -> Self {
        Self {
            name: "gemini".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            client_id: None,
            client_secret: None,
            default_scope: Some("https://www.googleapis.com/auth/cloud-platform".to_string()),
            timeout_seconds: 30,
        }
    }
}

/// 刷新结果
#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub token_type: String,
    pub scope: Option<String>,
}

/// Token 刷新器
pub struct TokenRefresher {
    client: Client,
    providers: HashMap<String, ProviderConfig>,
}

impl TokenRefresher {
    /// 创建新的刷新器
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let mut providers = HashMap::new();
        providers.insert("openai".to_string(), ProviderConfig::openai());
        providers.insert("anthropic".to_string(), ProviderConfig::anthropic());
        providers.insert("gemini".to_string(), ProviderConfig::gemini());

        Self { client, providers }
    }

    /// 添加 Provider 配置
    pub fn add_provider(&mut self, config: ProviderConfig) {
        self.providers.insert(config.name.clone(), config);
    }

    /// 刷新 Token
    pub async fn refresh(
        &self,
        provider: &str,
        refresh_token: &str,
        scope: Option<&str>,
    ) -> Result<RefreshResult> {
        let config = self
            .providers
            .get(provider)
            .ok_or_else(|| anyhow!("Unknown provider: {}", provider))?;

        let request = RefreshRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: refresh_token.to_string(),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            scope: scope
                .map(|s| s.to_string())
                .or(config.default_scope.clone()),
        };

        let response = self
            .client
            .post(&config.token_url)
            .header("Content-Type", "application/json")
            .json(&request)
            .timeout(Duration::from_secs(config.timeout_seconds))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Token refresh failed: {} - {}", status, body));
        }

        let refresh_response: RefreshResponse = response.json().await?;

        let expires_at = refresh_response
            .expires_in
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds as i64))
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        Ok(RefreshResult {
            access_token: refresh_response.access_token,
            refresh_token: refresh_response.refresh_token,
            expires_at,
            token_type: refresh_response
                .token_type
                .unwrap_or_else(|| "Bearer".to_string()),
            scope: refresh_response.scope,
        })
    }

    /// 使用客户端凭证获取 Token
    pub async fn client_credentials(
        &self,
        provider: &str,
        scope: Option<&str>,
    ) -> Result<RefreshResult> {
        let config = self
            .providers
            .get(provider)
            .ok_or_else(|| anyhow!("Unknown provider: {}", provider))?;

        let client_id = config
            .client_id
            .as_ref()
            .ok_or_else(|| anyhow!("Client ID not configured for provider: {}", provider))?;
        let client_secret = config
            .client_secret
            .as_ref()
            .ok_or_else(|| anyhow!("Client secret not configured for provider: {}", provider))?;

        let mut body = HashMap::new();
        body.insert("grant_type", "client_credentials");
        body.insert("client_id", client_id);
        body.insert("client_secret", client_secret);
        if let Some(s) = scope {
            body.insert("scope", s);
        }

        let response = self
            .client
            .post(&config.token_url)
            .form(&body)
            .timeout(Duration::from_secs(config.timeout_seconds))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Client credentials failed: {} - {}", status, body));
        }

        let refresh_response: RefreshResponse = response.json().await?;

        let expires_at = refresh_response
            .expires_in
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds as i64))
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        Ok(RefreshResult {
            access_token: refresh_response.access_token,
            refresh_token: refresh_response.refresh_token,
            expires_at,
            token_type: refresh_response
                .token_type
                .unwrap_or_else(|| "Bearer".to_string()),
            scope: refresh_response.scope,
        })
    }

    /// 撤销 Token
    pub async fn revoke(&self, provider: &str, token: &str) -> Result<()> {
        let config = self
            .providers
            .get(provider)
            .ok_or_else(|| anyhow!("Unknown provider: {}", provider))?;

        // 构建 revoke URL（通常在 token_url 基础上）
        let revoke_url = config.token_url.replace("/token", "/revoke");

        let mut body = HashMap::new();
        body.insert("token", token);

        let response = self
            .client
            .post(&revoke_url)
            .form(&body)
            .timeout(Duration::from_secs(config.timeout_seconds))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Token revocation failed: {} - {}", status, body));
        }

        Ok(())
    }

    /// 检查 Provider 是否支持
    pub fn supports_provider(&self, provider: &str) -> bool {
        self.providers.contains_key(provider)
    }

    /// 获取支持的 Providers
    pub fn supported_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

impl Default for TokenRefresher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_configs() {
        let openai = ProviderConfig::openai();
        assert_eq!(openai.name, "openai");
        assert!(openai.token_url.contains("openai"));

        let anthropic = ProviderConfig::anthropic();
        assert_eq!(anthropic.name, "anthropic");

        let gemini = ProviderConfig::gemini();
        assert_eq!(gemini.name, "gemini");
    }

    #[test]
    fn test_refresh_request_serialization() {
        let request = RefreshRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: "test_token".to_string(),
            client_id: Some("client123".to_string()),
            client_secret: None,
            scope: Some("read write".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("refresh_token"));
        assert!(json.contains("client123"));
    }

    #[test]
    fn test_refresh_response_deserialization() {
        let json = r#"{
            "access_token": "new_token",
            "refresh_token": "new_refresh",
            "expires_in": 3600,
            "token_type": "Bearer",
            "scope": "read"
        }"#;

        let response: RefreshResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "new_token");
        assert_eq!(response.refresh_token, Some("new_refresh".to_string()));
        assert_eq!(response.expires_in, Some(3600));
    }

    #[test]
    fn test_token_refresher_creation() {
        let refresher = TokenRefresher::new();

        assert!(refresher.supports_provider("openai"));
        assert!(refresher.supports_provider("anthropic"));
        assert!(refresher.supports_provider("gemini"));
        assert!(!refresher.supports_provider("unknown"));
    }

    #[test]
    fn test_supported_providers() {
        let refresher = TokenRefresher::new();
        let providers = refresher.supported_providers();

        assert!(providers.contains(&"openai".to_string()));
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_add_provider() {
        let mut refresher = TokenRefresher::new();

        refresher.add_provider(ProviderConfig {
            name: "custom".to_string(),
            token_url: "https://custom.example.com/token".to_string(),
            client_id: None,
            client_secret: None,
            default_scope: None,
            timeout_seconds: 30,
        });

        assert!(refresher.supports_provider("custom"));
    }
}
