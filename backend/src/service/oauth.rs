//! OAuth 讈权服务

use anyhow::{Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// OAuth 提供商
#[derive(Debug, Clone)]
pub enum OAuthProvider {
    Anthropic,
    OpenAI,
    Gemini,
}

/// OAuth Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub token_type: String,
    pub created_at: DateTime<Utc>,
}

impl OAuthToken {
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let expires_at = self.created_at + chrono::Duration::seconds(self.expires_in);
        now >= expires_at
    }
}

/// OAuth 授权服务
pub struct OAuthService {
    client: Client,
    anthropic_client_id: Option<String>,
    anthropic_client_secret: Option<String>,
    openai_client_id: Option<String>,
    openai_client_secret: Option<String>,
    gemini_client_id: Option<String>,
    gemini_client_secret: Option<String>,
}

impl OAuthService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            anthropic_client_id: None,
            anthropic_client_secret: None,
            openai_client_id: None,
            openai_client_secret: None,
            gemini_client_id: None,
            gemini_client_secret: None,
        }
    }
    
    pub fn with_anthropic(mut self, client_id: String, client_secret: String) -> Self {
        self.anthropic_client_id = Some(client_id);
        self.anthropic_client_secret = Some(client_secret);
        self
    }
    
    pub fn with_openai(mut self, client_id: String, client_secret: String) -> Self {
        self.openai_client_id = Some(client_id);
        self.openai_client_secret = Some(client_secret);
        self
    }
    
    pub fn with_gemini(mut self, client_id: String, client_secret: String) -> Self {
        self.gemini_client_id = Some(client_id);
        self.gemini_client_secret = Some(client_secret);
        self
    }
    
    /// 获取授权 URL
    pub fn get_authorization_url(&self, provider: &OAuthProvider, redirect_uri: &str, state: &str) -> Result<String> {
        match provider {
            OAuthProvider::Anthropic => {
                let client_id = self.anthropic_client_id.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Anthropic OAuth not configured"))?;
                
                Ok(format!(
                    "https://claude.ai/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}",
                    client_id,
                    urlencoding::encode(redirect_uri),
                    state
                ))
            }
            OAuthProvider::OpenAI => {
                let client_id = self.openai_client_id.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("OpenAI OAuth not configured"))?;
                
                Ok(format!(
                    "https://auth.openai.com/authorize?client_id={}&redirect_uri={}&response_type=code&scope=openid+profile+email&state={}",
                    client_id,
                    urlencoding::encode(redirect_uri),
                    state
                ))
            }
            OAuthProvider::Gemini => {
                let client_id = self.gemini_client_id.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Gemini OAuth not configured"))?;
                
                Ok(format!(
                    "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=https://www.googleapis.com/auth/generative-language&state={}",
                    client_id,
                    urlencoding::encode(redirect_uri),
                    state
                ))
            }
        }
    }
    
    /// 用授权码换取 Token
    pub async fn exchange_code(&self, provider: &OAuthProvider, code: &str, redirect_uri: &str) -> Result<OAuthToken> {
        match provider {
            OAuthProvider::Anthropic => self.exchange_anthropic_code(code, redirect_uri).await,
            OAuthProvider::OpenAI => self.exchange_openai_code(code, redirect_uri).await,
            OAuthProvider::Gemini => self.exchange_gemini_code(code, redirect_uri).await,
        }
    }
    
    async fn exchange_anthropic_code(&self, code: &str, redirect_uri: &str) -> Result<OAuthToken> {
        let client_id = self.anthropic_client_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Anthropic OAuth not configured"))?;
        let client_secret = self.anthropic_client_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Anthropic OAuth not configured"))?;
        
        let response = self.client
            .post("https://claude.ai/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;
        
        if !response.status().is_success() {
            bail!("Failed to exchange Anthropic code: {}", response.status());
        }
        
        let token: OAuthTokenResponse = response.json().await?;
        
        Ok(OAuthToken {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_in: token.expires_in,
            token_type: token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            created_at: Utc::now(),
        })
    }
    
    async fn exchange_openai_code(&self, code: &str, redirect_uri: &str) -> Result<OAuthToken> {
        let client_id = self.openai_client_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OpenAI OAuth not configured"))?;
        let client_secret = self.openai_client_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OpenAI OAuth not configured"))?;
        
        let response = self.client
            .post("https://auth.openai.com/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;
        
        if !response.status().is_success() {
            bail!("Failed to exchange OpenAI code: {}", response.status());
        }
        
        let token: OAuthTokenResponse = response.json().await?;
        
        Ok(OAuthToken {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_in: token.expires_in,
            token_type: token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            created_at: Utc::now(),
        })
    }
    
    async fn exchange_gemini_code(&self, code: &str, redirect_uri: &str) -> Result<OAuthToken> {
        let client_id = self.gemini_client_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Gemini OAuth not configured"))?;
        let client_secret = self.gemini_client_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Gemini OAuth not configured"))?;
        
        let response = self.client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;
        
        if !response.status().is_success() {
            bail!("Failed to exchange Gemini code: {}", response.status());
        }
        
        let token: OAuthTokenResponse = response.json().await?;
        
        Ok(OAuthToken {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_in: token.expires_in,
            token_type: token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            created_at: Utc::now(),
        })
    }
    
    /// 刷新 Token
    pub async fn refresh_token(&self, provider: &OAuthProvider, refresh_token: &str) -> Result<OAuthToken> {
        match provider {
            OAuthProvider::Anthropic => self.refresh_anthropic_token(refresh_token).await,
            OAuthProvider::OpenAI => self.refresh_openai_token(refresh_token).await,
            OAuthProvider::Gemini => self.refresh_gemini_token(refresh_token).await,
        }
    }
    
    async fn refresh_anthropic_token(&self, _refresh_token: &str) -> Result<OAuthToken> {
        // TODO: 实现 Anthropic token 刷新
        bail!("Anthropic token refresh not implemented")
    }
    
    async fn refresh_openai_token(&self, _refresh_token: &str) -> Result<OAuthToken> {
        // TODO: 实现 OpenAI token 刷新
        bail!("OpenAI token refresh not implemented")
    }
    
    async fn refresh_gemini_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        let client_id = self.gemini_client_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Gemini OAuth not configured"))?;
        let client_secret = self.gemini_client_secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Gemini OAuth not configured"))?;
        
        let response = self.client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;
        
        if !response.status().is_success() {
            bail!("Failed to refresh Gemini token: {}", response.status());
        }
        
        let token: OAuthTokenResponse = response.json().await?;
        
        Ok(OAuthToken {
            access_token: token.access_token,
            refresh_token: Some(refresh_token.to_string()),
            expires_in: token.expires_in,
            token_type: token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            created_at: Utc::now(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    token_type: Option<String>,
}

impl Default for OAuthService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_oauth_token_expiry() {
        let token = OAuthToken {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_in: 3600,
            token_type: "Bearer".to_string(),
            created_at: Utc::now(),
        };
        
        // Token 刚创建，不应该过期
        assert!(!token.is_expired());
    }
    
    #[test]
    fn test_oauth_token_expired() {
        let token = OAuthToken {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_in: -1, // 已过期
            token_type: "Bearer".to_string(),
            created_at: Utc::now(),
        };
        
        assert!(token.is_expired());
    }
    
    #[test]
    fn test_oauth_provider() {
        let providers = vec![
            OAuthProvider::Anthropic,
            OAuthProvider::OpenAI,
            OAuthProvider::Gemini,
        ];
        
        assert_eq!(providers.len(), 3);
    }
    
    #[test]
    fn test_oauth_service_creation() {
        let service = OAuthService::new();
        assert!(service.anthropic_client_id.is_none());
        assert!(service.openai_client_id.is_none());
        assert!(service.gemini_client_id.is_none());
    }
    
    #[test]
    fn test_oauth_service_with_config() {
        let service = OAuthService::new()
            .with_anthropic("anthropic_id".to_string(), "anthropic_secret".to_string())
            .with_openai("openai_id".to_string(), "openai_secret".to_string())
            .with_gemini("gemini_id".to_string(), "gemini_secret".to_string());
        
        assert!(service.anthropic_client_id.is_some());
        assert!(service.openai_client_id.is_some());
        assert!(service.gemini_client_id.is_some());
    }
}
