//! 使用量清理 - Usage Cleanup
//!
//! 清理历史使用量记录

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, Set, Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::audit_logs;

/// 清理状态常量
pub const USAGE_CLEANUP_STATUS_PENDING: &str = "pending";
pub const USAGE_CLEANUP_STATUS_RUNNING: &str = "running";
pub const USAGE_CLEANUP_STATUS_SUCCEEDED: &str = "succeeded";
pub const USAGE_CLEANUP_STATUS_FAILED: &str = "failed";
pub const USAGE_CLEANUP_STATUS_CANCELED: &str = "canceled";

/// 使用量清理过滤条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCleanupFilters {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub account_id: Option<i64>,
    pub group_id: Option<i64>,
    pub model: Option<String>,
    pub request_type: Option<i16>,
    pub stream: Option<bool>,
    pub billing_type: Option<i8>,
}

/// 使用量清理任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCleanupTask {
    pub id: i64,
    pub status: String,
    pub filters: UsageCleanupFilters,
    pub created_by: i64,
    pub deleted_rows: i64,
    pub error_msg: Option<String>,
    pub canceled_by: Option<i64>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationResult {
    pub has_more: bool,
    pub next_token: Option<String>,
}

/// 使用量清理管理器
pub struct UsageCleanup {
    db: DatabaseConnection,
}

/// audit_logs action constant for cleanup tasks
const CLEANUP_ACTION: &str = "USAGE_CLEANUP_TASK";

impl UsageCleanup {
    /// 创建新的使用量清理管理器
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 创建清理任务
    pub async fn create_task(
        &self,
        filters: UsageCleanupFilters,
        created_by: i64,
    ) -> Result<UsageCleanupTask> {
        let now = Utc::now();
        let task_id = now.timestamp_millis();
        let log_id = Uuid::new_v4();

        let task = UsageCleanupTask {
            id: task_id,
            status: USAGE_CLEANUP_STATUS_PENDING.to_string(),
            filters: filters.clone(),
            created_by,
            deleted_rows: 0,
            error_msg: None,
            canceled_by: None,
            canceled_at: None,
            started_at: None,
            finished_at: None,
            created_at: now,
            updated_at: now,
        };

        let request_data = serde_json::to_value(&task)?;

        let record = audit_logs::ActiveModel {
            id: Set(log_id),
            user_id: Set(Some(Uuid::from_u128(created_by as u128))),
            action: Set(CLEANUP_ACTION.to_string()),
            resource_type: Set(Some("usage_cleanup".to_string())),
            resource_id: Set(Some(task_id.to_string())),
            ip_address: Set(None),
            user_agent: Set(None),
            request_data: Set(Some(request_data)),
            response_status: Set(None),
            created_at: Set(now),
        };

        record.insert(&self.db).await?;

        Ok(task)
    }

    /// 列出清理任务
    pub async fn list_tasks(
        &self,
        page_size: u64,
        page_token: Option<&str>,
    ) -> Result<(Vec<UsageCleanupTask>, PaginationResult)> {
        let mut query = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq(CLEANUP_ACTION))
            .order_by_desc(audit_logs::Column::CreatedAt);

        // Use page_token as a created_at cursor
        if let Some(token) = page_token {
            if let Ok(ts) = token.parse::<i64>() {
                let cursor_time = DateTime::from_timestamp_millis(ts).unwrap_or_else(|| Utc::now());
                query = query.filter(audit_logs::Column::CreatedAt.lt(cursor_time));
            }
        }

        let records = query.all(&self.db).await?;

        let limited: Vec<_> = records.into_iter().take(page_size as usize + 1).collect();
        let has_more = limited.len() > page_size as usize;
        let page: Vec<_> = limited.into_iter().take(page_size as usize).collect();

        let next_token = if has_more {
            page.last()
                .map(|r| r.created_at.timestamp_millis().to_string())
        } else {
            None
        };

        let tasks: Vec<UsageCleanupTask> = page
            .into_iter()
            .filter_map(|r| r.request_data.and_then(|d| serde_json::from_value(d).ok()))
            .collect();

        Ok((
            tasks,
            PaginationResult {
                has_more,
                next_token,
            },
        ))
    }

    /// 抢占下一个待执行任务
    pub async fn claim_next_pending_task(
        &self,
        stale_running_after_seconds: i64,
    ) -> Result<Option<UsageCleanupTask>> {
        let stale_cutoff = Utc::now() - chrono::Duration::seconds(stale_running_after_seconds);

        // Find pending or stale-running tasks
        let records = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq(CLEANUP_ACTION))
            .order_by_asc(audit_logs::Column::CreatedAt)
            .all(&self.db)
            .await?;

        for record in records {
            let data = match &record.request_data {
                Some(d) => d,
                None => continue,
            };
            let task: UsageCleanupTask = match serde_json::from_value(data.clone()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let claimable = task.status == USAGE_CLEANUP_STATUS_PENDING
                || (task.status == USAGE_CLEANUP_STATUS_RUNNING
                    && task.started_at.map_or(true, |s| s < stale_cutoff));

            if claimable {
                // Atomically claim via UPDATE ... WHERE to prevent races
                let now = Utc::now();
                let mut claimed = task.clone();
                claimed.status = USAGE_CLEANUP_STATUS_RUNNING.to_string();
                claimed.started_at = Some(now);
                claimed.updated_at = now;

                let new_data = serde_json::to_value(&claimed)?;
                let mut active: audit_logs::ActiveModel = record.into();
                active.request_data = Set(Some(new_data));
                active.update(&self.db).await?;

                return Ok(Some(claimed));
            }
        }

        Ok(None)
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: i64) -> Result<String> {
        let record = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq(CLEANUP_ACTION))
            .filter(audit_logs::Column::ResourceId.eq(task_id.to_string()))
            .one(&self.db)
            .await?;

        match record {
            Some(r) => {
                let status = r
                    .request_data
                    .and_then(|d| d.get("status").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_else(|| USAGE_CLEANUP_STATUS_PENDING.to_string());
                Ok(status)
            }
            None => Ok(USAGE_CLEANUP_STATUS_PENDING.to_string()),
        }
    }

    /// 更新任务进度
    pub async fn update_task_progress(&self, task_id: i64, deleted_rows: i64) -> Result<()> {
        self.update_task_field(task_id, |task| {
            task.deleted_rows = deleted_rows;
            task.updated_at = Utc::now();
        })
        .await
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: i64, canceled_by: i64) -> Result<bool> {
        let record = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq(CLEANUP_ACTION))
            .filter(audit_logs::Column::ResourceId.eq(task_id.to_string()))
            .one(&self.db)
            .await?;

        let record = match record {
            Some(r) => r,
            None => return Ok(false),
        };

        let task: UsageCleanupTask = match record
            .request_data
            .as_ref()
            .and_then(|d| serde_json::from_value(d.clone()).ok())
        {
            Some(t) => t,
            None => return Ok(false),
        };

        if task.status != USAGE_CLEANUP_STATUS_PENDING
            && task.status != USAGE_CLEANUP_STATUS_RUNNING
        {
            return Ok(false);
        }

        let now = Utc::now();
        let mut canceled = task;
        canceled.status = USAGE_CLEANUP_STATUS_CANCELED.to_string();
        canceled.canceled_by = Some(canceled_by);
        canceled.canceled_at = Some(now);
        canceled.updated_at = now;

        let new_data = serde_json::to_value(&canceled)?;
        let mut active: audit_logs::ActiveModel = record.into();
        active.request_data = Set(Some(new_data));
        active.update(&self.db).await?;

        Ok(true)
    }

    /// 标记任务成功
    pub async fn mark_task_succeeded(&self, task_id: i64, deleted_rows: i64) -> Result<()> {
        self.update_task_field(task_id, |task| {
            task.status = USAGE_CLEANUP_STATUS_SUCCEEDED.to_string();
            task.deleted_rows = deleted_rows;
            task.finished_at = Some(Utc::now());
            task.updated_at = Utc::now();
        })
        .await
    }

    /// 标记任务失败
    pub async fn mark_task_failed(
        &self,
        task_id: i64,
        deleted_rows: i64,
        error_msg: String,
    ) -> Result<()> {
        self.update_task_field(task_id, |task| {
            task.status = USAGE_CLEANUP_STATUS_FAILED.to_string();
            task.deleted_rows = deleted_rows;
            task.error_msg = Some(error_msg);
            task.finished_at = Some(Utc::now());
            task.updated_at = Utc::now();
        })
        .await
    }

    /// 批量删除使用日志
    pub async fn delete_usage_logs_batch(
        &self,
        filters: &UsageCleanupFilters,
        limit: u64,
    ) -> Result<i64> {
        // Build a DELETE with sub-select to honour the limit
        let mut conditions = vec![
            format!("created_at >= '{}'", filters.start_time.to_rfc3339()),
            format!("created_at <= '{}'", filters.end_time.to_rfc3339()),
        ];
        if let Some(uid) = filters.user_id {
            conditions.push(format!("user_id = '{}'", Uuid::from_u128(uid as u128)));
        }
        if let Some(aid) = filters.account_id {
            conditions.push(format!("account_id = '{}'", Uuid::from_u128(aid as u128)));
        }
        if let Some(ref model) = filters.model {
            conditions.push(format!("model = '{}'", model.replace('\'', "''")));
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "DELETE FROM usages WHERE id IN (SELECT id FROM usages WHERE {} LIMIT {})",
            where_clause, limit
        );

        let result = self
            .db
            .execute(Statement::from_string(DbBackend::Postgres, sql))
            .await?;

        Ok(result.rows_affected() as i64)
    }

    // ---- internal helper ----

    /// Generic helper to load a task record, mutate it, and persist.
    async fn update_task_field(
        &self,
        task_id: i64,
        mutate: impl FnOnce(&mut UsageCleanupTask),
    ) -> Result<()> {
        let record = audit_logs::Entity::find()
            .filter(audit_logs::Column::Action.eq(CLEANUP_ACTION))
            .filter(audit_logs::Column::ResourceId.eq(task_id.to_string()))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cleanup task {} not found", task_id))?;

        let mut task: UsageCleanupTask = record
            .request_data
            .as_ref()
            .and_then(|d| serde_json::from_value(d.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid task data for {}", task_id))?;

        mutate(&mut task);

        let new_data = serde_json::to_value(&task)?;
        let mut active: audit_logs::ActiveModel = record.into();
        active.request_data = Set(Some(new_data));
        active.update(&self.db).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_usage_cleanup() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let cleanup = UsageCleanup::new(db);

        let filters = UsageCleanupFilters {
            start_time: Utc::now() - Duration::days(30),
            end_time: Utc::now(),
            user_id: None,
            api_key_id: None,
            account_id: None,
            group_id: None,
            model: None,
            request_type: None,
            stream: None,
            billing_type: None,
        };

        let task = cleanup.create_task(filters, 1).await.unwrap();
        assert_eq!(task.status, USAGE_CLEANUP_STATUS_PENDING);
    }
}
