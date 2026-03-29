//! Account quota reset service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Quota reset record
#[derive(Debug, Clone)]
pub struct QuotaResetRecord {
    pub account_id: i64,
    pub previous_quota: u64,
    pub new_quota: u64,
    pub reset_at: i64,
    pub reason: String,
}

/// Account quota reset service
pub struct AccountQuotaResetService {
    records: Arc<RwLock<HashMap<i64, Vec<QuotaResetRecord>>>>,
    quotas: Arc<RwLock<HashMap<i64, u64>>>,
}

impl Default for AccountQuotaResetService {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountQuotaResetService {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_quota(&self, account_id: i64, quota: u64) {
        let mut quotas = self.quotas.write().await;
        quotas.insert(account_id, quota);
    }

    pub async fn reset_quota(
        &self,
        account_id: i64,
        new_quota: u64,
        reason: &str,
    ) -> QuotaResetRecord {
        let mut quotas = self.quotas.write().await;
        let previous = quotas.get(&account_id).copied().unwrap_or(0);

        quotas.insert(account_id, new_quota);

        let record = QuotaResetRecord {
            account_id,
            previous_quota: previous,
            new_quota,
            reset_at: chrono::Utc::now().timestamp(),
            reason: reason.to_string(),
        };

        let mut records = self.records.write().await;
        records
            .entry(account_id)
            .or_insert_with(Vec::new)
            .push(record.clone());

        record
    }

    pub async fn get_quota(&self, account_id: i64) -> Option<u64> {
        let quotas = self.quotas.read().await;
        quotas.get(&account_id).copied()
    }

    pub async fn get_reset_history(&self, account_id: i64) -> Vec<QuotaResetRecord> {
        let records = self.records.read().await;
        records.get(&account_id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quota_reset() {
        let service = AccountQuotaResetService::new();

        service.set_quota(123, 1000).await;
        let record = service.reset_quota(123, 2000, "Monthly reset").await;

        assert_eq!(record.previous_quota, 1000);
        assert_eq!(record.new_quota, 2000);

        let quota = service.get_quota(123).await.unwrap();
        assert_eq!(quota, 2000);
    }
}
