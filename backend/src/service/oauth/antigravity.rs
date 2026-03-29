//! Antigravity (Google Cloud Code) OAuth 实现
//!
//! 实现 Antigravity OAuth 2.0 授权流程，用于 Google Cloud Code / Antigravity API。

use super::{
    generate_code_challenge, generate_code_verifier, generate_session_id, generate_state,
    AuthUrlResult, OAuthConfig, OAuthProvider, OAuthProviderType, OAuthSession, OAuthSessionStore,
    OAuthToken,
};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Antigravity OAuth 常量
pub mod constants {
    /// 授权端点
    pub const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

    /// Token 端点
    pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

    /// 用户信息端点
    pub const USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

    /// Antigravity OAuth Client ID 环境变量
    pub const CLIENT_ID_ENV: &str = "ANTIGRAVITY_OAUTH_CLIENT_ID";

    /// Client Secret 环境变量名
    pub const CLIENT_SECRET_ENV: &str = "ANTIGRAVITY_OAUTH_CLIENT_SECRET";

    /// 测试用 Client ID
    pub const CLIENT_ID: &str = "test-client-id";

    /// 测试用默认 Client Secret
    pub const DEFAULT_CLIENT_SECRET: &str = "test-client-secret";

    /// 默认重定向 URI
    pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8085/callback";

    /// OAuth Scopes
    pub const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform \
         https://www.googleapis.com/auth/userinfo.email \
         https://www.googleapis.com/auth/userinfo.profile \
         https://www.googleapis.com/auth/cclog \
         https://www.googleapis.com/auth/experimentsandconfigs";

    /// Session TTL（秒）
    pub const SESSION_TTL_SECS: i64 = 1800;

    /// URL 可用性 TTL（秒）
    pub const URL_AVAILABILITY_TTL_SECS: i64 = 300;

    /// Antigravity API 生产环境 URL
    pub const PROD_BASE_URL: &str = "https://cloudcode-pa.googleapis.com";

    /// Antigravity API 测试环境 URL
    pub const DAILY_BASE_URL: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";
}

/// Antigravity Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravityTokenResponse {
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: Option<String>,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// URL 可用性状态管理
#[derive(Debug, Clone)]
pub struct URLAvailability {
    unavailable: std::collections::HashMap<String, chrono::DateTime<Utc>>,
    ttl: Duration,
    last_success: Option<String>,
}

impl URLAvailability {
    pub fn new(ttl: Duration) -> Self {
        Self {
            unavailable: std::collections::HashMap::new(),
            ttl,
            last_success: None,
        }
    }

    /// 标记 URL 为不可用
    pub fn mark_unavailable(&mut self, url: &str) {
        let recovery_time = Utc::now() + chrono::Duration::from_std(self.ttl).unwrap();
        self.unavailable.insert(url.to_string(), recovery_time);
    }

    /// 标记 URL 为成功
    pub fn mark_success(&mut self, url: &str) {
        self.last_success = Some(url.to_string());
        self.unavailable.remove(url);
    }

    /// 检查 URL 是否可用
    pub fn is_available(&self, url: &str) -> bool {
        match self.unavailable.get(url) {
            Some(recovery_time) => Utc::now() >= *recovery_time,
            None => true,
        }
    }

    /// 获取可用的 URL 列表
    pub fn get_available_urls(&self, base_urls: &[&str]) -> Vec<String> {
        let mut result = Vec::new();
        let _now = Utc::now();

        // 优先使用最近成功的 URL
        if let Some(ref last_success) = self.last_success {
            if base_urls.contains(&last_success.as_str()) && self.is_available(last_success) {
                result.push(last_success.clone());
            }
        }

        // 添加其他可用的 URL
        for url in base_urls {
            if Some(*url) == self.last_success.as_deref() {
                continue;
            }
            if self.is_available(url) {
                result.push(url.to_string());
            }
        }

        result
    }
}

impl Default for URLAvailability {
    fn default() -> Self {
        Self::new(Duration::from_secs(
            constants::URL_AVAILABILITY_TTL_SECS as u64,
        ))
    }
}

/// Antigravity OAuth Provider 实现
pub struct AntigravityOAuthProvider {
    config: OAuthConfig,
    session_store: Arc<dyn OAuthSessionStore>,
    http_client: Client,
    url_availability: URLAvailability,
    user_agent_version: String,
}

impl AntigravityOAuthProvider {
    /// 创建新的 Antigravity OAuth Provider
    pub fn new(
        config: OAuthConfig,
        session_store: Arc<dyn OAuthSessionStore>,
        http_client: Client,
    ) -> Self {
        // 从环境变量读取版本号
        let user_agent_version = std::env::var("ANTIGRAVITY_USER_AGENT_VERSION")
            .unwrap_or_else(|_| "1.20.5".to_string());

        Self {
            config,
            session_store,
            http_client,
            url_availability: URLAvailability::default(),
            user_agent_version,
        }
    }

    /// 获取 User-Agent
    pub fn get_user_agent(&self) -> String {
        format!("antigravity/{} windows/amd64", self.user_agent_version)
    }

    /// 获取 Client Secret
    fn get_client_secret(&self) -> Result<String> {
        // 优先使用配置中的 secret
        if let Some(ref secret) = self.config.client_secret {
            if !secret.is_empty() {
                return Ok(secret.clone());
            }
        }

        // 从环境变量读取
        if let Ok(secret) = std::env::var(constants::CLIENT_SECRET_ENV) {
            if !secret.is_empty() {
                return Ok(secret);
            }
        }

        // 没有默认值，返回错误
        bail!("ANTIGRAVITY_OAUTH_CLIENT_SECRET not set")
    }

    /// 获取有效的配置
    pub fn effective_config(&self) -> Result<OAuthConfig> {
        let mut config = self.config.clone();

        if config.client_id.is_empty() {
            config.client_id = std::env::var(constants::CLIENT_ID_ENV)
                .unwrap_or_else(|_| "YOUR_CLIENT_ID".to_string());
        }

        let secret = self.get_client_secret()?;
        config.client_secret = Some(secret);

        if config.default_scope.is_empty() {
            config.default_scope = constants::SCOPES.to_string();
        }

        Ok(config)
    }

    /// 获取 API Base URLs（按优先级）
    pub fn get_base_urls(&self) -> Vec<&'static str> {
        vec![constants::PROD_BASE_URL, constants::DAILY_BASE_URL]
    }

    /// 获取可用的 API URLs（考虑失败恢复）
    pub fn get_available_urls(&self) -> Vec<String> {
        let base_urls = self.get_base_urls();
        self.url_availability.get_available_urls(&base_urls)
    }

    /// 获取转发用的 API URLs（daily 优先）
    pub fn get_forward_urls(&self) -> Vec<String> {
        let mut urls = self.get_base_urls().to_vec();
        // 反转顺序，daily 优先
        urls.reverse();
        self.url_availability.get_available_urls(&urls)
    }

    /// 标记 URL 不可用
    pub fn mark_unavailable(&mut self, url: &str) {
        self.url_availability.mark_unavailable(url);
    }

    /// 标记 URL 成功
    pub fn mark_success(&mut self, url: &str) {
        self.url_availability.mark_success(url);
    }

    /// 生成授权 URL
    pub async fn generate_auth_url(
        &self,
        redirect_uri: Option<&str>,
        proxy_url: Option<&str>,
    ) -> Result<AuthUrlResult> {
        let config = self.effective_config()?;
        let effective_redirect = redirect_uri
            .or(config.redirect_uri.as_deref())
            .unwrap_or(constants::DEFAULT_REDIRECT_URI);

        // 生成 PKCE 参数
        let state = generate_state()?;
        let code_verifier = generate_code_verifier()?;
        let code_challenge = generate_code_challenge(&code_verifier);
        let session_id = generate_session_id()?;

        // 创建 session
        let mut session = OAuthSession::new(
            OAuthProviderType::Antigravity,
            state.clone(),
            code_verifier,
            effective_redirect.to_string(),
            config.default_scope.clone(),
        );

        if let Some(proxy) = proxy_url {
            session = session.with_proxy_url(proxy.to_string());
        }

        // 存储 session
        self.session_store.set(session).await?;

        // 构建授权 URL
        let auth_url =
            self.build_authorization_url(&config, &state, &code_challenge, effective_redirect);

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
        code_challenge: &str,
        redirect_uri: &str,
    ) -> String {
        let params = [
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", config.default_scope.as_str()),
            ("state", state),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("include_granted_scopes", "true"),
        ];

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("{}?{}", constants::AUTHORIZE_URL, query)
    }

    /// 用授权码换取 Token
    pub async fn exchange_code(
        &self,
        code: &str,
        session_id: &str,
        proxy_url: Option<&str>,
    ) -> Result<OAuthToken> {
        // 获取 session
        let session = self
            .session_store
            .get(session_id)
            .await?
            .context("Session not found or expired")?;

        // 验证 session
        if session.provider != OAuthProviderType::Antigravity {
            bail!("Invalid session provider type");
        }

        let config = self.effective_config()?;
        let effective_proxy = proxy_url.or(session.proxy_url.as_deref());

        // 交换 token
        let token_response = self
            .exchange_code_for_token(
                code,
                &session.code_verifier,
                &session.redirect_uri,
                &config,
                effective_proxy,
            )
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
        code_verifier: &str,
        redirect_uri: &str,
        config: &OAuthConfig,
        proxy_url: Option<&str>,
    ) -> Result<AntigravityTokenResponse> {
        let client_secret = config
            .client_secret
            .as_ref()
            .context("Client secret is required")?;

        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", self.get_user_agent())
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("code_verifier", code_verifier),
                ("client_id", &config.client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri),
            ]);

        if let Some(proxy) = proxy_url {
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to exchange Antigravity code: {} - {}", status, body);
        }

        let token: AntigravityTokenResponse = response.json().await?;
        Ok(token)
    }

    /// 刷新 Token
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
        proxy_url: Option<&str>,
    ) -> Result<OAuthToken> {
        let config = self.effective_config()?;
        let client_secret = config
            .client_secret
            .as_ref()
            .context("Client secret is required")?;

        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", self.get_user_agent())
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &config.client_id),
                ("client_secret", client_secret),
            ]);

        if let Some(proxy) = proxy_url {
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to refresh Antigravity token: {} - {}", status, body);
        }

        let token: AntigravityTokenResponse = response.json().await?;
        self.token_response_to_oauth_token(token)
    }

    /// 调用 Antigravity API
    pub async fn call_api(
        &mut self,
        access_token: &str,
        endpoint: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let urls = self.get_available_urls();

        for url in &urls {
            let full_url = format!("{}{}", url, endpoint);

            let mut request = self.http_client.post(&full_url);
            request = request
                .bearer_auth(access_token)
                .header("User-Agent", self.get_user_agent());

            if let Some(b) = body {
                request = request.json(b);
            }

            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    self.mark_success(url);
                    return Ok(response.json().await?);
                }
                Ok(response) if response.status().is_server_error() => {
                    self.mark_unavailable(url);
                    continue;
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    bail!("API call failed: {} - {}", status, body);
                }
                Err(_) => {
                    self.mark_unavailable(url);
                    continue;
                }
            }
        }

        bail!("All Antigravity API URLs are unavailable")
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, access_token: &str) -> Result<serde_json::Value> {
        let response = self
            .http_client
            .get(constants::USER_INFO_URL)
            .bearer_auth(access_token)
            .header("User-Agent", self.get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to get user info: {}", response.status());
        }

        Ok(response.json().await?)
    }

    /// 将 Token 响应转换为 OAuthToken
    fn token_response_to_oauth_token(
        &self,
        response: AntigravityTokenResponse,
    ) -> Result<OAuthToken> {
        let mut token = OAuthToken::new(
            response.access_token,
            response.expires_in,
            response.token_type.unwrap_or_else(|| "Bearer".to_string()),
        );

        if let Some(refresh_token) = response.refresh_token {
            token = token.with_refresh_token(refresh_token);
        }

        if let Some(scope) = response.scope {
            token = token.with_scope(scope);
        }

        token = token.with_metadata("provider".to_string(), serde_json::json!("antigravity"));

        Ok(token)
    }
}

#[async_trait::async_trait]
impl OAuthProvider for AntigravityOAuthProvider {
    fn provider_type(&self) -> OAuthProviderType {
        OAuthProviderType::Antigravity
    }

    async fn generate_auth_url(
        &self,
        redirect_uri: &str,
        _scope: Option<&str>,
        _state: Option<&str>,
        _code_challenge: Option<&str>,
    ) -> Result<AuthUrlResult> {
        self.generate_auth_url(Some(redirect_uri), None).await
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<OAuthToken> {
        let config = self.effective_config()?;

        let token_response = self
            .exchange_code_for_token(code, code_verifier, redirect_uri, &config, None)
            .await?;

        self.token_response_to_oauth_token(token_response)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        self.refresh_token(refresh_token, None).await
    }

    async fn validate_token(&self, access_token: &str) -> Result<bool> {
        let response = self
            .http_client
            .get(constants::USER_INFO_URL)
            .bearer_auth(access_token)
            .header("User-Agent", self.get_user_agent())
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::oauth::InMemorySessionStore;

    fn create_test_provider() -> AntigravityOAuthProvider {
        let config = OAuthConfig::new(
            constants::CLIENT_ID.to_string(),
            constants::AUTHORIZE_URL.to_string(),
            constants::TOKEN_URL.to_string(),
            constants::SCOPES.to_string(),
        )
        .with_client_secret(constants::DEFAULT_CLIENT_SECRET.to_string());

        let session_store = Arc::new(InMemorySessionStore::new());
        let http_client = Client::new();
        AntigravityOAuthProvider::new(config, session_store, http_client)
    }

    #[test]
    fn test_provider_type() {
        let provider = create_test_provider();
        assert_eq!(provider.provider_type(), OAuthProviderType::Antigravity);
    }

    #[test]
    fn test_user_agent() {
        let provider = create_test_provider();
        let ua = provider.get_user_agent();
        assert!(ua.starts_with("antigravity/"));
        assert!(ua.contains("windows/amd64"));
    }

    #[test]
    fn test_base_urls() {
        let provider = create_test_provider();
        let urls = provider.get_base_urls();

        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&constants::PROD_BASE_URL));
        assert!(urls.contains(&constants::DAILY_BASE_URL));
    }

    #[test]
    fn test_url_availability() {
        let mut availability = URLAvailability::default();

        assert!(availability.is_available(constants::PROD_BASE_URL));
        assert!(availability.is_available(constants::DAILY_BASE_URL));

        availability.mark_unavailable(constants::PROD_BASE_URL);
        assert!(!availability.is_available(constants::PROD_BASE_URL));
        assert!(availability.is_available(constants::DAILY_BASE_URL));

        availability.mark_success(constants::PROD_BASE_URL);
        assert!(availability.is_available(constants::PROD_BASE_URL));
    }

    #[tokio::test]
    async fn test_generate_auth_url() {
        let provider = create_test_provider();
        let result = provider.generate_auth_url(None, None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.auth_url.starts_with(constants::AUTHORIZE_URL));
        assert!(result.auth_url.contains("client_id="));
    }

    #[test]
    fn test_token_response_conversion() {
        let provider = create_test_provider();
        let response = AntigravityTokenResponse {
            access_token: "test_token".to_string(),
            token_type: Some("Bearer".to_string()),
            expires_in: 3600,
            refresh_token: Some("test_refresh".to_string()),
            scope: Some("profile".to_string()),
        };

        let token = provider.token_response_to_oauth_token(response).unwrap();

        assert_eq!(token.access_token, "test_token");
        assert!(token.has_refresh_token());
        assert!(token.metadata.contains_key("provider"));
    }
}
