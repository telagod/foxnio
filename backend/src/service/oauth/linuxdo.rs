//! LinuxDo OAuth 认证实现
//!
//! 实现 LinuxDo 社区 OAuth 2.0 授权流程

use super::{
    generate_session_id, generate_state, AuthUrlResult, OAuthConfig, OAuthProvider,
    OAuthProviderType, OAuthSession, OAuthSessionStore, OAuthToken,
};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// LinuxDo OAuth 常量
pub mod constants {
    /// 授权端点
    pub const AUTHORIZE_URL: &str = "https://connect.linux.do/oauth2/authorize";

    /// Token 端点
    pub const TOKEN_URL: &str = "https://connect.linux.do/oauth2/token";

    /// 用户信息端点
    pub const USER_INFO_URL: &str = "https://connect.linux.do/api/user";

    /// 默认 Client ID
    pub const DEFAULT_CLIENT_ID: &str = "linuxdo_client";

    /// 默认 Client Secret
    pub const DEFAULT_CLIENT_SECRET: &str = "";

    /// 默认重定向 URI
    pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8085/callback/linuxdo";

    /// OAuth Scopes
    pub const SCOPES: &str = "read write";

    /// Session TTL（秒）
    pub const SESSION_TTL_SECS: i64 = 1800;
}

/// LinuxDo 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxDoUserInfo {
    pub id: i64,
    pub username: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub active: Option<bool>,
    pub trust_level: Option<i32>,
    pub can_edit: Option<bool>,
    pub can_create_topic: Option<bool>,
}

/// LinuxDo Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxDoTokenResponse {
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// LinuxDo OAuth Provider 实现
pub struct LinuxDoOAuthProvider {
    config: OAuthConfig,
    session_store: Arc<dyn OAuthSessionStore>,
    http_client: Client,
}

impl LinuxDoOAuthProvider {
    /// 创建新的 LinuxDo OAuth Provider
    pub fn new(
        config: OAuthConfig,
        session_store: Arc<dyn OAuthSessionStore>,
        http_client: Client,
    ) -> Self {
        Self {
            config,
            session_store,
            http_client,
        }
    }

    /// 获取有效的配置
    pub fn effective_config(&self) -> Result<OAuthConfig> {
        let mut config = self.config.clone();

        if config.client_id.is_empty() {
            config.client_id = std::env::var("LINUXDO_CLIENT_ID")
                .unwrap_or_else(|_| constants::DEFAULT_CLIENT_ID.to_string());
        }

        if config.client_secret.is_none()
            || config.client_secret.as_ref().map_or(true, |s| s.is_empty())
        {
            config.client_secret = Some(
                std::env::var("LINUXDO_CLIENT_SECRET")
                    .unwrap_or_else(|_| constants::DEFAULT_CLIENT_SECRET.to_string()),
            );
        }

        if config.default_scope.is_empty() {
            config.default_scope = constants::SCOPES.to_string();
        }

        Ok(config)
    }

    /// 生成授权 URL
    pub async fn generate_auth_url(&self, redirect_uri: Option<&str>) -> Result<AuthUrlResult> {
        let config = self.effective_config()?;
        let effective_redirect = redirect_uri
            .or(config.redirect_uri.as_deref())
            .unwrap_or(constants::DEFAULT_REDIRECT_URI);

        // 生成 state
        let state = generate_state()?;
        let session_id = generate_session_id()?;

        // 创建 session
        let session = OAuthSession::new(
            OAuthProviderType::LinuxDo,
            state.clone(),
            String::new(), // LinuxDo 不需要 code_verifier
            effective_redirect.to_string(),
            config.default_scope.clone(),
        );

        // 存储 session
        self.session_store.set(session).await?;

        // 构建授权 URL
        let auth_url = self.build_authorization_url(&config, &state, effective_redirect);

        Ok(AuthUrlResult {
            auth_url,
            session_id,
        })
    }

    /// 构建授权 URL
    fn build_authorization_url(
        &self,
        config: &OAuthConfig,
        state: &str,
        redirect_uri: &str,
    ) -> String {
        let params = [
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", config.default_scope.as_str()),
            ("state", state),
        ];

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", constants::AUTHORIZE_URL, query)
    }

    /// 用授权码换取 Token
    pub async fn exchange_code(&self, code: &str, session_id: &str) -> Result<OAuthToken> {
        // 获取 session
        let session = self
            .session_store
            .get(session_id)
            .await?
            .context("Session not found or expired")?;

        // 验证 session
        if session.provider != OAuthProviderType::LinuxDo {
            bail!("Invalid session provider type");
        }

        let config = self.effective_config()?;

        // 交换 token
        let token_response = self
            .exchange_code_for_token(code, &session.redirect_uri, &config)
            .await?;

        // 删除已使用的 session
        self.session_store.delete(session_id).await?;

        // 转换为 OAuthToken
        self.token_response_to_oauth_token(token_response)
    }

    /// 调用 Token 端点
    async fn exchange_code_for_token(
        &self,
        code: &str,
        redirect_uri: &str,
        config: &OAuthConfig,
    ) -> Result<LinuxDoTokenResponse> {
        let client_secret = config
            .client_secret
            .as_ref()
            .context("Client secret is required")?;

        let response = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &config.client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to exchange LinuxDo code: {} - {}", status, body);
        }

        let token: LinuxDoTokenResponse = response.json().await?;
        Ok(token)
    }

    /// 刷新 Token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        let config = self.effective_config()?;
        let client_secret = config
            .client_secret
            .as_ref()
            .context("Client secret is required")?;

        let response = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &config.client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to refresh LinuxDo token: {} - {}", status, body);
        }

        let token: LinuxDoTokenResponse = response.json().await?;
        self.token_response_to_oauth_token(token)
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, access_token: &str) -> Result<LinuxDoUserInfo> {
        let response = self
            .http_client
            .get(constants::USER_INFO_URL)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to get user info: {}", response.status());
        }

        let user_info: LinuxDoUserInfo = response.json().await?;
        Ok(user_info)
    }

    /// 将 Token 响应转换为 OAuthToken
    fn token_response_to_oauth_token(&self, response: LinuxDoTokenResponse) -> Result<OAuthToken> {
        let mut token = OAuthToken::new(
            response.access_token,
            response.expires_in.unwrap_or(7200),
            response.token_type.unwrap_or_else(|| "Bearer".to_string()),
        );

        if let Some(refresh_token) = response.refresh_token {
            token = token.with_refresh_token(refresh_token);
        }

        if let Some(scope) = response.scope {
            token = token.with_scope(scope);
        }

        token = token.with_metadata("provider".to_string(), serde_json::json!("linuxdo"));

        Ok(token)
    }
}

#[async_trait::async_trait]
impl OAuthProvider for LinuxDoOAuthProvider {
    fn provider_type(&self) -> OAuthProviderType {
        OAuthProviderType::LinuxDo
    }

    async fn generate_auth_url(
        &self,
        redirect_uri: &str,
        _scope: Option<&str>,
        _state: Option<&str>,
        _code_challenge: Option<&str>,
    ) -> Result<AuthUrlResult> {
        self.generate_auth_url(Some(redirect_uri)).await
    }

    async fn exchange_code(
        &self,
        code: &str,
        _code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<OAuthToken> {
        // LinuxDo 不需要 code_verifier，但我们仍然需要生成 session_id
        let _session_id = generate_session_id()?;
        let session = OAuthSession::new(
            OAuthProviderType::LinuxDo,
            String::new(),
            String::new(),
            redirect_uri.to_string(),
            constants::SCOPES.to_string(),
        );
        self.session_store.set(session).await?;

        // 直接调用交换方法
        let config = self.effective_config()?;
        let token_response = self
            .exchange_code_for_token(code, redirect_uri, &config)
            .await?;

        self.token_response_to_oauth_token(token_response)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        self.refresh_token(refresh_token).await
    }

    async fn validate_token(&self, access_token: &str) -> Result<bool> {
        let response = self
            .http_client
            .get(constants::USER_INFO_URL)
            .bearer_auth(access_token)
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::oauth::InMemorySessionStore;

    fn create_test_provider() -> LinuxDoOAuthProvider {
        let config = OAuthConfig::new(
            "test_client_id".to_string(),
            constants::AUTHORIZE_URL.to_string(),
            constants::TOKEN_URL.to_string(),
            constants::SCOPES.to_string(),
        );

        let session_store = Arc::new(InMemorySessionStore::new());
        let http_client = Client::new();
        LinuxDoOAuthProvider::new(config, session_store, http_client)
    }

    #[test]
    fn test_provider_type() {
        let provider = create_test_provider();
        assert_eq!(provider.provider_type(), OAuthProviderType::LinuxDo);
    }

    #[tokio::test]
    async fn test_generate_auth_url() {
        let provider = create_test_provider();
        let result = provider.generate_auth_url(None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.auth_url.starts_with(constants::AUTHORIZE_URL));
        assert!(result.auth_url.contains("client_id="));
    }

    #[test]
    fn test_token_response_conversion() {
        let provider = create_test_provider();
        let response = LinuxDoTokenResponse {
            access_token: "test_token".to_string(),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(3600),
            refresh_token: Some("test_refresh".to_string()),
            scope: Some("read write".to_string()),
        };

        let token = provider.token_response_to_oauth_token(response).unwrap();

        assert_eq!(token.access_token, "test_token");
        assert!(token.has_refresh_token());
        assert!(token.metadata.contains_key("provider"));
    }
}
