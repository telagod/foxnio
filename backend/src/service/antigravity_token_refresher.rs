use crate::service::antigravity_token_provider::{
    AntigravityToken, AntigravityTokenProvider, TokenError,
};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Token refresher for Antigravity API
pub struct AntigravityTokenRefresher {
    provider: std::sync::Arc<AntigravityTokenProvider>,
    client: Client,
    config: RefreshConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
    pub refresh_ahead_seconds: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("Token error: {0}")]
    Token(#[from] TokenError),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("No refresh token")]
    NoRefreshToken,
}

impl AntigravityTokenRefresher {
    pub fn new(provider: std::sync::Arc<AntigravityTokenProvider>, config: RefreshConfig) -> Self {
        Self {
            provider,
            client: Client::new(),
            config,
        }
    }

    /// Refresh token if needed
    pub async fn refresh_if_needed(
        &self,
        account_id: i64,
    ) -> Result<AntigravityToken, RefreshError> {
        let token = self.provider.get(account_id).await?;

        let refresh_time = token.expires_at - Duration::seconds(self.config.refresh_ahead_seconds);
        if Utc::now() >= refresh_time {
            return self.refresh(account_id, &token).await;
        }

        Ok(token)
    }

    /// Force refresh
    pub async fn refresh(
        &self,
        account_id: i64,
        current_token: &AntigravityToken,
    ) -> Result<AntigravityToken, RefreshError> {
        let refresh_token = current_token
            .refresh_token
            .as_ref()
            .ok_or(RefreshError::NoRefreshToken)?;

        let response = self
            .client
            .post(&self.config.token_url)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        let new_token = AntigravityToken {
            access_token: response.access_token,
            refresh_token: Some(refresh_token.clone()),
            expires_at: Utc::now() + Duration::seconds(response.expires_in),
            scope: response.scope.split(' ').map(|s| s.to_string()).collect(),
        };

        self.provider.store(account_id, new_token.clone()).await?;

        Ok(new_token)
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    scope: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_config() {
        let config = RefreshConfig {
            client_id: "test".to_string(),
            client_secret: "secret".to_string(),
            token_url: "https://auth.antigravity.ai/token".to_string(),
            refresh_ahead_seconds: 300,
        };

        assert_eq!(config.refresh_ahead_seconds, 300);
    }
}
