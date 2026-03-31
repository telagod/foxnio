//! HTTP Webhook 告警通道

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

use super::{AlertChannel, AlertSendResult, WebhookChannelConfig};
use crate::alert::{Alert, AlertChannelType};

/// Webhook 告警通道
pub struct WebhookChannel {
    config: WebhookChannelConfig,
    client: Client,
    name: String,
}

impl WebhookChannel {
    pub fn new(config: WebhookChannelConfig) -> Self {
        let name = format!("Webhook:{}", config.url);

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            config,
            client,
            name,
        }
    }

    /// 构建请求体
    fn build_body(&self, alert: &Alert) -> serde_json::Value {
        serde_json::json!({
            "level": alert.level.as_str(),
            "title": alert.title,
            "message": alert.message,
            "source": alert.source,
            "timestamp": alert.timestamp.to_rfc3339(),
            "labels": alert.labels,
            "summary": alert.to_summary(),
        })
    }
}

#[async_trait]
impl AlertChannel for WebhookChannel {
    async fn send(&self, alert: &Alert) -> AlertSendResult {
        let body = self.build_body(alert);

        let mut request = match self.config.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.config.url),
            "POST" => self.client.post(&self.config.url).json(&body),
            "PUT" => self.client.put(&self.config.url).json(&body),
            _ => self.client.post(&self.config.url).json(&body),
        };

        // 添加自定义请求头
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // 发送请求
        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    AlertSendResult::success(AlertChannelType::Webhook)
                } else {
                    AlertSendResult::failure(
                        AlertChannelType::Webhook,
                        format!(
                            "HTTP {}: {}",
                            response.status().as_u16(),
                            response.status().canonical_reason().unwrap_or("Unknown")
                        ),
                    )
                }
            }
            Err(e) => {
                AlertSendResult::failure(AlertChannelType::Webhook, format!("Request failed: {e}"))
            }
        }
    }

    fn channel_type(&self) -> AlertChannelType {
        AlertChannelType::Webhook
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        !self.config.url.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::AlertLevel;

    fn create_test_config() -> WebhookChannelConfig {
        WebhookChannelConfig {
            url: "https://example.com/webhook".to_string(),
            method: "POST".to_string(),
            headers: std::collections::HashMap::new(),
            timeout_secs: 30,
        }
    }

    fn create_test_alert() -> Alert {
        Alert::new(AlertLevel::Error, "API 错误", "请求超时").with_source("api_gateway")
    }

    #[test]
    fn test_webhook_channel_creation() {
        let config = create_test_config();
        let channel = WebhookChannel::new(config);

        assert_eq!(channel.channel_type(), AlertChannelType::Webhook);
        assert!(channel.is_available());
    }

    #[test]
    fn test_build_body() {
        let config = create_test_config();
        let channel = WebhookChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_body(&alert);

        assert_eq!(body["level"], "error");
        assert_eq!(body["title"], "API 错误");
        assert_eq!(body["source"], "api_gateway");
    }

    #[test]
    fn test_channel_not_available() {
        let config = WebhookChannelConfig {
            url: "".to_string(),
            method: "POST".to_string(),
            headers: std::collections::HashMap::new(),
            timeout_secs: 30,
        };

        let channel = WebhookChannel::new(config);
        assert!(!channel.is_available());
    }
}
