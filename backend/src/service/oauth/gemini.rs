//! Google Gemini OAuth 实现
//!
//! 实现 Google OAuth 2.0 授权流程，用于 Gemini API 访问。
//! 支持两种 OAuth 类型：
//! - Code Assist: 用于 Gemini CLI / Google AI Studio
//! - AI Studio: 用于直接访问 Gemini API

use super::{
    generate_code_challenge, generate_code_verifier, generate_session_id, generate_state,
    AuthUrlResult, OAuthConfig, OAuthProvider, OAuthProviderType, OAuthSession, OAuthSessionStore,
    OAuthToken,
};
use anyhow::{bail, Context, Result};
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Gemini OAuth 常量
pub mod constants {
    /// 授权端点
    pub const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

    /// Token 端点
    pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

    /// 用户信息端点
    pub const USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";

    /// Gemini CLI OAuth Client ID
    pub const GEMINI_CLI_CLIENT_ID: &str =
        "905249001602-t5qaa6tbu5bqcqc9h1eb85vntgumm1er.apps.googleusercontent.com";

    /// Client Secret 环境变量名
    pub const GEMINI_CLI_CLIENT_SECRET_ENV: &str = "GEMINI_CLI_OAUTH_CLIENT_SECRET";

    /// 默认重定向 URI
    pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8085/callback";

    /// Code Assist 默认 scope
    pub const DEFAULT_CODE_ASSIST_SCOPES: &str = "https://www.googleapis.com/auth/userinfo.email \
         https://www.googleapis.com/auth/userinfo.profile \
         https://www.googleapis.com/auth/cloud-platform \
         https://www.googleapis.com/auth/aiplatform.codeassist";

    /// AI Studio scope
    pub const DEFAULT_AI_STUDIO_SCOPES: &str = "https://www.googleapis.com/auth/userinfo.email \
         https://www.googleapis.com/auth/userinfo.profile \
         https://www.googleapis.com/auth/generative-language.retriever";

    /// Session TTL（秒）
    pub const SESSION_TTL_SECS: i64 = 1800;
}

/// OAuth 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum GeminiOAuthType {
    /// Code Assist (Gemini CLI)
    #[default]
    CodeAssist,
    /// AI Studio
    AiStudio,
    /// Google One
    GoogleOne,
}

/// Gemini Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiTokenResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    #[serde(rename = "token_type")]
    pub token_type: Option<String>,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// ID Token Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDTokenClaims {
    /// Subject (user ID)
    pub sub: Option<String>,
    /// Email
    pub email: Option<String>,
    /// Email verified
    pub email_verified: Option<bool>,
    /// Issuer
    pub iss: Option<String>,
    /// Audience
    pub aud: Option<Vec<String>>,
    /// Expiration
    pub exp: Option<i64>,
    /// Issued at
    pub iat: Option<i64>,

    /// OpenAI 兼容的 auth claims
    #[serde(rename = "https://api.openai.com/auth")]
    pub openai_auth: Option<OpenAIAuthClaims>,
}

/// OpenAI Auth Claims（兼容格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIAuthClaims {
    pub chatgpt_account_id: Option<String>,
    pub chatgpt_user_id: Option<String>,
    pub chatgpt_plan_type: Option<String>,
    pub user_id: Option<String>,
    pub poid: Option<String>,
    pub organizations: Option<Vec<OrganizationClaim>>,
}

/// Organization Claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationClaim {
    pub id: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
    pub is_default: Option<bool>,
}

/// Gemini OAuth Provider 实现
pub struct GeminiOAuthProvider {
    config: OAuthConfig,
    session_store: Arc<dyn OAuthSessionStore>,
    http_client: Client,
    oauth_type: GeminiOAuthType,
}

impl GeminiOAuthProvider {
    /// 创建新的 Gemini OAuth Provider
    pub fn new(
        config: OAuthConfig,
        session_store: Arc<dyn OAuthSessionStore>,
        http_client: Client,
    ) -> Self {
        Self {
            config,
            session_store,
            http_client,
            oauth_type: GeminiOAuthType::CodeAssist,
        }
    }

    /// 设置 OAuth 类型
    pub fn with_oauth_type(mut self, oauth_type: GeminiOAuthType) -> Self {
        self.oauth_type = oauth_type;
        self
    }

    /// 获取有效的 OAuth 配置
    pub fn effective_config(&self) -> Result<OAuthConfig> {
        let mut config = self.config.clone();

        // 如果没有配置 client_id/secret，使用内置的 Gemini CLI 配置
        if config.client_id.is_empty() {
            config.client_id = constants::GEMINI_CLI_CLIENT_ID.to_string();

            // 从环境变量获取 client_secret
            let secret =
                std::env::var(constants::GEMINI_CLI_CLIENT_SECRET_ENV).context(format!(
                    "Gemini OAuth client_secret not configured. Set {} environment variable",
                    constants::GEMINI_CLI_CLIENT_SECRET_ENV
                ))?;
            config.client_secret = Some(secret);
        }

        // 设置默认 scope
        if config.default_scope.is_empty() {
            config.default_scope = match self.oauth_type {
                GeminiOAuthType::AiStudio => constants::DEFAULT_AI_STUDIO_SCOPES.to_string(),
                _ => constants::DEFAULT_CODE_ASSIST_SCOPES.to_string(),
            };
        }

        // 过滤受限 scope（对于内置客户端）
        if config.client_id == constants::GEMINI_CLI_CLIENT_ID {
            config.default_scope = self.filter_restricted_scopes(&config.default_scope);
        }

        Ok(config)
    }

    /// 过滤内置客户端不支持的 scope
    fn filter_restricted_scopes(&self, scopes: &str) -> String {
        let restricted_prefixes = [
            "https://www.googleapis.com/auth/generative-language",
            "https://www.googleapis.com/auth/drive",
        ];

        scopes
            .split_whitespace()
            .filter(|scope| {
                !restricted_prefixes
                    .iter()
                    .any(|prefix| scope.starts_with(prefix))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 生成授权 URL
    pub async fn generate_auth_url(
        &self,
        redirect_uri: Option<&str>,
        scope: Option<&str>,
        project_id: Option<&str>,
        tier_id: Option<&str>,
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

        // 获取有效 scope
        let effective_scope = scope.unwrap_or(&config.default_scope);

        // 创建 session
        let mut session = OAuthSession::new(
            OAuthProviderType::Gemini,
            state.clone(),
            code_verifier,
            effective_redirect.to_string(),
            effective_scope.to_string(),
        );

        if let Some(proxy) = proxy_url {
            session = session.with_proxy_url(proxy.to_string());
        }

        if let Some(project) = project_id {
            session = session.with_extra("project_id".to_string(), serde_json::json!(project));
        }

        if let Some(tier) = tier_id {
            session = session.with_extra("tier_id".to_string(), serde_json::json!(tier));
        }

        session = session.with_extra(
            "oauth_type".to_string(),
            serde_json::json!(self.oauth_type.as_str()),
        );

        // 存储 session
        self.session_store.set(session).await?;

        // 构建授权 URL
        let auth_url = self.build_authorization_url(
            &config,
            &state,
            &code_challenge,
            effective_redirect,
            effective_scope,
            project_id,
        );

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
        scope: &str,
        project_id: Option<&str>,
    ) -> String {
        let mut params = vec![
            ("response_type", "code".to_string()),
            ("client_id", config.client_id.clone()),
            ("redirect_uri", redirect_uri.to_string()),
            ("scope", scope.to_string()),
            ("state", state.to_string()),
            ("code_challenge", code_challenge.to_string()),
            ("code_challenge_method", "S256".to_string()),
            ("access_type", "offline".to_string()),
            ("prompt", "consent".to_string()),
            ("include_granted_scopes", "true".to_string()),
        ];

        if let Some(project) = project_id {
            params.push(("project_id", project.to_string()));
        }

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
        if session.provider != OAuthProviderType::Gemini {
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
        self.token_response_to_oauth_token(token_response, &session)
    }

    /// 调用 Token 端点
    async fn exchange_code_for_token(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        config: &OAuthConfig,
        proxy_url: Option<&str>,
    ) -> Result<GeminiTokenResponse> {
        let client_secret = config
            .client_secret
            .as_ref()
            .context("Client secret is required for token exchange")?;

        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
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
            bail!("Failed to exchange Gemini code: {} - {}", status, body);
        }

        let token: GeminiTokenResponse = response.json().await?;
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
            .context("Client secret is required for token refresh")?;

        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
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
            bail!("Failed to refresh Gemini token: {} - {}", status, body);
        }

        let token: GeminiTokenResponse = response.json().await?;

        // 刷新时不改变 session 信息
        let empty_session = OAuthSession::new(
            OAuthProviderType::Gemini,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );

        self.token_response_to_oauth_token(token, &empty_session)
    }

    /// 解析 ID Token
    pub fn parse_id_token(&self, id_token: &str) -> Result<IDTokenClaims> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            bail!("Invalid JWT format: expected 3 parts, got {}", parts.len());
        }

        // 解码 payload
        let payload = parts[1];
        let payload = self.add_base64_padding(payload);

        let decoded = base64::engine::general_purpose::URL_SAFE
            .decode(&payload)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&payload))
            .context("Failed to decode JWT payload")?;

        let claims: IDTokenClaims =
            serde_json::from_slice(&decoded).context("Failed to parse JWT claims")?;

        Ok(claims)
    }

    /// 添加 base64 填充
    fn add_base64_padding(&self, s: &str) -> String {
        let padding = match s.len() % 4 {
            2 => "==",
            3 => "=",
            _ => "",
        };
        format!("{s}{padding}")
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, access_token: &str) -> Result<serde_json::Value> {
        let response = self
            .http_client
            .get(constants::USER_INFO_URL)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to get user info: {}", response.status());
        }

        let user_info = response.json().await?;
        Ok(user_info)
    }

    /// 将 Token 响应转换为 OAuthToken
    fn token_response_to_oauth_token(
        &self,
        response: GeminiTokenResponse,
        session: &OAuthSession,
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

        // 解析 ID Token 获取额外信息
        if let Some(id_token) = &response.id_token {
            if let Ok(claims) = self.parse_id_token(id_token) {
                if let Some(email) = claims.email {
                    token = token.with_metadata("email".to_string(), serde_json::json!(email));
                }
                if let Some(sub) = claims.sub {
                    token = token.with_metadata("sub".to_string(), serde_json::json!(sub));
                }
            }
        }

        // 添加 session 信息
        if let Some(project_id) = session.extra.get("project_id") {
            token = token.with_metadata("project_id".to_string(), project_id.clone());
        }

        if let Some(tier_id) = session.extra.get("tier_id") {
            token = token.with_metadata("tier_id".to_string(), tier_id.clone());
        }

        if let Some(oauth_type) = session.extra.get("oauth_type") {
            token = token.with_metadata("oauth_type".to_string(), oauth_type.clone());
        }

        Ok(token)
    }
}

impl GeminiOAuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CodeAssist => "code_assist",
            Self::AiStudio => "ai_studio",
            Self::GoogleOne => "google_one",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "ai_studio" => Self::AiStudio,
            "google_one" => Self::GoogleOne,
            _ => Self::CodeAssist,
        }
    }
}

#[async_trait::async_trait]
impl OAuthProvider for GeminiOAuthProvider {
    fn provider_type(&self) -> OAuthProviderType {
        OAuthProviderType::Gemini
    }

    async fn generate_auth_url(
        &self,
        redirect_uri: &str,
        scope: Option<&str>,
        _state: Option<&str>,
        _code_challenge: Option<&str>,
    ) -> Result<AuthUrlResult> {
        self.generate_auth_url(Some(redirect_uri), scope, None, None, None)
            .await
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

        let empty_session = OAuthSession::new(
            OAuthProviderType::Gemini,
            String::new(),
            String::new(),
            redirect_uri.to_string(),
            String::new(),
        );

        self.token_response_to_oauth_token(token_response, &empty_session)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        self.refresh_token(refresh_token, None).await
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

    fn create_test_provider() -> GeminiOAuthProvider {
        let config = OAuthConfig::new(
            "test_client_id".to_string(),
            constants::AUTHORIZE_URL.to_string(),
            constants::TOKEN_URL.to_string(),
            constants::DEFAULT_CODE_ASSIST_SCOPES.to_string(),
        )
        .with_client_secret("test_secret".to_string());

        let session_store = Arc::new(InMemorySessionStore::new());
        let http_client = Client::new();
        GeminiOAuthProvider::new(config, session_store, http_client)
    }

    #[test]
    fn test_provider_type() {
        let provider = create_test_provider();
        assert_eq!(provider.provider_type(), OAuthProviderType::Gemini);
    }

    #[test]
    fn test_oauth_type() {
        assert_eq!(GeminiOAuthType::CodeAssist.as_str(), "code_assist");
        assert_eq!(
            GeminiOAuthType::parse("ai_studio"),
            GeminiOAuthType::AiStudio
        );
    }

    #[test]
    fn test_filter_restricted_scopes() {
        let provider = create_test_provider();
        let scopes = "https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/generative-language";

        let filtered = provider.filter_restricted_scopes(scopes);
        assert!(!filtered.contains("generative-language"));
        assert!(filtered.contains("userinfo.email"));
    }

    #[tokio::test]
    async fn test_generate_auth_url() {
        let provider = create_test_provider();
        let result = provider
            .generate_auth_url(Some("http://localhost/callback"), None, None, None, None)
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.auth_url.starts_with(constants::AUTHORIZE_URL));
        assert!(result.auth_url.contains("client_id="));
    }

    #[test]
    fn test_token_response_conversion() {
        let provider = create_test_provider();
        let response = GeminiTokenResponse {
            access_token: "test_token".to_string(),
            id_token: None,
            token_type: Some("Bearer".to_string()),
            expires_in: 3600,
            refresh_token: Some("test_refresh".to_string()),
            scope: Some("profile".to_string()),
        };

        let session = OAuthSession::new(
            OAuthProviderType::Gemini,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );

        let token = provider
            .token_response_to_oauth_token(response, &session)
            .unwrap();

        assert_eq!(token.access_token, "test_token");
        assert!(token.has_refresh_token());
    }
}
