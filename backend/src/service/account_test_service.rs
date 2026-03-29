//! Account test service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Account test result
#[derive(Debug, Clone)]
pub struct AccountTestResult {
    pub account_id: i64,
    pub test_type: String,
    pub passed: bool,
    pub message: String,
    pub tested_at: i64,
}

/// Account test service
pub struct AccountTestService {
    results: Arc<RwLock<HashMap<i64, Vec<AccountTestResult>>>>,
}

impl Default for AccountTestService {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountTestService {
    pub fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run_test(&self, account_id: i64, test_type: &str) -> AccountTestResult {
        // Simulate test
        let result = AccountTestResult {
            account_id,
            test_type: test_type.to_string(),
            passed: true,
            message: "Test passed".to_string(),
            tested_at: chrono::Utc::now().timestamp(),
        };

        let mut results = self.results.write().await;
        results
            .entry(account_id)
            .or_insert_with(Vec::new)
            .push(result.clone());

        result
    }

    pub async fn get_results(&self, account_id: i64) -> Vec<AccountTestResult> {
        let results = self.results.read().await;
        results.get(&account_id).cloned().unwrap_or_default()
    }

    pub async fn clear_results(&self, account_id: i64) {
        let mut results = self.results.write().await;
        results.remove(&account_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_account_test() {
        let service = AccountTestService::new();

        let result = service.run_test(123, "connection").await;
        assert!(result.passed);

        let results = service.get_results(123).await;
        assert_eq!(results.len(), 1);
    }
}
