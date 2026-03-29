//! Proxy service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub is_active: bool,
}

/// Proxy service
pub struct ProxyService {
    proxies: Arc<RwLock<HashMap<String, ProxyConfig>>>,
}

impl Default for ProxyService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyService {
    pub fn new() -> Self {
        Self {
            proxies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add(&self, config: ProxyConfig) {
        let mut proxies = self.proxies.write().await;
        proxies.insert(config.id.clone(), config);
    }

    pub async fn get(&self, id: &str) -> Option<ProxyConfig> {
        let proxies = self.proxies.read().await;
        proxies.get(id).cloned()
    }

    pub async fn list_active(&self) -> Vec<ProxyConfig> {
        let proxies = self.proxies.read().await;
        proxies.values().filter(|p| p.is_active).cloned().collect()
    }

    pub async fn remove(&self, id: &str) -> bool {
        let mut proxies = self.proxies.write().await;
        proxies.remove(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proxy_service() {
        let service = ProxyService::new();

        let config = ProxyConfig {
            id: "proxy-1".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            protocol: "http".to_string(),
            username: None,
            password: None,
            is_active: true,
        };

        service.add(config).await;
        let proxy = service.get("proxy-1").await.unwrap();
        assert_eq!(proxy.host, "127.0.0.1");
    }
}
