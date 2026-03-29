//! OAuth service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

/// OAuth token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub token_type: String,
}

/// OAuth service
pub struct OAuthService {
    providers: Arc<RwLock<HashMap<String, OAuthProvider>>>,
    tokens: Arc<RwLock<HashMap<i64, HashMap<String, OAuthToken>>>>,
}

impl Default for OAuthService {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthService {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_provider(&self, provider: OAuthProvider) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name.clone(), provider);
    }

    pub async fn get_provider(&self, name: &str) -> Option<OAuthProvider> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    pub async fn store_token(&self, user_id: i64, provider: &str, token: OAuthToken) {
        let mut tokens = self.tokens.write().await;
        let user_tokens = tokens.entry(user_id).or_insert_with(HashMap::new);
        user_tokens.insert(provider.to_string(), token);
    }

    pub async fn get_token(&self, user_id: i64, provider: &str) -> Option<OAuthToken> {
        let tokens = self.tokens.read().await;
        tokens
            .get(&user_id)
            .and_then(|ut| ut.get(provider).cloned())
    }

    pub async fn remove_token(&self, user_id: i64, provider: &str) {
        let mut tokens = self.tokens.write().await;
        if let Some(user_tokens) = tokens.get_mut(&user_id) {
            user_tokens.remove(provider);
        }
    }

    pub fn build_auth_url(&self, provider: &OAuthProvider, state: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&scope={}&state={}&response_type=code",
            provider.auth_url,
            provider.client_id,
            urlencoding::encode(&provider.redirect_uri),
            provider.scopes.join(" "),
            state
        )
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oauth() {
        let service = OAuthService::new();

        service
            .register_provider(OAuthProvider {
                name: "google".to_string(),
                client_id: "client-id".to_string(),
                client_secret: "secret".to_string(),
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
                redirect_uri: "http://localhost/callback".to_string(),
                scopes: vec!["email".to_string()],
            })
            .await;

        let provider = service.get_provider("google").await.unwrap();
        assert_eq!(provider.client_id, "client-id");
    }
}
