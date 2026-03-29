//! Scheduled Test Plan Service

#![allow(dead_code)]

use crate::entity::{scheduled_test_plans, scheduled_test_results};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
pub struct CreateTestPlanRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    pub cron_expr: String,
    pub test_config: JsonValue,
    pub created_by: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTestPlanRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub cron_expr: Option<String>,
    pub test_config: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct TestPlanResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub cron_expr: String,
    pub test_config: JsonValue,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_result: Option<JsonValue>,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TestResultResponse {
    pub id: i64,
    pub plan_id: i64,
    pub plan_name: String,
    pub status: String,
    pub result: Option<JsonValue>,
    pub error_message: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl From<scheduled_test_plans::Model> for TestPlanResponse {
    fn from(model: scheduled_test_plans::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            enabled: model.enabled,
            cron_expr: model.cron_expr,
            test_config: model.test_config,
            last_run_at: model.last_run_at,
            next_run_at: model.next_run_at,
            last_result: model.last_result,
            created_by: model.created_by,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub struct ScheduledTestPlanService;

impl ScheduledTestPlanService {
    /// Create test plan
    pub async fn create(
        db: &DatabaseConnection,
        req: CreateTestPlanRequest,
    ) -> Result<TestPlanResponse> {
        let now = Utc::now();
        let plan = scheduled_test_plans::ActiveModel {
            id: ActiveValue::NotSet,
            name: ActiveValue::Set(req.name),
            description: ActiveValue::Set(req.description),
            enabled: ActiveValue::Set(req.enabled),
            cron_expr: ActiveValue::Set(req.cron_expr),
            test_config: ActiveValue::Set(req.test_config),
            last_run_at: ActiveValue::Set(None),
            next_run_at: ActiveValue::Set(None),
            last_result: ActiveValue::Set(None),
            created_by: ActiveValue::Set(req.created_by),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = plan.insert(db).await?;
        Ok(result.into())
    }

    /// List test plans
    pub async fn list(
        db: &DatabaseConnection,
        enabled_only: bool,
    ) -> Result<Vec<TestPlanResponse>> {
        let mut query = scheduled_test_plans::Entity::find();

        if enabled_only {
            query = query.filter(scheduled_test_plans::Column::Enabled.eq(true));
        }

        let results = query
            .order_by_desc(scheduled_test_plans::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Update test plan
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        req: UpdateTestPlanRequest,
    ) -> Result<Option<TestPlanResponse>> {
        let plan = scheduled_test_plans::Entity::find_by_id(id).one(db).await?;

        match plan {
            Some(model) => {
                let mut active_model: scheduled_test_plans::ActiveModel = model.into();

                if let Some(name) = req.name {
                    active_model.name = ActiveValue::Set(name);
                }
                if let Some(description) = req.description {
                    active_model.description = ActiveValue::Set(Some(description));
                }
                if let Some(enabled) = req.enabled {
                    active_model.enabled = ActiveValue::Set(enabled);
                }
                if let Some(cron_expr) = req.cron_expr {
                    active_model.cron_expr = ActiveValue::Set(cron_expr);
                }
                if let Some(test_config) = req.test_config {
                    active_model.test_config = ActiveValue::Set(test_config);
                }
                active_model.updated_at = ActiveValue::Set(Utc::now());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete test plan
    pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let result = scheduled_test_plans::Entity::delete_by_id(id)
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Record test result
    pub async fn record_result(
        db: &DatabaseConnection,
        plan_id: i64,
        status: &str,
        result: Option<JsonValue>,
        error_message: Option<String>,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        let now = Utc::now();

        // Create result record
        let test_result = scheduled_test_results::ActiveModel {
            id: ActiveValue::NotSet,
            plan_id: ActiveValue::Set(plan_id),
            status: ActiveValue::Set(status.to_string()),
            result: ActiveValue::Set(result.clone()),
            error_message: ActiveValue::Set(error_message.clone()),
            duration_ms: ActiveValue::Set(duration_ms),
            created_at: ActiveValue::Set(now),
        };
        test_result.insert(db).await?;

        // Update plan with last run info
        let plan = scheduled_test_plans::Entity::find_by_id(plan_id)
            .one(db)
            .await?;

        if let Some(model) = plan {
            let mut active_model: scheduled_test_plans::ActiveModel = model.into();
            active_model.last_run_at = ActiveValue::Set(Some(now));
            active_model.last_result = ActiveValue::Set(result);
            active_model.updated_at = ActiveValue::Set(now);
            active_model.update(db).await?;
        }

        Ok(())
    }

    /// Get test results for plan
    pub async fn get_results(
        db: &DatabaseConnection,
        plan_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<TestResultResponse>> {
        let plan = scheduled_test_plans::Entity::find_by_id(plan_id)
            .one(db)
            .await?;

        let plan_name = plan.map(|p| p.name).unwrap_or_default();

        let results = scheduled_test_results::Entity::find()
            .filter(scheduled_test_results::Column::PlanId.eq(plan_id))
            .order_by_desc(scheduled_test_results::Column::CreatedAt)
            .paginate(db, page_size)
            .fetch_page(page)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| TestResultResponse {
                id: r.id,
                plan_id: r.plan_id,
                plan_name: plan_name.clone(),
                status: r.status,
                result: r.result,
                error_message: r.error_message,
                duration_ms: r.duration_ms,
                created_at: r.created_at,
            })
            .collect())
    }
}
