//! Dashboard query service
//!
//! 将 admin dashboard 的聚合与图表查询从 handler 中下沉，避免控制器承担查询细节。

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, PaginatorTrait,
    QueryFilter, QuerySelect, Statement,
};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

use crate::entity::{accounts, api_keys, usages, users};
use crate::metrics::{
    self, BATCH_ERRORS, BATCH_OPERATIONS_TOTAL, BATCH_OPERATION_LAST_SIZE,
    BATCH_OPERATION_THROUGHPUT,
};
use crate::service::ops_metrics_collector::{MetricsCollector, MetricsCollectorConfig};

#[derive(Debug, Clone, Serialize)]
pub struct DashboardStats {
    pub users: UserStats,
    pub accounts: AccountStats,
    pub api_keys: ApiKeyStats,
    pub usage: UsageStats,
    pub ops: OpsStats,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserStats {
    pub total: i64,
    pub active: i64,
    pub new_today: i64,
    pub new_this_week: i64,
    pub new_this_month: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountStats {
    pub total: i64,
    pub active: i64,
    pub healthy: i64,
    pub by_platform: Vec<PlatformStats>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlatformStats {
    pub platform: String,
    pub count: i64,
    pub healthy_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyStats {
    pub total: i64,
    pub active: i64,
    pub expiring_soon: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageStats {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub today_requests: i64,
    pub today_tokens: i64,
    pub today_cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpsStats {
    pub active_users_24h: i64,
    pub error_rate_1h: f64,
    pub avg_response_time_ms: f64,
    pub cache_hit_rate: f64,
    pub batch_operations_total: u64,
    pub batch_errors_total: u64,
    pub latest_fast_import_throughput: f64,
    pub latest_fast_import_preview_throughput: f64,
    pub latest_fast_import_size: i64,
    pub latest_fast_import_preview_size: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct UsageTotals {
    pub total_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartData {
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartDataset {
    pub label: String,
    pub data: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "borderColor")]
    pub border_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "backgroundColor")]
    pub background_color: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DistributionData {
    pub labels: Vec<String>,
    pub data: Vec<i64>,
    pub total: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct DashboardDateRange {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

impl DashboardDateRange {
    pub fn parse(start_date: Option<&str>, end_date: Option<&str>) -> Result<Self> {
        let end_time = end_date
            .map(parse_day_end)
            .transpose()?
            .unwrap_or_else(Utc::now);
        let start_time = start_date
            .map(parse_day_start)
            .transpose()?
            .unwrap_or_else(|| start_of_day(end_time - Duration::days(6)));

        Self::new(start_time, end_time)
    }

    pub fn new(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Self> {
        if start_time > end_time {
            return Err(anyhow!("start_date must be less than or equal to end_date"));
        }

        Ok(Self {
            start_time,
            end_time,
        })
    }

    pub fn labels(&self) -> Vec<String> {
        build_labels(self.start_time, self.end_time)
    }
}

pub struct DashboardQueryService {
    db: DatabaseConnection,
}

impl DashboardQueryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        let now = Utc::now();
        let today_start = start_of_day(now);
        let week_start = start_of_day(now - Duration::days(6));
        let month_start = Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .ok_or_else(|| anyhow!("invalid month"))?;

        // --- Users: DB count queries ---
        let total_users = users::Entity::find().count(&self.db).await? as i64;
        let active_users = users::Entity::find()
            .filter(users::Column::Status.eq("active"))
            .count(&self.db)
            .await? as i64;
        let new_today = users::Entity::find()
            .filter(users::Column::CreatedAt.gte(today_start))
            .count(&self.db)
            .await? as i64;
        let new_this_week = users::Entity::find()
            .filter(users::Column::CreatedAt.gte(week_start))
            .count(&self.db)
            .await? as i64;
        let new_this_month = users::Entity::find()
            .filter(users::Column::CreatedAt.gte(month_start))
            .count(&self.db)
            .await? as i64;

        // --- Accounts: DB aggregation instead of full table scan ---
        let total_accounts = accounts::Entity::find().count(&self.db).await? as i64;
        let active_accounts = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .count(&self.db)
            .await? as i64;
        let healthy_accounts = accounts::Entity::find()
            .filter(accounts::Column::Status.eq("active"))
            .filter(accounts::Column::LastError.is_null())
            .count(&self.db)
            .await? as i64;

        // Platform stats via raw SQL GROUP BY
        #[derive(Debug, FromQueryResult)]
        struct PlatformRow {
            provider: String,
            total: i64,
            healthy: i64,
        }
        let platform_rows = PlatformRow::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT provider,
                      COUNT(*)::bigint AS total,
                      COUNT(*) FILTER (WHERE status = 'active' AND last_error IS NULL)::bigint AS healthy
               FROM accounts GROUP BY provider ORDER BY provider"#,
            [],
        ))
        .all(&self.db)
        .await?;
        let by_platform = platform_rows
            .into_iter()
            .map(|row| PlatformStats {
                platform: row.provider,
                count: row.total,
                healthy_count: row.healthy,
            })
            .collect();

        // --- API Keys: DB count queries instead of full table scan ---
        let total_api_keys = api_keys::Entity::find().count(&self.db).await? as i64;
        let active_api_keys = api_keys::Entity::find()
            .filter(api_keys::Column::Status.eq("active"))
            .count(&self.db)
            .await? as i64;
        let expiring_soon_at = now + Duration::days(7);
        let expiring_soon = api_keys::Entity::find()
            .filter(api_keys::Column::Status.eq("active"))
            .filter(api_keys::Column::ExpiresAt.gt(now))
            .filter(api_keys::Column::ExpiresAt.lte(expiring_soon_at))
            .count(&self.db)
            .await? as i64;

        // --- Usages: DB aggregation instead of full table scan ---
        #[derive(Debug, FromQueryResult)]
        struct UsageAgg {
            cnt: i64,
            sum_input: i64,
            sum_output: i64,
            sum_cost: i64,
        }
        let total_usage = UsageAgg::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT COUNT(*)::bigint AS cnt,
                      COALESCE(SUM(input_tokens), 0)::bigint AS sum_input,
                      COALESCE(SUM(output_tokens), 0)::bigint AS sum_output,
                      COALESCE(SUM(cost), 0)::bigint AS sum_cost
               FROM usages"#,
            [],
        ))
        .one(&self.db)
        .await?
        .unwrap_or(UsageAgg { cnt: 0, sum_input: 0, sum_output: 0, sum_cost: 0 });

        let today_usage = UsageAgg::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT COUNT(*)::bigint AS cnt,
                      COALESCE(SUM(input_tokens), 0)::bigint AS sum_input,
                      COALESCE(SUM(output_tokens), 0)::bigint AS sum_output,
                      COALESCE(SUM(cost), 0)::bigint AS sum_cost
               FROM usages WHERE created_at >= $1"#,
            [today_start.into()],
        ))
        .one(&self.db)
        .await?
        .unwrap_or(UsageAgg { cnt: 0, sum_input: 0, sum_output: 0, sum_cost: 0 });

        let usage = UsageStats {
            total_requests: total_usage.cnt,
            total_tokens: total_usage.sum_input + total_usage.sum_output,
            total_cost: total_usage.sum_cost as f64 / 100.0,
            today_requests: today_usage.cnt,
            today_tokens: today_usage.sum_input + today_usage.sum_output,
            today_cost: today_usage.sum_cost as f64 / 100.0,
        };

        let snapshot = MetricsCollector::new(self.db.clone(), MetricsCollectorConfig::default())
            .collect_snapshot()
            .await?;
        let ops = OpsStats {
            active_users_24h: snapshot.active_users_24h,
            error_rate_1h: snapshot.error_rate_1h,
            avg_response_time_ms: snapshot.avg_response_time_ms,
            cache_hit_rate: metrics::CacheMetrics::hit_rate(),
            batch_operations_total: BATCH_OPERATIONS_TOTAL.get(),
            batch_errors_total: BATCH_ERRORS.get(),
            latest_fast_import_throughput: BATCH_OPERATION_THROUGHPUT
                .with_label_values(&["fast_import", "fast"])
                .get(),
            latest_fast_import_preview_throughput: BATCH_OPERATION_THROUGHPUT
                .with_label_values(&["fast_import_preview", "preview"])
                .get(),
            latest_fast_import_size: BATCH_OPERATION_LAST_SIZE
                .with_label_values(&["fast_import", "fast"])
                .get(),
            latest_fast_import_preview_size: BATCH_OPERATION_LAST_SIZE
                .with_label_values(&["fast_import_preview", "preview"])
                .get(),
        };

        Ok(DashboardStats {
            users: UserStats {
                total: total_users,
                active: active_users,
                new_today,
                new_this_week,
                new_this_month,
            },
            accounts: AccountStats {
                total: total_accounts,
                active: active_accounts,
                healthy: healthy_accounts,
                by_platform,
            },
            api_keys: ApiKeyStats {
                total: total_api_keys,
                active: active_api_keys,
                expiring_soon,
            },
            usage,
            ops,
            updated_at: Utc::now(),
        })
    }

    pub async fn get_usage_totals(&self, range: DashboardDateRange) -> Result<UsageTotals> {
        let usages = self.load_usages(range).await?;
        Ok(summarize_usage_totals(&usages))
    }

    pub async fn get_trend_data(&self, range: DashboardDateRange) -> Result<ChartData> {
        let usages = self.load_usages(range).await?;
        let labels = range.labels();

        let mut requests_by_day = BTreeMap::<String, f64>::new();
        let mut tokens_by_day = BTreeMap::<String, f64>::new();
        let mut cost_by_day = BTreeMap::<String, f64>::new();

        for usage in usages {
            let day = usage.created_at.format("%Y-%m-%d").to_string();
            *requests_by_day.entry(day.clone()).or_insert(0.0) += 1.0;
            *tokens_by_day.entry(day.clone()).or_insert(0.0) +=
                (usage.input_tokens + usage.output_tokens) as f64;
            *cost_by_day.entry(day).or_insert(0.0) += usage.cost as f64 / 100.0;
        }

        Ok(ChartData {
            labels: labels.clone(),
            datasets: vec![
                ChartDataset {
                    label: "请求数".to_string(),
                    data: labels
                        .iter()
                        .map(|label| requests_by_day.get(label).copied().unwrap_or(0.0))
                        .collect(),
                    color: Some("#3b82f6".to_string()),
                    border_color: None,
                    background_color: None,
                    fill: None,
                },
                ChartDataset {
                    label: "Token 数".to_string(),
                    data: labels
                        .iter()
                        .map(|label| tokens_by_day.get(label).copied().unwrap_or(0.0))
                        .collect(),
                    color: Some("#10b981".to_string()),
                    border_color: None,
                    background_color: None,
                    fill: None,
                },
                ChartDataset {
                    label: "费用(元)".to_string(),
                    data: labels
                        .iter()
                        .map(|label| cost_by_day.get(label).copied().unwrap_or(0.0))
                        .collect(),
                    color: Some("#f59e0b".to_string()),
                    border_color: None,
                    background_color: None,
                    fill: None,
                },
            ],
        })
    }

    pub async fn get_line_chart_data(&self, range: DashboardDateRange) -> Result<ChartData> {
        let usages = self.load_usages(range).await?;
        let labels = range.labels();
        let mut response_time_by_day = BTreeMap::<String, (i64, i64)>::new();

        for usage in usages {
            let response_time_ms = usage
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("response_time_ms"))
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let day = usage.created_at.format("%Y-%m-%d").to_string();
            let entry = response_time_by_day.entry(day).or_insert((0, 0));
            entry.0 += response_time_ms;
            entry.1 += 1;
        }

        Ok(ChartData {
            labels: labels.clone(),
            datasets: vec![ChartDataset {
                label: "响应时间 (ms)".to_string(),
                data: labels
                    .iter()
                    .map(|label| match response_time_by_day.get(label) {
                        Some((total, count)) if *count > 0 => *total as f64 / *count as f64,
                        _ => 0.0,
                    })
                    .collect(),
                color: None,
                border_color: Some("#10b981".to_string()),
                background_color: None,
                fill: Some(false),
            }],
        })
    }

    pub async fn get_model_distribution(&self) -> Result<DistributionData> {
        #[derive(Debug, FromQueryResult)]
        struct ModelCount {
            model: String,
            cnt: i64,
        }
        let rows = ModelCount::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT model, COUNT(*)::bigint AS cnt
               FROM usages GROUP BY model ORDER BY cnt DESC"#,
            [],
        ))
        .all(&self.db)
        .await?;

        let total = rows.iter().map(|r| r.cnt).sum();
        Ok(DistributionData {
            labels: rows.iter().map(|r| r.model.clone()).collect(),
            data: rows.iter().map(|r| r.cnt).collect(),
            total,
        })
    }

    pub async fn get_platform_distribution(&self) -> Result<DistributionData> {
        #[derive(Debug, FromQueryResult)]
        struct ProviderCount {
            provider: String,
            cnt: i64,
        }
        let rows = ProviderCount::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT provider, COUNT(*)::bigint AS cnt
               FROM accounts GROUP BY provider ORDER BY cnt DESC"#,
            [],
        ))
        .all(&self.db)
        .await?;

        let total = rows.iter().map(|r| r.cnt).sum();
        Ok(DistributionData {
            labels: rows.iter().map(|r| r.provider.clone()).collect(),
            data: rows.iter().map(|r| r.cnt).collect(),
            total,
        })
    }

    pub async fn get_pie_chart_data(&self) -> Result<ChartData> {
        #[derive(Debug, FromQueryResult)]
        struct PieRow {
            success_count: i64,
            failure_count: i64,
            timeout_count: i64,
        }
        let row = PieRow::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT
                 COUNT(*) FILTER (WHERE success = true)::bigint AS success_count,
                 COUNT(*) FILTER (WHERE success = false AND (error_message IS NULL OR (LOWER(error_message) NOT LIKE '%timeout%' AND LOWER(error_message) NOT LIKE '%timed out%')))::bigint AS failure_count,
                 COUNT(*) FILTER (WHERE success = false AND (LOWER(error_message) LIKE '%timeout%' OR LOWER(error_message) LIKE '%timed out%'))::bigint AS timeout_count
               FROM usages"#,
            [],
        ))
        .one(&self.db)
        .await?
        .unwrap_or(PieRow { success_count: 0, failure_count: 0, timeout_count: 0 });

        Ok(ChartData {
            labels: vec!["成功".to_string(), "失败".to_string(), "超时".to_string()],
            datasets: vec![ChartDataset {
                label: "请求结果".to_string(),
                data: vec![
                    row.success_count as f64,
                    row.failure_count as f64,
                    row.timeout_count as f64,
                ],
                color: None,
                border_color: None,
                background_color: Some(JsonValue::Array(vec![
                    JsonValue::String("#10b981".to_string()),
                    JsonValue::String("#ef4444".to_string()),
                    JsonValue::String("#f59e0b".to_string()),
                ])),
                fill: None,
            }],
        })
    }

    async fn load_usages(&self, range: DashboardDateRange) -> Result<Vec<usages::Model>> {
        use sea_orm::QueryOrder;
        Ok(usages::Entity::find()
            .filter(usages::Column::CreatedAt.gte(range.start_time))
            .filter(usages::Column::CreatedAt.lte(range.end_time))
            .order_by_asc(usages::Column::CreatedAt)
            .limit(50_000)
            .all(&self.db)
            .await?)
    }

    /// LLM API 实时指标（从 Prometheus 内存指标读取）
    pub fn get_llm_metrics(&self) -> LlmMetrics {
        use crate::metrics;

        LlmMetrics {
            avg_ttft_seconds: 0.0, // TTFT 需要从 histogram 采样，暂用 0
            active_connections: metrics::ACTIVE_CONNECTIONS.get(),
            total_requests: metrics::REQUESTS_TOTAL.get() as i64,
            provider_health: Vec::new(), // 由前端从 /metrics 端点拉取
            queue_depths: Vec::new(),    // 由前端从 /metrics 端点拉取
            cache_hit_rate: metrics::CacheMetrics::hit_rate(),
        }
    }
}

/// LLM API 实时指标
#[derive(Debug, Clone, Serialize)]
pub struct LlmMetrics {
    pub avg_ttft_seconds: f64,
    pub active_connections: i64,
    pub total_requests: i64,
    pub provider_health: Vec<ProviderHealthStatus>,
    pub queue_depths: Vec<QueueDepthInfo>,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthStatus {
    pub provider: String,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueDepthInfo {
    pub provider: String,
    pub depth: i64,
}

fn summarize_usage_totals(usages: &[usages::Model]) -> UsageTotals {
    UsageTotals {
        total_requests: usages.len() as i64,
        total_input_tokens: usages.iter().map(|usage| usage.input_tokens).sum(),
        total_output_tokens: usages.iter().map(|usage| usage.output_tokens).sum(),
        total_cost: usages.iter().map(|usage| usage.cost).sum(),
    }
}

fn parse_day_start(value: &str) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid start date"))?;
    Ok(Utc.from_utc_datetime(&naive))
}

fn parse_day_end(value: &str) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    let naive = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| anyhow!("invalid end date"))?;
    Ok(Utc.from_utc_datetime(&naive))
}

fn start_of_day(value: DateTime<Utc>) -> DateTime<Utc> {
    let naive = value.date_naive().and_hms_opt(0, 0, 0).expect("valid time");
    Utc.from_utc_datetime(&naive)
}

fn build_labels(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Vec<String> {
    let mut labels = Vec::new();
    let mut current = start_time.date_naive();
    let end = end_time.date_naive();

    while current <= end {
        labels.push(current.format("%Y-%m-%d").to_string());
        current += chrono::Duration::days(1);
    }

    labels
}

#[cfg(test)]
mod tests {
    use super::DashboardDateRange;
    use chrono::{TimeZone, Utc};

    #[test]
    fn date_range_rejects_reversed_bounds() {
        let start_time = Utc.with_ymd_and_hms(2026, 4, 3, 0, 0, 0).single().unwrap();
        let end_time = Utc
            .with_ymd_and_hms(2026, 4, 2, 23, 59, 59)
            .single()
            .unwrap();

        let result = DashboardDateRange::new(start_time, end_time);

        assert!(result.is_err());
    }

    #[test]
    fn date_range_labels_include_both_bounds() {
        let start_time = Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).single().unwrap();
        let end_time = Utc
            .with_ymd_and_hms(2026, 4, 3, 23, 59, 59)
            .single()
            .unwrap();

        let range = DashboardDateRange::new(start_time, end_time).unwrap();

        assert_eq!(
            range.labels(),
            vec![
                "2026-04-01".to_string(),
                "2026-04-02".to_string(),
                "2026-04-03".to_string(),
            ]
        );
    }
}
