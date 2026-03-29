//! Scheduled test runner service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Scheduled test
#[derive(Debug, Clone)]
pub struct ScheduledTest {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub test_type: String,
    pub is_active: bool,
    pub last_run: Option<i64>,
    pub next_run: Option<i64>,
}

/// Test run result
#[derive(Debug, Clone)]
pub struct TestRunResult {
    pub test_id: String,
    pub run_at: i64,
    pub passed: bool,
    pub duration_ms: u64,
    pub message: String,
}

/// Scheduled test runner service
pub struct ScheduledTestRunnerService {
    tests: Arc<RwLock<HashMap<String, ScheduledTest>>>,
    results: Arc<RwLock<Vec<TestRunResult>>>,
}

impl Default for ScheduledTestRunnerService {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduledTestRunnerService {
    pub fn new() -> Self {
        Self {
            tests: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn schedule_test(&self, test: ScheduledTest) {
        let mut tests = self.tests.write().await;
        tests.insert(test.id.clone(), test);
    }

    pub async fn run_test(&self, test_id: &str) -> Result<TestRunResult, String> {
        let tests = self.tests.read().await;
        let _test = tests.get(test_id).ok_or("Test not found")?;

        // Simulate test execution
        let result = TestRunResult {
            test_id: test_id.to_string(),
            run_at: chrono::Utc::now().timestamp(),
            passed: true,
            duration_ms: 100,
            message: "Test passed".to_string(),
        };

        let mut results = self.results.write().await;
        results.push(result.clone());

        Ok(result)
    }

    pub async fn list_scheduled_tests(&self) -> Vec<ScheduledTest> {
        let tests = self.tests.read().await;
        tests.values().cloned().collect()
    }

    pub async fn get_recent_results(&self, limit: usize) -> Vec<TestRunResult> {
        let results = self.results.read().await;
        results.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduled_runner() {
        let service = ScheduledTestRunnerService::new();

        service
            .schedule_test(ScheduledTest {
                id: "test-1".to_string(),
                name: "Health Check".to_string(),
                cron_expression: "0 * * * *".to_string(),
                test_type: "health".to_string(),
                is_active: true,
                last_run: None,
                next_run: None,
            })
            .await;

        let result = service.run_test("test-1").await.unwrap();
        assert!(result.passed);
    }
}
