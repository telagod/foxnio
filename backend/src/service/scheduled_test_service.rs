//! Scheduled Test Execution Service
//!
//! Finds active test plans due for execution, runs HTTP checks,
//! and records results in the database.

#![allow(dead_code)]

use crate::entity::{scheduled_test_plans, scheduled_test_results};
use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Outcome of a single test execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub plan_id: i64,
    pub success: bool,
    pub latency_ms: i64,
    pub error: Option<String>,
}

/// Parsed test configuration stored in `test_config` JSON.
#[derive(Debug, Clone, Deserialize)]
struct TestConfig {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_expected_status")]
    expected_status: u16,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
    headers: Option<std::collections::HashMap<String, String>>,
    body: Option<serde_json::Value>,
}

fn default_method() -> String {
    "GET".to_string()
}
fn default_expected_status() -> u16 {
    200
}
fn default_timeout_ms() -> u64 {
    10_000
}

pub struct ScheduledTestService {
    db: DatabaseConnection,
    http: reqwest::Client,
}

impl ScheduledTestService {
    pub fn new(db: DatabaseConnection) -> Self {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("failed to build reqwest client");
        Self { db, http }
    }

    /// Find all active plans whose `next_run_at <= now` and execute them.
    pub async fn run_pending_tests(&self) -> Result<Vec<TestResult>> {
        let now = Utc::now();

        let plans = scheduled_test_plans::Entity::find()
            .filter(scheduled_test_plans::Column::Enabled.eq(true))
            .filter(scheduled_test_plans::Column::NextRunAt.lte(now))
            .all(&self.db)
            .await
            .context("query pending test plans")?;

        let mut results = Vec::with_capacity(plans.len());
        for plan in &plans {
            let result = self.execute_test(plan).await;
            match result {
                Ok(r) => results.push(r),
                Err(e) => {
                    tracing::error!(plan_id = plan.id, "test execution failed: {e:#}");
                    results.push(TestResult {
                        plan_id: plan.id,
                        success: false,
                        latency_ms: 0,
                        error: Some(format!("{e:#}")),
                    });
                }
            }
        }
        Ok(results)
    }

    /// Execute a single test plan: HTTP request, compare, record.
    pub async fn execute_test(
        &self,
        plan: &scheduled_test_plans::Model,
    ) -> Result<TestResult> {
        let config: TestConfig = serde_json::from_value(plan.test_config.clone())
            .context("parse test_config")?;

        let timeout = std::time::Duration::from_millis(config.timeout_ms);

        // Build request
        let method: reqwest::Method = config
            .method
            .to_uppercase()
            .parse()
            .unwrap_or(reqwest::Method::GET);

        let mut req = self.http.request(method, &config.url).timeout(timeout);

        if let Some(headers) = &config.headers {
            for (k, v) in headers {
                req = req.header(k.as_str(), v.as_str());
            }
        }
        if let Some(body) = &config.body {
            req = req.json(body);
        }

        // Execute and measure
        let start = Instant::now();
        let response = req.send().await;
        let latency_ms = start.elapsed().as_millis() as i64;

        let (success, error) = match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == config.expected_status {
                    (true, None)
                } else {
                    (
                        false,
                        Some(format!(
                            "expected status {}, got {}",
                            config.expected_status, status
                        )),
                    )
                }
            }
            Err(e) => (false, Some(format!("{e:#}"))),
        };

        let status_str = if success { "success" } else { "failed" };

        // Record result in DB
        let result_json = serde_json::json!({
            "success": success,
            "latency_ms": latency_ms,
        });

        self.record_result(
            plan.id,
            status_str,
            Some(result_json),
            error.clone(),
            Some(latency_ms),
        )
        .await?;

        Ok(TestResult {
            plan_id: plan.id,
            success,
            latency_ms,
            error,
        })
    }

    /// Query historical results for a plan.
    pub async fn get_results(
        &self,
        plan_id: i64,
        limit: u64,
    ) -> Result<Vec<scheduled_test_results::Model>> {
        let results = scheduled_test_results::Entity::find()
            .filter(scheduled_test_results::Column::PlanId.eq(plan_id))
            .order_by_desc(scheduled_test_results::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .context("query test results")?;
        Ok(results)
    }

    // ── internal ──

    async fn record_result(
        &self,
        plan_id: i64,
        status: &str,
        result: Option<serde_json::Value>,
        error_message: Option<String>,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        let now = Utc::now();

        let record = scheduled_test_results::ActiveModel {
            id: ActiveValue::NotSet,
            plan_id: ActiveValue::Set(plan_id),
            status: ActiveValue::Set(status.to_string()),
            result: ActiveValue::Set(result.clone()),
            error_message: ActiveValue::Set(error_message),
            duration_ms: ActiveValue::Set(duration_ms),
            created_at: ActiveValue::Set(now),
        };
        record.insert(&self.db).await.context("insert test result")?;

        // Update plan last_run_at / last_result
        let plan = scheduled_test_plans::Entity::find_by_id(plan_id)
            .one(&self.db)
            .await?;

        if let Some(model) = plan {
            let mut am: scheduled_test_plans::ActiveModel = model.into();
            am.last_run_at = ActiveValue::Set(Some(now));
            am.last_result = ActiveValue::Set(result);
            am.updated_at = ActiveValue::Set(now);
            am.update(&self.db).await.context("update plan after run")?;
        }

        Ok(())
    }
}
