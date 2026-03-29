use bytes::Bytes;
use futures::Stream;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// HTTP transport layer for OpenAI API requests
pub struct OpenAIClientTransport {
    client: Client,
    config: TransportConfig,
    retry_state: Arc<RwLock<RetryState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub timeout_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub max_idle_connections: usize,
    pub idle_timeout_seconds: u64,
    pub enable_compression: bool,
    pub user_agent: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            connect_timeout_seconds: 10,
            max_retries: 3,
            retry_delay_ms: 100,
            max_idle_connections: 100,
            idle_timeout_seconds: 90,
            enable_compression: true,
            user_agent: "FoxNIO/1.0".to_string(),
        }
    }
}

#[derive(Debug, Default)]
struct RetryState {
    retry_counts: HashMap<String, u32>,
    last_retry_at: HashMap<String, std::time::Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    pub method: HttpMethod,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
    pub timeout_override: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    #[error("Rate limited")]
    RateLimited,
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

pub type TransportResult<T> = Result<T, TransportError>;
pub type StreamResponse = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

impl OpenAIClientTransport {
    pub fn new(config: TransportConfig) -> Result<Self, TransportError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .pool_max_idle_per_host(config.max_idle_connections)
            .pool_idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            // gzip is enabled by default in reqwest 0.11+
            .user_agent(&config.user_agent)
            .build()?;

        Ok(Self {
            client,
            config,
            retry_state: Arc::new(RwLock::new(RetryState::default())),
        })
    }

    /// Execute HTTP request with retry logic
    pub async fn execute(&self, config: RequestConfig) -> TransportResult<Response> {
        let mut retry_count = 0;
        let request_id = uuid::Uuid::new_v4().to_string();

        loop {
            match self.execute_once(&config).await {
                Ok(response) => {
                    // Check if we should retry
                    if self.should_retry(&response).await {
                        retry_count += 1;
                        if retry_count >= self.config.max_retries {
                            return Err(TransportError::MaxRetriesExceeded);
                        }

                        let delay = self.calculate_retry_delay(retry_count);
                        debug!(
                            "Retrying request {} (attempt {}/{}), delay: {:?}",
                            request_id, retry_count, self.config.max_retries, delay
                        );

                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Ok(response);
                }
                Err(e) => {
                    if self.is_retryable_error(&e) {
                        retry_count += 1;
                        if retry_count >= self.config.max_retries {
                            return Err(TransportError::MaxRetriesExceeded);
                        }

                        let delay = self.calculate_retry_delay(retry_count);
                        debug!(
                            "Retrying request {} due to error: {} (attempt {}/{})",
                            request_id, e, retry_count, self.config.max_retries
                        );

                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }

    /// Execute request once
    async fn execute_once(&self, config: &RequestConfig) -> TransportResult<Response> {
        let mut request = match config.method {
            HttpMethod::GET => self.client.get(&config.url),
            HttpMethod::POST => self.client.post(&config.url),
            HttpMethod::PUT => self.client.put(&config.url),
            HttpMethod::DELETE => self.client.delete(&config.url),
            HttpMethod::PATCH => self.client.patch(&config.url),
        };

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Add body
        if let Some(body) = &config.body {
            request = request.body(body.clone());
        }

        // Set timeout override
        if let Some(timeout) = config.timeout_override {
            request = request.timeout(timeout);
        }

        let response = request.send().await?;

        Ok(response)
    }

    /// Execute streaming request
    pub async fn execute_stream(&self, config: RequestConfig) -> TransportResult<StreamResponse> {
        let response = self.execute(config).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::InvalidResponse(format!(
                "Stream request failed with status {}: {}",
                status, body
            )));
        }

        Ok(Box::pin(response.bytes_stream()) as StreamResponse)
    }

    /// Check if response should be retried
    async fn should_retry(&self, response: &Response) -> bool {
        let status = response.status();

        // Retry on 5xx errors
        if status.is_server_error() {
            return true;
        }

        // Retry on 429 (rate limit)
        if status == StatusCode::TOO_MANY_REQUESTS {
            return true;
        }

        false
    }

    /// Check if error is retryable
    fn is_retryable_error(&self, error: &TransportError) -> bool {
        match error {
            TransportError::Timeout(_) => true,
            TransportError::ConnectionError(_) => true,
            TransportError::Http(e) => {
                e.is_timeout()
                    || e.is_connect()
                    || e.status().map_or(false, |s| s.is_server_error())
            }
            _ => false,
        }
    }

    /// Calculate retry delay with exponential backoff
    fn calculate_retry_delay(&self, retry_count: u32) -> Duration {
        let base_delay = self.config.retry_delay_ms;
        let exponential_delay = base_delay * 2u64.pow(retry_count - 1);
        let max_delay = 10_000; // 10 seconds max

        Duration::from_millis(exponential_delay.min(max_delay))
    }

    /// Check rate limit from response headers
    pub async fn check_rate_limit(&self, response: &Response) -> Option<RateLimitInfo> {
        let headers = response.headers();

        let limit = headers
            .get("x-ratelimit-limit")?
            .to_str()
            .ok()?
            .parse()
            .ok()?;

        let remaining = headers
            .get("x-ratelimit-remaining")?
            .to_str()
            .ok()?
            .parse()
            .ok()?;

        let reset = headers
            .get("x-ratelimit-reset")?
            .to_str()
            .ok()?
            .parse()
            .ok()?;

        Some(RateLimitInfo {
            limit,
            remaining,
            reset_seconds: reset,
        })
    }

    /// Get connection pool stats
    pub fn get_pool_stats(&self) -> PoolStats {
        // Reqwest doesn't expose pool stats directly, so we return a placeholder
        PoolStats {
            idle_connections: self.config.max_idle_connections,
            active_connections: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub idle_connections: usize,
    pub active_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_transport_creation() {
        let config = TransportConfig::default();
        let transport = OpenAIClientTransport::new(config);
        assert!(transport.is_ok());
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = TransportConfig::default();
        let transport = OpenAIClientTransport::new(config).unwrap();

        let delay1 = transport.calculate_retry_delay(1);
        let delay2 = transport.calculate_retry_delay(2);
        let delay3 = transport.calculate_retry_delay(3);

        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }
}
