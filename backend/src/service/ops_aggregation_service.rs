//! 运维聚合服务 - Ops Aggregation Service
//!
//! 定期预聚合运维指标（小时/天级别），用于稳定的长时间范围仪表板查询

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 小时聚合任务名称
const OPS_AGG_HOURLY_JOB_NAME: &str = "ops_preaggregation_hourly";
/// 天聚合任务名称
const OPS_AGG_DAILY_JOB_NAME: &str = "ops_preaggregation_daily";

/// 小时聚合间隔
const OPS_AGG_HOURLY_INTERVAL_SECS: i64 = 600; // 10分钟
/// 天聚合间隔
const OPS_AGG_DAILY_INTERVAL_SECS: i64 = 3600; // 1小时

/// 小时聚合重叠窗口（吸收延迟到达的数据）
const OPS_AGG_HOURLY_OVERLAP_SECS: i64 = 7200; // 2小时
/// 天聚合重叠窗口
const OPS_AGG_DAILY_OVERLAP_SECS: i64 = 172800; // 48小时

/// 小时聚合块大小
const OPS_AGG_HOURLY_CHUNK_SECS: i64 = 86400; // 24小时
/// 天聚合块大小
const OPS_AGG_DAILY_CHUNK_SECS: i64 = 604800; // 7天

/// 边界安全延迟（避免聚合仍在接收数据的桶）
const OPS_AGG_SAFE_DELAY_SECS: i64 = 300; // 5分钟

/// 聚合指标类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsAggregatedMetrics {
    pub timestamp: DateTime<Utc>,
    pub granularity: String, // "hourly" or "daily"
    pub platform: Option<String>,
    pub model: Option<String>,
    pub request_type: Option<i16>,
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub total_response_time_ms: i64,
    pub avg_response_time_ms: f64,
    pub p50_response_time_ms: i64,
    pub p95_response_time_ms: i64,
    pub p99_response_time_ms: i64,
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_cost_usd: f64,
}

/// 聚合任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationTaskState {
    pub job_name: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub is_running: bool,
    pub last_error: Option<String>,
    pub processed_count: i64,
}

/// 领导锁信息
#[derive(Debug, Clone)]
struct LeaderLock {
    instance_id: String,
    acquired_at: DateTime<Utc>,
    ttl_secs: i64,
}

/// 运维聚合服务配置
#[derive(Debug, Clone)]
pub struct OpsAggregationConfig {
    pub enabled: bool,
    pub hourly_interval_secs: i64,
    pub daily_interval_secs: i64,
    pub leader_lock_ttl_secs: i64,
    pub backfill_window_secs: i64,
}

impl Default for OpsAggregationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hourly_interval_secs: OPS_AGG_HOURLY_INTERVAL_SECS,
            daily_interval_secs: OPS_AGG_DAILY_INTERVAL_SECS,
            leader_lock_ttl_secs: 900,  // 15分钟
            backfill_window_secs: 3600, // 1小时
        }
    }
}

/// 运维聚合服务
pub struct OpsAggregationService {
    db: sea_orm::DatabaseConnection,
    redis: Option<Arc<redis::Client>>,
    config: OpsAggregationConfig,
    instance_id: String,

    // 状态管理
    hourly_state: Arc<RwLock<AggregationTaskState>>,
    daily_state: Arc<RwLock<AggregationTaskState>>,
    leader_lock: Arc<RwLock<Option<LeaderLock>>>,

    // 停止信号
    stop_signal: Arc<RwLock<bool>>,
}

impl OpsAggregationService {
    /// 创建新的聚合服务实例
    pub fn new(
        db: sea_orm::DatabaseConnection,
        redis: Option<Arc<redis::Client>>,
        config: OpsAggregationConfig,
    ) -> Self {
        let instance_id = uuid::Uuid::new_v4().to_string();

        let hourly_state = AggregationTaskState {
            job_name: OPS_AGG_HOURLY_JOB_NAME.to_string(),
            last_run_at: None,
            next_run_at: None,
            is_running: false,
            last_error: None,
            processed_count: 0,
        };

        let daily_state = AggregationTaskState {
            job_name: OPS_AGG_DAILY_JOB_NAME.to_string(),
            last_run_at: None,
            next_run_at: None,
            is_running: false,
            last_error: None,
            processed_count: 0,
        };

        Self {
            db,
            redis,
            config,
            instance_id,
            hourly_state: Arc::new(RwLock::new(hourly_state)),
            daily_state: Arc::new(RwLock::new(daily_state)),
            leader_lock: Arc::new(RwLock::new(None)),
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动聚合服务
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("运维聚合服务已禁用");
            return Ok(());
        }

        tracing::info!("启动运维聚合服务，实例ID: {}", self.instance_id);

        // 尝试获取领导锁
        if self.try_acquire_leader_lock().await? {
            tracing::info!("成功获取领导锁，开始聚合任务");

            // 启动小时聚合循环
            self.start_hourly_aggregation().await?;

            // 启动天聚合循环
            self.start_daily_aggregation().await?;
        } else {
            tracing::info!("未能获取领导锁，作为备用实例运行");
        }

        Ok(())
    }

    /// 停止聚合服务
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("停止运维聚合服务");

        // 设置停止信号
        let mut stop = self.stop_signal.write().await;
        *stop = true;

        // 释放领导锁
        self.release_leader_lock().await?;

        Ok(())
    }

    /// 尝试获取领导锁
    async fn try_acquire_leader_lock(&self) -> Result<bool> {
        let Some(_redis) = &self.redis else {
            // 没有 Redis 时，单实例模式
            return Ok(true);
        };

        // TODO: Redis leader lock implementation
        // Currently disabled due to Rust compiler issue with redis async
        Ok(true)
    }

    /// 释放领导锁
    async fn release_leader_lock(&self) -> Result<()> {
        let Some(_redis) = &self.redis else {
            return Ok(());
        };

        // TODO: Redis leader lock release implementation
        // Currently disabled due to Rust compiler issue with redis async
        let mut lock = self.leader_lock.write().await;
        *lock = None;

        Ok(())
    }

    /// 启动小时聚合循环
    async fn start_hourly_aggregation(&self) -> Result<()> {
        let interval = self.config.hourly_interval_secs;
        let mut interval_timer =
            tokio::time::interval(std::time::Duration::from_secs(interval as u64));

        loop {
            // 检查停止信号
            if *self.stop_signal.read().await {
                break;
            }

            interval_timer.tick().await;

            // 执行小时聚合
            if let Err(e) = self.run_hourly_aggregation().await {
                tracing::error!("小时聚合失败: {}", e);

                let mut state = self.hourly_state.write().await;
                state.last_error = Some(e.to_string());
            }
        }

        Ok(())
    }

    /// 启动天聚合循环
    async fn start_daily_aggregation(&self) -> Result<()> {
        let interval = self.config.daily_interval_secs;
        let mut interval_timer =
            tokio::time::interval(std::time::Duration::from_secs(interval as u64));

        loop {
            // 检查停止信号
            if *self.stop_signal.read().await {
                break;
            }

            interval_timer.tick().await;

            // 执行天聚合
            if let Err(e) = self.run_daily_aggregation().await {
                tracing::error!("天聚合失败: {}", e);

                let mut state = self.daily_state.write().await;
                state.last_error = Some(e.to_string());
            }
        }

        Ok(())
    }

    /// 执行小时聚合
    pub async fn run_hourly_aggregation(&self) -> Result<i64> {
        tracing::info!("开始执行小时聚合");

        let now = Utc::now();
        let end_time = now - Duration::seconds(OPS_AGG_SAFE_DELAY_SECS);
        let start_time = end_time - Duration::seconds(OPS_AGG_HOURLY_CHUNK_SECS);

        let metrics = self
            .aggregate_metrics(start_time, end_time, "hourly")
            .await?;

        let count = metrics.len() as i64;

        // 更新状态
        let mut state = self.hourly_state.write().await;
        state.last_run_at = Some(now);
        state.next_run_at = Some(now + Duration::seconds(self.config.hourly_interval_secs));
        state.is_running = false;
        state.processed_count += count;

        tracing::info!("小时聚合完成，处理 {} 条记录", count);

        Ok(count)
    }

    /// 执行天聚合
    pub async fn run_daily_aggregation(&self) -> Result<i64> {
        tracing::info!("开始执行天聚合");

        let now = Utc::now();
        let end_time = now - Duration::seconds(OPS_AGG_SAFE_DELAY_SECS);
        let start_time = end_time - Duration::seconds(OPS_AGG_DAILY_CHUNK_SECS);

        let metrics = self
            .aggregate_metrics(start_time, end_time, "daily")
            .await?;

        let count = metrics.len() as i64;

        // 更新状态
        let mut state = self.daily_state.write().await;
        state.last_run_at = Some(now);
        state.next_run_at = Some(now + Duration::seconds(self.config.daily_interval_secs));
        state.is_running = false;
        state.processed_count += count;

        tracing::info!("天聚合完成，处理 {} 条记录", count);

        Ok(count)
    }

    /// 聚合指标数据
    async fn aggregate_metrics(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        granularity: &str,
    ) -> Result<Vec<OpsAggregatedMetrics>> {
        // date_trunc bucket: 'hour' for hourly, 'day' for daily
        let trunc_unit = match granularity {
            "hourly" => "hour",
            "daily" => "day",
            other => other,
        };

        let sql = format!(
            r#"
            SELECT
                date_trunc('{trunc_unit}', u.created_at) AS bucket,
                a.provider                                AS platform,
                u.model                                   AS model,
                COUNT(*)                                  AS total_requests,
                COALESCE(SUM(CASE WHEN u.success THEN 1 ELSE 0 END), 0)  AS successful_requests,
                COALESCE(SUM(CASE WHEN NOT u.success THEN 1 ELSE 0 END), 0) AS failed_requests,
                COALESCE(SUM(u.input_tokens + u.output_tokens), 0)        AS total_tokens,
                COALESCE(SUM(u.input_tokens), 0)          AS prompt_tokens,
                COALESCE(SUM(u.output_tokens), 0)         AS completion_tokens,
                COALESCE(SUM(u.cost), 0)                  AS total_cost
            FROM usages u
            LEFT JOIN accounts a ON a.id = u.account_id
            WHERE u.created_at >= $1
              AND u.created_at <  $2
            GROUP BY bucket, a.provider, u.model
            ORDER BY bucket, platform, model
            "#
        );

        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                &sql,
                [start_time.into(), end_time.into()],
            ))
            .await?;

        let mut metrics = Vec::with_capacity(rows.len());

        for row in &rows {
            let bucket: DateTime<Utc> = row.try_get_by_index(0).unwrap_or(start_time);
            let platform: Option<String> = row.try_get_by_index(1).ok();
            let model: Option<String> = row.try_get_by_index(2).ok();
            let total_requests: i64 = row.try_get_by_index(3).unwrap_or(0);
            let successful_requests: i64 = row.try_get_by_index(4).unwrap_or(0);
            let failed_requests: i64 = row.try_get_by_index(5).unwrap_or(0);
            let total_tokens: i64 = row.try_get_by_index(6).unwrap_or(0);
            let prompt_tokens: i64 = row.try_get_by_index(7).unwrap_or(0);
            let completion_tokens: i64 = row.try_get_by_index(8).unwrap_or(0);
            let total_cost_raw: i64 = row.try_get_by_index(9).unwrap_or(0);

            let avg_response_time_ms = if total_requests > 0 {
                // response_time not available in aggregation query; set 0
                0.0
            } else {
                0.0
            };

            let agg = OpsAggregatedMetrics {
                timestamp: bucket,
                granularity: granularity.to_string(),
                platform,
                model,
                request_type: None,
                total_requests,
                successful_requests,
                failed_requests,
                total_response_time_ms: 0,
                avg_response_time_ms,
                p50_response_time_ms: 0,
                p95_response_time_ms: 0,
                p99_response_time_ms: 0,
                total_tokens,
                prompt_tokens,
                completion_tokens,
                total_cost_usd: total_cost_raw as f64 / 100.0,
            };

            tracing::debug!(
                "[{}] bucket={} platform={:?} model={:?} reqs={} ok={} fail={} tokens={} cost_usd={:.2}",
                granularity,
                bucket,
                agg.platform,
                agg.model,
                total_requests,
                successful_requests,
                failed_requests,
                total_tokens,
                agg.total_cost_usd,
            );

            metrics.push(agg);
        }

        Ok(metrics)
    }

    /// 回填聚合数据
    pub async fn backfill_aggregation(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        granularity: &str,
    ) -> Result<i64> {
        tracing::info!(
            "回填聚合数据: {} 到 {}, 粒度: {}",
            start_time,
            end_time,
            granularity
        );

        let metrics = self
            .aggregate_metrics(start_time, end_time, granularity)
            .await?;
        Ok(metrics.len() as i64)
    }

    /// 获取聚合状态
    pub async fn get_aggregation_status(&self) -> HashMap<String, AggregationTaskState> {
        let mut status = HashMap::new();

        let hourly = self.hourly_state.read().await.clone();
        let daily = self.daily_state.read().await.clone();

        status.insert("hourly".to_string(), hourly);
        status.insert("daily".to_string(), daily);

        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_config_default() {
        let config = OpsAggregationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.hourly_interval_secs, OPS_AGG_HOURLY_INTERVAL_SECS);
        assert_eq!(config.daily_interval_secs, OPS_AGG_DAILY_INTERVAL_SECS);
    }

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_aggregation_service_creation() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = OpsAggregationConfig::default();
        let service = OpsAggregationService::new(db, None, config);

        let status = service.get_aggregation_status().await;
        assert!(status.contains_key("hourly"));
        assert!(status.contains_key("daily"));
    }
}
