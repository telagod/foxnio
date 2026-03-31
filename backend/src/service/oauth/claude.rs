//! Claude (Anthropic) OAuth 实现
//!
//! 实现 Claude OAuth 2.0 授权流程，包括：
//! - PKCE 授权码流程
//! - Token 刷新
//! - Cookie 自动授权

use super::{
    generate_code_challenge, generate_code_verifier, generate_session_id, generate_state,
    AuthUrlResult, OAuthProvider, OAuthProviderType, OAuthSession, OAuthSessionStore, OAuthToken,
};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Claude OAuth 常量
pub mod constants {
    /// OAuth Client ID
    pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

    /// 授权端点
    pub const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";

    /// Token 端点
    pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

    /// 重定向 URI
    pub const REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";

    /// 完整 scope（浏览器授权 URL）
    pub const SCOPE_OAUTH: &str =
        "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers";

    /// API scope（内部 API 调用）
    pub const SCOPE_API: &str =
        "user:profile user:inference user:sessions:claude_code user:mcp_servers";

    /// Setup Token scope（仅推理）
    pub const SCOPE_INFERENCE: &str = "user:inference";

    /// Session TTL（秒）
    pub const SESSION_TTL_SECS: i64 = 1800; // 30 分钟
}

/// Claude OAuth Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTokenResponse {
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: Option<String>,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub organization: Option<ClaudeOrgInfo>,
    pub account: Option<ClaudeAccountInfo>,
}

/// Claude 组织信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOrgInfo {
    pub uuid: String,
}

/// Claude 账号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAccountInfo {
    pub uuid: String,
    pub email_address: Option<String>,
}

/// Claude OAuth Provider 实现
pub struct ClaudeOAuthProvider {
    session_store: Arc<dyn OAuthSessionStore>,
    http_client: Client,
}

impl ClaudeOAuthProvider {
    /// 创建新的 Claude OAuth Provider
    pub fn new(session_store: Arc<dyn OAuthSessionStore>, http_client: Client) -> Self {
        Self {
            session_store,
            http_client,
        }
    }

    /// 生成完整 scope 的授权 URL
    pub async fn generate_oauth_url(&self, proxy_url: Option<&str>) -> Result<AuthUrlResult> {
        self.generate_auth_url_with_scope(constants::SCOPE_OAUTH, proxy_url)
            .await
    }

    /// 生成 Setup Token 授权 URL（仅推理权限）
    pub async fn generate_setup_token_url(&self, proxy_url: Option<&str>) -> Result<AuthUrlResult> {
        self.generate_auth_url_with_scope(constants::SCOPE_INFERENCE, proxy_url)
            .await
    }

    /// 生成指定 scope 的授权 URL
    pub async fn generate_auth_url_with_scope(
        &self,
        scope: &str,
        proxy_url: Option<&str>,
    ) -> Result<AuthUrlResult> {
        // 生成 PKCE 参数
        let state = generate_state()?;
        let code_verifier = generate_code_verifier()?;
        let code_challenge = generate_code_challenge(&code_verifier);
        let session_id = generate_session_id()?;

        // 创建 session
        let session = OAuthSession::new(
            OAuthProviderType::Claude,
            state.clone(),
            code_verifier,
            constants::REDIRECT_URI.to_string(),
            scope.to_string(),
        );

        let session = if let Some(proxy) = proxy_url {
            session.with_proxy_url(proxy.to_string())
        } else {
            session
        };

        // 存储 session
        self.session_store.set(session).await?;

        // 构建授权 URL
        let auth_url = self.build_authorization_url(&state, &code_challenge, scope);

        Ok(AuthUrlResult {
            auth_url,
            session_id,
        })
    }

    /// 构建授权 URL
    fn build_authorization_url(&self, state: &str, code_challenge: &str, scope: &str) -> String {
        use urlencoding::encode;

        format!(
            "{}?code=true&client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
            constants::AUTHORIZE_URL,
            constants::CLIENT_ID,
            encode(constants::REDIRECT_URI),
            encode(scope).replace("%20", "+"),
            code_challenge,
            state
        )
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
        if session.provider != OAuthProviderType::Claude {
            bail!("Invalid session provider type");
        }

        // 使用 session 中的 proxy_url 或参数中的
        let effective_proxy = proxy_url.or(session.proxy_url.as_deref());

        // 交换 token
        let token_response = self
            .exchange_code_for_token(
                code,
                &session.code_verifier,
                &session.state,
                effective_proxy,
            )
            .await?;

        // 删除已使用的 session
        self.session_store.delete(session_id).await?;

        // 转换为 OAuthToken
        self.token_response_to_oauth_token(token_response)
    }

    /// 调用 Token 端点交换 code
    async fn exchange_code_for_token(
        &self,
        code: &str,
        code_verifier: &str,
        state: &str,
        proxy_url: Option<&str>,
    ) -> Result<ClaudeTokenResponse> {
        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("code_verifier", code_verifier),
                ("state", state),
                ("client_id", constants::CLIENT_ID),
                ("redirect_uri", constants::REDIRECT_URI),
            ]);

        // 添加代理支持
        if let Some(proxy) = proxy_url {
            // 通过自定义 header 传递代理信息
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to exchange Claude code: {} - {}", status, body);
        }

        let token: ClaudeTokenResponse = response.json().await?;
        Ok(token)
    }

    /// 刷新 Token
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
        proxy_url: Option<&str>,
    ) -> Result<OAuthToken> {
        let mut request = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", constants::CLIENT_ID),
            ]);

        if let Some(proxy) = proxy_url {
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to refresh Claude token: {} - {}", status, body);
        }

        let token: ClaudeTokenResponse = response.json().await?;
        self.token_response_to_oauth_token(token)
    }

    /// 使用 sessionKey 进行 Cookie 授权
    pub async fn cookie_auth(
        &self,
        session_key: &str,
        scope: &str,
        proxy_url: Option<&str>,
    ) -> Result<OAuthToken> {
        // Step 1: 获取组织信息
        let org_uuid = self.get_organization_uuid(session_key, proxy_url).await?;

        // Step 2: 生成 PKCE 参数
        let state = generate_state()?;
        let code_verifier = generate_code_verifier()?;
        let code_challenge = generate_code_challenge(&code_verifier);

        // Step 3: 使用 cookie 获取授权码
        let auth_code = self
            .get_authorization_code(
                session_key,
                &org_uuid,
                scope,
                &code_challenge,
                &state,
                proxy_url,
            )
            .await?;

        // Step 4: 交换 token
        let is_setup_token = scope == constants::SCOPE_INFERENCE;
        let token_response = self
            .exchange_code_for_token(&auth_code, &code_verifier, &state, proxy_url)
            .await?;

        let mut token = self.token_response_to_oauth_token(token_response)?;

        // 确保有 org_uuid
        if !token.metadata.contains_key("org_uuid") {
            token = token.with_metadata("org_uuid".to_string(), serde_json::json!(org_uuid));
        }

        if is_setup_token {
            token = token.with_metadata("is_setup_token".to_string(), serde_json::json!(true));
        }

        Ok(token)
    }

    /// 获取组织 UUID
    async fn get_organization_uuid(
        &self,
        session_key: &str,
        proxy_url: Option<&str>,
    ) -> Result<String> {
        let url = "https://claude.ai/api/organizations";

        let mut request = self
            .http_client
            .get(url)
            .header("Cookie", format!("sessionKey={session_key}"));

        if let Some(proxy) = proxy_url {
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            bail!("Failed to get organization info: {}", response.status());
        }

        let body = response.text().await?;

        // 解析响应获取第一个组织的 UUID
        let orgs: Vec<serde_json::Value> =
            serde_json::from_str(&body).context("Failed to parse organization response")?;

        if let Some(org) = orgs.first() {
            org["uuid"]
                .as_str()
                .map(|s| s.to_string())
                .context("No uuid in organization")
        } else {
            bail!("No organizations found")
        }
    }

    /// 使用 cookie 获取授权码
    async fn get_authorization_code(
        &self,
        session_key: &str,
        org_uuid: &str,
        scope: &str,
        code_challenge: &str,
        state: &str,
        proxy_url: Option<&str>,
    ) -> Result<String> {
        let url = format!(
            "https://claude.ai/oauth/authorize/code?organization_uuid={}&scope={}&code_challenge={}&state={}",
            org_uuid,
            urlencoding::encode(scope),
            code_challenge,
            state
        );

        let mut request = self
            .http_client
            .get(&url)
            .header("Cookie", format!("sessionKey={session_key}"));

        if let Some(proxy) = proxy_url {
            request = request.header("X-Proxy-URL", proxy);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            bail!("Failed to get authorization code: {}", response.status());
        }

        let body = response.text().await?;
        let json: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse authorization code response")?;

        json["code"]
            .as_str()
            .map(|s| s.to_string())
            .context("No code in response")
    }

    /// 将 ClaudeTokenResponse 转换为 OAuthToken
    fn token_response_to_oauth_token(&self, response: ClaudeTokenResponse) -> Result<OAuthToken> {
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

        // 添加组织和账号信息到 metadata
        if let Some(org) = response.organization {
            token = token.with_metadata("org_uuid".to_string(), serde_json::json!(org.uuid));
        }

        if let Some(account) = response.account {
            if let Some(email) = account.email_address {
                token = token.with_metadata("email".to_string(), serde_json::json!(email));
            }
            token =
                token.with_metadata("account_uuid".to_string(), serde_json::json!(account.uuid));
        }

        Ok(token)
    }
}

#[async_trait::async_trait]
impl OAuthProvider for ClaudeOAuthProvider {
    fn provider_type(&self) -> OAuthProviderType {
        OAuthProviderType::Claude
    }

    async fn generate_auth_url(
        &self,
        _redirect_uri: &str,
        scope: Option<&str>,
        _state: Option<&str>,
        _code_challenge: Option<&str>,
    ) -> Result<AuthUrlResult> {
        let effective_scope = scope.unwrap_or(constants::SCOPE_OAUTH);
        self.generate_auth_url_with_scope(effective_scope, None)
            .await
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<OAuthToken> {
        let state = generate_state()?;

        let token_response = self
            .http_client
            .post(constants::TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("code_verifier", code_verifier),
                ("state", &state),
                ("client_id", constants::CLIENT_ID),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await?;

        if !token_response.status().is_success() {
            bail!("Failed to exchange code: {}", token_response.status());
        }

        let response: ClaudeTokenResponse = token_response.json().await?;
        self.token_response_to_oauth_token(response)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        self.refresh_token(refresh_token, None).await
    }

    async fn validate_token(&self, access_token: &str) -> Result<bool> {
        // 尝试调用一个需要认证的 API 来验证 token
        let response = self
            .http_client
            .get("https://api.anthropic.com/v1/users/me")
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

    fn create_test_provider() -> ClaudeOAuthProvider {
        let session_store = Arc::new(InMemorySessionStore::new());
        let http_client = Client::new();
        ClaudeOAuthProvider::new(session_store, http_client)
    }

    #[test]
    fn test_provider_type() {
        let provider = create_test_provider();
        assert_eq!(provider.provider_type(), OAuthProviderType::Claude);
    }

    #[test]
    fn test_constants() {
        assert!(!constants::CLIENT_ID.is_empty());
        assert!(constants::AUTHORIZE_URL.starts_with("https://"));
        assert!(constants::TOKEN_URL.starts_with("https://"));
    }

    #[test]
    fn test_build_authorization_url() {
        let provider = create_test_provider();
        let url = provider.build_authorization_url(
            "test_state",
            "test_challenge",
            constants::SCOPE_OAUTH,
        );

        assert!(url.contains("client_id="));
        assert!(url.contains("state=test_state"));
        assert!(url.contains("code_challenge=test_challenge"));
    }

    #[tokio::test]
    async fn test_generate_oauth_url() {
        let provider = create_test_provider();
        let result = provider.generate_oauth_url(None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.auth_url.starts_with(constants::AUTHORIZE_URL));
        assert!(!result.session_id.is_empty());
    }

    #[tokio::test]
    async fn test_generate_setup_token_url() {
        let provider = create_test_provider();
        let result = provider.generate_setup_token_url(None).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        // Scope is URL-encoded, so "user:inference" becomes "user%3Ainference"
        assert!(
            result.auth_url.contains("user%3Ainference")
                || result.auth_url.contains("user:inference")
        );
    }

    #[test]
    fn test_token_response_conversion() {
        let provider = create_test_provider();
        let response = ClaudeTokenResponse {
            access_token: "test_token".to_string(),
            token_type: Some("Bearer".to_string()),
            expires_in: 3600,
            refresh_token: Some("test_refresh".to_string()),
            scope: Some("user:profile".to_string()),
            organization: Some(ClaudeOrgInfo {
                uuid: "org-uuid".to_string(),
            }),
            account: Some(ClaudeAccountInfo {
                uuid: "account-uuid".to_string(),
                email_address: Some("test@example.com".to_string()),
            }),
        };

        let token = provider.token_response_to_oauth_token(response).unwrap();

        assert_eq!(token.access_token, "test_token");
        assert!(token.has_refresh_token());
        assert!(token.metadata.contains_key("org_uuid"));
    }
}
