use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Quota fetcher for Antigravity API
pub struct AntigravityQuotaFetcher {
    client: Client,
    api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaResponse {
    pub account_id: String,
    pub total_quota: u64,
    pub used_quota: u64,
    pub remaining_quota: u64,
    pub reset_at: DateTime<Utc>,
    pub quota_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid response")]
    InvalidResponse,
}

impl AntigravityQuotaFetcher {
    pub fn new(api_url: String) -> Self {
        Self {
            client: Client::new(),
            api_url,
        }
    }

    /// Fetch quota for account
    pub async fn fetch(
        &self,
        account_id: &str,
        api_key: &str,
    ) -> Result<QuotaResponse, FetchError> {
        let url = format!("{}/quota/{}", self.api_url, account_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?
            .json::<QuotaResponse>()
            .await?;

        Ok(response)
    }

    /// Fetch multiple quotas
    pub async fn fetch_batch(
        &self,
        account_ids: &[&str],
        api_key: &str,
    ) -> Result<Vec<QuotaResponse>, FetchError> {
        let mut results = Vec::new();

        for account_id in account_ids {
            match self.fetch(account_id, api_key).await {
                Ok(quota) => results.push(quota),
                Err(_) => continue, // Skip failed requests
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let fetcher = AntigravityQuotaFetcher::new("https://api.antigravity.ai".to_string());
        assert_eq!(fetcher.api_url, "https://api.antigravity.ai");
    }
}
