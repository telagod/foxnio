use crate::service::gemini_token_provider::{GeminiToken, GeminiTokenProvider, TokenError};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Token refresher for Gemini API
pub struct GeminiTokenRefresher {
    provider: Arc<GeminiTokenProvider>,
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

#[derive(Debug, Serialize)]
struct RefreshRequest {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    grant_type: String,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: i64,
    token_type: String,
    scope: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("Token error: {0}")]
    Token(#[from] TokenError),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("No refresh token")]
    NoRefreshToken,
    #[error("Refresh failed: {0}")]
    RefreshFailed(String),
}

impl GeminiTokenRefresher {
    pub fn new(provider: Arc<GeminiTokenProvider>, config: RefreshConfig) -> Self {
        Self {
            provider,
            client: Client::new(),
            config,
        }
    }

    /// Refresh token if needed
    pub async fn refresh_if_needed(&self, account_id: i64) -> Result<GeminiToken, RefreshError> {
        let token = self.provider.get_token(account_id).await?;

        // Check if we should refresh ahead of time
        let refresh_time = token.expires_at - Duration::seconds(self.config.refresh_ahead_seconds);
        if Utc::now() >= refresh_time {
            return self.refresh(account_id, &token).await;
        }

        Ok(token)
    }

    /// Force refresh token
    pub async fn refresh(
        &self,
        account_id: i64,
        current_token: &GeminiToken,
    ) -> Result<GeminiToken, RefreshError> {
        let refresh_token = current_token
            .refresh_token
            .as_ref()
            .ok_or(RefreshError::NoRefreshToken)?;

        let request = RefreshRequest {
            client_id: self.config.client_id.clone(),
            client_secret: self.config.client_secret.clone(),
            refresh_token: refresh_token.clone(),
            grant_type: "refresh_token".to_string(),
        };

        let response = self
            .client
            .post(&self.config.token_url)
            .form(&request)
            .send()
            .await?
            .json::<RefreshResponse>()
            .await?;

        let new_token = GeminiToken {
            access_token: response.access_token,
            refresh_token: Some(refresh_token.clone()),
            expires_at: Utc::now() + Duration::seconds(response.expires_in),
            scope: response
                .scope
                .map(|s| s.split(' ').map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            token_type: response.token_type,
        };

        self.provider
            .store_token(account_id, new_token.clone())
            .await?;

        Ok(new_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_config() {
        let config = RefreshConfig {
            client_id: "test".to_string(),
            client_secret: "secret".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            refresh_ahead_seconds: 300,
        };

        assert_eq!(config.refresh_ahead_seconds, 300);
    }
}
