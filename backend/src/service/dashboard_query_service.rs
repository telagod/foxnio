//! Dashboard query service
//!
//! 将 admin dashboard 的聚合与图表查询从 handler 中下沉，避免控制器承担查询细节。

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

use crate::entity::{accounts, api_keys, usages, users};

#[derive(Debug, Clone, Serialize)]
pub struct DashboardStats {
    pub users: UserStats,
    pub accounts: AccountStats,
    pub api_keys: ApiKeyStats,
    pub usage: UsageStats,
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

        let all_accounts = accounts::Entity::find().all(&self.db).await?;
        let total_accounts = all_accounts.len() as i64;
        let active_accounts = all_accounts
            .iter()
            .filter(|account| account.is_active())
            .count() as i64;
        let healthy_accounts = all_accounts
            .iter()
            .filter(|account| account.is_active() && account.last_error.is_none())
            .count() as i64;

        let mut platform_map = BTreeMap::<String, PlatformStats>::new();
        for account in &all_accounts {
            let entry = platform_map
                .entry(account.provider.clone())
                .or_insert_with(|| PlatformStats {
                    platform: account.provider.clone(),
                    count: 0,
                    healthy_count: 0,
                });
            entry.count += 1;
            if account.is_active() && account.last_error.is_none() {
                entry.healthy_count += 1;
            }
        }

        let expiring_soon_at = now + Duration::days(7);
        let all_api_keys = api_keys::Entity::find().all(&self.db).await?;
        let total_api_keys = all_api_keys.len() as i64;
        let active_api_keys = all_api_keys.iter().filter(|key| key.is_active()).count() as i64;
        let expiring_soon = all_api_keys
            .iter()
            .filter(|key| key.status == "active")
            .filter_map(|key| key.expires_at)
            .filter(|expires_at| *expires_at > now && *expires_at <= expiring_soon_at)
            .count() as i64;

        let all_usages = usages::Entity::find().all(&self.db).await?;
        let usage = summarize_usage(&all_usages, today_start);

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
                by_platform: platform_map.into_values().collect(),
            },
            api_keys: ApiKeyStats {
                total: total_api_keys,
                active: active_api_keys,
                expiring_soon,
            },
            usage,
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
        let usages = usages::Entity::find().all(&self.db).await?;
        let mut counts = BTreeMap::<String, i64>::new();

        for usage in usages {
            *counts.entry(usage.model).or_insert(0) += 1;
        }

        Ok(DistributionData {
            labels: counts.keys().cloned().collect(),
            data: counts.values().copied().collect(),
            total: counts.values().sum(),
        })
    }

    pub async fn get_platform_distribution(&self) -> Result<DistributionData> {
        let accounts = accounts::Entity::find().all(&self.db).await?;
        let mut counts = BTreeMap::<String, i64>::new();

        for account in accounts {
            *counts.entry(account.provider).or_insert(0) += 1;
        }

        Ok(DistributionData {
            labels: counts.keys().cloned().collect(),
            data: counts.values().copied().collect(),
            total: counts.values().sum(),
        })
    }

    pub async fn get_pie_chart_data(&self) -> Result<ChartData> {
        let usages = usages::Entity::find().all(&self.db).await?;
        let mut success = 0.0;
        let mut failure = 0.0;
        let mut timeout = 0.0;

        for usage in usages {
            if usage.success {
                success += 1.0;
                continue;
            }

            let is_timeout = usage
                .error_message
                .as_deref()
                .map(|message| {
                    let message = message.to_ascii_lowercase();
                    message.contains("timeout") || message.contains("timed out")
                })
                .unwrap_or(false);

            if is_timeout {
                timeout += 1.0;
            } else {
                failure += 1.0;
            }
        }

        Ok(ChartData {
            labels: vec!["成功".to_string(), "失败".to_string(), "超时".to_string()],
            datasets: vec![ChartDataset {
                label: "请求结果".to_string(),
                data: vec![success, failure, timeout],
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
        Ok(usages::Entity::find()
            .filter(usages::Column::CreatedAt.gte(range.start_time))
            .filter(usages::Column::CreatedAt.lte(range.end_time))
            .all(&self.db)
            .await?)
    }
}

fn summarize_usage(usages: &[usages::Model], today_start: DateTime<Utc>) -> UsageStats {
    let today_usages = usages
        .iter()
        .filter(|usage| usage.created_at >= today_start)
        .collect::<Vec<_>>();
    let totals = summarize_usage_totals(usages);
    let today_totals =
        summarize_usage_totals(&today_usages.iter().copied().cloned().collect::<Vec<_>>());

    UsageStats {
        total_requests: totals.total_requests,
        total_tokens: totals.total_input_tokens + totals.total_output_tokens,
        total_cost: totals.total_cost as f64 / 100.0,
        today_requests: today_totals.total_requests,
        today_tokens: today_totals.total_input_tokens + today_totals.total_output_tokens,
        today_cost: today_totals.total_cost as f64 / 100.0,
    }
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
