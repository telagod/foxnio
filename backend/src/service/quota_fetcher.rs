//! Quota fetcher service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Quota information
#[derive(Debug, Clone)]
pub struct QuotaInfo {
    pub user_id: i64,
    pub total_quota: u64,
    pub used_quota: u64,
    pub remaining_quota: u64,
    pub reset_time: i64,
}

/// Quota fetcher service
pub struct QuotaFetcher {
    /// Quota cache
    quotas: Arc<RwLock<HashMap<i64, QuotaInfo>>>,
}

impl Default for QuotaFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaFetcher {
    pub fn new() -> Self {
        Self {
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetch quota for user
    pub async fn fetch(&self, user_id: i64) -> Option<QuotaInfo> {
        let quotas = self.quotas.read().await;
        quotas.get(&user_id).cloned()
    }

    /// Update quota
    pub async fn update(&self, user_id: i64, total: u64, used: u64) {
        let mut quotas = self.quotas.write().await;
        quotas.insert(
            user_id,
            QuotaInfo {
                user_id,
                total_quota: total,
                used_quota: used,
                remaining_quota: total.saturating_sub(used),
                reset_time: chrono::Utc::now().timestamp() + 30 * 24 * 3600,
            },
        );
    }

    /// Check if user has quota remaining
    pub async fn has_quota(&self, user_id: i64, amount: u64) -> bool {
        let quotas = self.quotas.read().await;
        quotas
            .get(&user_id)
            .map(|q| q.remaining_quota >= amount)
            .unwrap_or(false)
    }

    /// Consume quota
    pub async fn consume(&self, user_id: i64, amount: u64) -> Result<(), String> {
        let mut quotas = self.quotas.write().await;
        let quota = quotas.get_mut(&user_id).ok_or("Quota not found")?;

        if quota.remaining_quota < amount {
            return Err("Insufficient quota".to_string());
        }

        quota.used_quota += amount;
        quota.remaining_quota -= amount;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quota_fetcher() {
        let fetcher = QuotaFetcher::new();

        fetcher.update(123, 1000, 200).await;
        let quota = fetcher.fetch(123).await.unwrap();

        assert_eq!(quota.total_quota, 1000);
        assert_eq!(quota.used_quota, 200);
        assert_eq!(quota.remaining_quota, 800);
    }
}
