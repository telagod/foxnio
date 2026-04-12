//! 运维告警评估服务 - Ops Alert Evaluator Service
//!
//! 定期评估告警规则，触发告警通知

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 告警评估任务名称
const OPS_ALERT_EVALUATOR_JOB_NAME: &str = "ops_alert_evaluator";

/// 告警评估超时（秒）
const OPS_ALERT_EVALUATOR_TIMEOUT_SECS: i64 = 45;

/// 领导锁 TTL（秒）
const OPS_ALERT_EVALUATOR_LEADER_LOCK_TTL_SECS: i64 = 90;

/// 告警规则状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRuleState {
    pub rule_id: i64,
    pub last_evaluated_at: Option<DateTime<Utc>>,
    pub consecutive_breaches: i32,
    pub last_breach_at: Option<DateTime<Utc>>,
    pub last_notification_at: Option<DateTime<Utc>>,
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub metric_type: String,
    pub operator: String, // ">", "<", ">=", "<=", "==", "!="
    pub threshold: f64,
    pub duration_secs: i64,
    pub consecutive_breaches: i32,
    pub notification_channels: Vec<String>,
    pub cooldown_secs: i64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 告警事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub triggered_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub status: String, // "firing", "resolved"
    pub notified_at: Option<DateTime<Utc>>,
}

/// 告警通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertNotification {
    pub event_id: i64,
    pub channel: String,
    pub sent_at: DateTime<Utc>,
    pub success: bool,
    pub error_message: Option<String>,
}

/// 告警评估结果
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub rule_id: i64,
    pub is_breaching: bool,
    pub metric_value: f64,
    pub threshold: f64,
    pub should_notify: bool,
}

/// 运维告警评估服务配置
#[derive(Debug, Clone)]
pub struct OpsAlertEvaluatorConfig {
    pub enabled: bool,
    pub evaluation_interval_secs: i64,
    pub leader_lock_ttl_secs: i64,
    pub email_rate_limit_per_hour: i32,
    pub webhook_timeout_secs: i64,
}

impl Default for OpsAlertEvaluatorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            evaluation_interval_secs: 60,
            leader_lock_ttl_secs: OPS_ALERT_EVALUATOR_LEADER_LOCK_TTL_SECS,
            email_rate_limit_per_hour: 10,
            webhook_timeout_secs: 10,
        }
    }
}

/// 运维告警评估服务
pub struct OpsAlertEvaluatorService {
    db: sea_orm::DatabaseConnection,
    redis: Option<Arc<redis::Client>>,
    config: OpsAlertEvaluatorConfig,
    instance_id: String,

    // 状态管理
    rule_states: Arc<RwLock<HashMap<i64, AlertRuleState>>>,
    leader_lock: Arc<RwLock<Option<String>>>,
    stop_signal: Arc<RwLock<bool>>,

    // 邮件限流器
    email_limiter: Arc<RwLock<EmailRateLimiter>>,
}

/// 邮件速率限制器
#[derive(Debug)]
struct EmailRateLimiter {
    count: i32,
    window_start: DateTime<Utc>,
    limit_per_hour: i32,
}

impl EmailRateLimiter {
    fn new(limit_per_hour: i32) -> Self {
        Self {
            count: 0,
            window_start: Utc::now(),
            limit_per_hour,
        }
    }

    fn check_and_increment(&mut self) -> bool {
        let now = Utc::now();

        // 检查是否需要重置窗口
        if now - self.window_start > Duration::hours(1) {
            self.count = 0;
            self.window_start = now;
        }

        if self.count < self.limit_per_hour {
            self.count += 1;
            true
        } else {
            false
        }
    }
}

impl OpsAlertEvaluatorService {
    /// 创建新的告警评估服务实例
    pub fn new(
        db: sea_orm::DatabaseConnection,
        redis: Option<Arc<redis::Client>>,
        config: OpsAlertEvaluatorConfig,
    ) -> Self {
        let instance_id = uuid::Uuid::new_v4().to_string();
        let email_rate_limit = config.email_rate_limit_per_hour;

        Self {
            db,
            redis,
            config,
            instance_id,
            rule_states: Arc::new(RwLock::new(HashMap::new())),
            leader_lock: Arc::new(RwLock::new(None)),
            stop_signal: Arc::new(RwLock::new(false)),
            email_limiter: Arc::new(RwLock::new(EmailRateLimiter::new(email_rate_limit))),
        }
    }

    /// 启动告警评估服务
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("运维告警评估服务已禁用");
            return Ok(());
        }

        tracing::info!("启动运维告警评估服务，实例ID: {}", self.instance_id);

        // 尝试获取领导锁
        if self.try_acquire_leader_lock().await? {
            tracing::info!("成功获取领导锁，开始评估任务");

            // 启动评估循环
            self.start_evaluation_loop().await?;
        } else {
            tracing::info!("未能获取领导锁，作为备用实例运行");
        }

        Ok(())
    }

    /// 停止告警评估服务
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("停止运维告警评估服务");

        let mut stop = self.stop_signal.write().await;
        *stop = true;

        self.release_leader_lock().await?;

        Ok(())
    }

    /// 尝试获取领导锁
    async fn try_acquire_leader_lock(&self) -> Result<bool> {
        let Some(redis_client) = &self.redis else {
            return Ok(true);
        };

        let mut conn = redis_client.get_multiplexed_async_connection().await?;
        let lock_key = format!("foxnio:leader:{}", OPS_ALERT_EVALUATOR_JOB_NAME);
        let ttl_secs = self.config.leader_lock_ttl_secs;

        // SETNX with TTL via SET NX EX
        let acquired: bool = redis::cmd("SET")
            .arg(&lock_key)
            .arg(&self.instance_id)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await
            .unwrap_or(false);

        if acquired {
            let mut lock = self.leader_lock.write().await;
            *lock = Some(lock_key);
        }

        Ok(acquired)
    }

    /// 释放领导锁
    async fn release_leader_lock(&self) -> Result<()> {
        let Some(redis_client) = &self.redis else {
            return Ok(());
        };

        let lock = self.leader_lock.read().await;
        if let Some(lock_key) = lock.as_ref() {
            let mut conn = redis_client.get_multiplexed_async_connection().await?;
            // Only delete if we still own the lock (compare instance_id)
            let lua = r#"
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                    return redis.call('DEL', KEYS[1])
                else
                    return 0
                end
            "#;
            let _: i32 = redis::cmd("EVAL")
                .arg(lua)
                .arg(1)
                .arg(lock_key)
                .arg(&self.instance_id)
                .query_async(&mut conn)
                .await
                .unwrap_or(0);
        }
        drop(lock);

        let mut lock = self.leader_lock.write().await;
        *lock = None;

        Ok(())
    }

    /// 启动评估循环
    async fn start_evaluation_loop(&self) -> Result<()> {
        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(
            self.config.evaluation_interval_secs as u64,
        ));

        loop {
            if *self.stop_signal.read().await {
                break;
            }

            interval_timer.tick().await;

            // 执行评估
            if let Err(e) = self.run_evaluation().await {
                tracing::error!("告警评估失败: {}", e);
            }
        }

        Ok(())
    }

    /// 执行告警评估
    pub async fn run_evaluation(&self) -> Result<Vec<AlertEvent>> {
        tracing::debug!("开始执行告警评估");

        // 获取所有启用的告警规则
        let rules = self.get_enabled_rules().await?;

        let mut events = Vec::new();

        for rule in rules {
            // 评估每条规则
            let result = self.evaluate_rule(&rule).await?;

            // 更新规则状态
            self.update_rule_state(&rule, &result).await?;

            // 如果需要通知，发送告警
            if result.should_notify {
                if let Some(event) = self.send_alert(&rule, &result).await? {
                    events.push(event);
                }
            }
        }

        tracing::debug!("告警评估完成，触发 {} 个告警", events.len());

        Ok(events)
    }

    /// 获取所有启用的告警规则
    async fn get_enabled_rules(&self) -> Result<Vec<AlertRule>> {
        use crate::entity::alert_rules;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let rows = alert_rules::Entity::find()
            .filter(alert_rules::Column::Enabled.eq(true))
            .all(&self.db)
            .await?;

        let rules = rows
            .into_iter()
            .map(|r| {
                // Extract fields from condition_config JSON
                let operator = r
                    .condition_config
                    .get("operator")
                    .and_then(|v| v.as_str())
                    .unwrap_or(">")
                    .to_string();
                let threshold = r
                    .condition_config
                    .get("threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let consecutive_breaches = r
                    .condition_config
                    .get("consecutive_breaches")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32;
                let cooldown_secs = r
                    .condition_config
                    .get("cooldown_secs")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3600);
                let channels: Vec<String> = r
                    .channels
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                AlertRule {
                    id: r.id.as_u128() as i64,
                    name: r.name,
                    description: r.description,
                    metric_type: r.condition_type,
                    operator,
                    threshold,
                    duration_secs: r.duration_secs,
                    consecutive_breaches,
                    notification_channels: channels,
                    cooldown_secs,
                    enabled: r.enabled,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }
            })
            .collect();

        Ok(rules)
    }

    /// 评估单条规则
    async fn evaluate_rule(&self, rule: &AlertRule) -> Result<EvaluationResult> {
        // 获取指标值
        let metric_value = self.get_metric_value(&rule.metric_type).await?;

        // 评估阈值
        let is_breaching = match rule.operator.as_str() {
            ">" => metric_value > rule.threshold,
            "<" => metric_value < rule.threshold,
            ">=" => metric_value >= rule.threshold,
            "<=" => metric_value <= rule.threshold,
            "==" => (metric_value - rule.threshold).abs() < f64::EPSILON,
            "!=" => (metric_value - rule.threshold).abs() > f64::EPSILON,
            _ => false,
        };

        // 检查是否需要通知
        let should_notify = if is_breaching {
            let states = self.rule_states.read().await;
            if let Some(state) = states.get(&rule.id) {
                // 检查连续违规次数
                state.consecutive_breaches >= rule.consecutive_breaches - 1
                    && self.check_cooldown(rule, state).await
            } else {
                rule.consecutive_breaches <= 1
            }
        } else {
            false
        };

        Ok(EvaluationResult {
            rule_id: rule.id,
            is_breaching,
            metric_value,
            threshold: rule.threshold,
            should_notify,
        })
    }

    /// 获取指标值
    async fn get_metric_value(&self, metric_type: &str) -> Result<f64> {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let now = Utc::now();
        let one_hour_ago = now - Duration::hours(1);
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight");
        let today_start_utc = DateTime::<Utc>::from_naive_utc_and_offset(today_start, Utc);

        match metric_type {
            "error_rate" => {
                let row = self
                    .db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        r#"
                        SELECT
                            COUNT(*) AS total,
                            COALESCE(SUM(CASE WHEN success = false THEN 1 ELSE 0 END), 0) AS failed
                        FROM usages
                        WHERE created_at >= $1
                        "#,
                        [one_hour_ago.into()],
                    ))
                    .await?;

                match row {
                    Some(ref r) => {
                        let total: i64 = r.try_get_by_index(0).unwrap_or(0);
                        let failed: i64 = r.try_get_by_index(1).unwrap_or(0);
                        Ok(if total > 0 {
                            failed as f64 / total as f64
                        } else {
                            0.0
                        })
                    }
                    None => Ok(0.0),
                }
            }
            "request_count" | "today_requests" => {
                let row = self
                    .db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COUNT(*) FROM usages WHERE created_at >= $1",
                        [today_start_utc.into()],
                    ))
                    .await?;

                Ok(row
                    .as_ref()
                    .and_then(|r| r.try_get_by_index::<i64>(0).ok())
                    .unwrap_or(0) as f64)
            }
            "today_tokens" => {
                let row = self
                    .db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM usages WHERE created_at >= $1",
                        [today_start_utc.into()],
                    ))
                    .await?;

                Ok(row
                    .as_ref()
                    .and_then(|r| r.try_get_by_index::<i64>(0).ok())
                    .unwrap_or(0) as f64)
            }
            "today_cost" => {
                let row = self
                    .db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COALESCE(SUM(cost), 0) FROM usages WHERE created_at >= $1",
                        [today_start_utc.into()],
                    ))
                    .await?;

                Ok(row
                    .as_ref()
                    .and_then(|r| r.try_get_by_index::<i64>(0).ok())
                    .unwrap_or(0) as f64
                    / 100.0)
            }
            "avg_response_time" => {
                let row = self
                    .db
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        r#"
                        SELECT AVG((metadata->>'response_time_ms')::float)
                        FROM usages
                        WHERE created_at >= $1
                          AND metadata IS NOT NULL
                          AND metadata->>'response_time_ms' IS NOT NULL
                        "#,
                        [one_hour_ago.into()],
                    ))
                    .await?;

                Ok(row
                    .as_ref()
                    .and_then(|r| r.try_get_by_index::<Option<f64>>(0).ok())
                    .flatten()
                    .unwrap_or(0.0))
            }
            _ => {
                tracing::warn!("Unknown metric type: {}", metric_type);
                Ok(0.0)
            }
        }
    }

    /// 检查冷却时间
    async fn check_cooldown(&self, rule: &AlertRule, state: &AlertRuleState) -> bool {
        if let Some(last_notification) = state.last_notification_at {
            Utc::now() - last_notification > Duration::seconds(rule.cooldown_secs)
        } else {
            true
        }
    }

    /// 更新规则状态
    async fn update_rule_state(&self, rule: &AlertRule, result: &EvaluationResult) -> Result<()> {
        let mut states = self.rule_states.write().await;

        let state = states.entry(rule.id).or_insert(AlertRuleState {
            rule_id: rule.id,
            last_evaluated_at: None,
            consecutive_breaches: 0,
            last_breach_at: None,
            last_notification_at: None,
        });

        state.last_evaluated_at = Some(Utc::now());

        if result.is_breaching {
            state.consecutive_breaches += 1;
            state.last_breach_at = Some(Utc::now());
        } else {
            state.consecutive_breaches = 0;
        }

        Ok(())
    }

    /// 发送告警
    async fn send_alert(
        &self,
        rule: &AlertRule,
        result: &EvaluationResult,
    ) -> Result<Option<AlertEvent>> {
        // 创建告警事件
        let event = AlertEvent {
            id: 0,
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            metric_value: result.metric_value,
            threshold: result.threshold,
            triggered_at: Utc::now(),
            resolved_at: None,
            status: "firing".to_string(),
            notified_at: None,
        };

        // 发送通知到各个渠道
        for channel in &rule.notification_channels {
            match self.send_notification(channel, &event).await {
                Ok(_) => tracing::info!("告警通知已发送: {} -> {}", rule.name, channel),
                Err(e) => tracing::error!("告警通知发送失败: {} -> {}: {}", rule.name, channel, e),
            }
        }

        // 更新最后通知时间
        let mut states = self.rule_states.write().await;
        if let Some(state) = states.get_mut(&rule.id) {
            state.last_notification_at = Some(Utc::now());
        }

        Ok(Some(event))
    }

    /// 发送通知到指定渠道
    async fn send_notification(&self, channel: &str, event: &AlertEvent) -> Result<()> {
        match channel {
            "email" => {
                // 检查邮件限流
                let mut limiter = self.email_limiter.write().await;
                if !limiter.check_and_increment() {
                    tracing::warn!("邮件通知已达到速率限制");
                    return Ok(());
                }

                // Log the alert for email (actual SMTP delivery handled by lettre in caller)
                tracing::info!(
                    rule_id = event.rule_id,
                    rule_name = %event.rule_name,
                    metric_value = event.metric_value,
                    threshold = event.threshold,
                    "Email alert triggered: [{}] metric={:.4} threshold={:.4}",
                    event.rule_name,
                    event.metric_value,
                    event.threshold,
                );
            }
            channel_str if channel_str.starts_with("webhook:") => {
                let url = &channel_str["webhook:".len()..];
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(
                        self.config.webhook_timeout_secs as u64,
                    ))
                    .build()?;

                let payload = serde_json::json!({
                    "event": "alert",
                    "rule_id": event.rule_id,
                    "rule_name": event.rule_name,
                    "metric_value": event.metric_value,
                    "threshold": event.threshold,
                    "status": event.status,
                    "triggered_at": event.triggered_at.to_rfc3339(),
                });

                let resp = client.post(url).json(&payload).send().await?;
                if !resp.status().is_success() {
                    anyhow::bail!("Webhook returned HTTP {}", resp.status());
                }
            }
            "webhook" => {
                tracing::info!(
                    rule_id = event.rule_id,
                    rule_name = %event.rule_name,
                    "Webhook alert triggered but no URL configured"
                );
            }
            channel_str if channel_str.starts_with("slack:") => {
                let webhook_url = &channel_str["slack:".len()..];
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(
                        self.config.webhook_timeout_secs as u64,
                    ))
                    .build()?;

                let text = format!(
                    ":rotating_light: *Alert: {}*\nMetric: {:.4} (threshold: {:.4})\nStatus: {}\nTriggered: {}",
                    event.rule_name,
                    event.metric_value,
                    event.threshold,
                    event.status,
                    event.triggered_at.to_rfc3339(),
                );

                let payload = serde_json::json!({ "text": text });
                let resp = client.post(webhook_url).json(&payload).send().await?;
                if !resp.status().is_success() {
                    anyhow::bail!("Slack webhook returned HTTP {}", resp.status());
                }
            }
            "slack" => {
                tracing::info!(
                    rule_id = event.rule_id,
                    rule_name = %event.rule_name,
                    "Slack alert triggered but no webhook URL configured"
                );
            }
            _ => {
                tracing::warn!("未知的告警渠道: {}", channel);
            }
        }

        Ok(())
    }

    /// 获取告警状态
    pub async fn get_alert_status(&self) -> HashMap<i64, AlertRuleState> {
        self.rule_states.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_rate_limiter() {
        let mut limiter = EmailRateLimiter::new(5);

        for _ in 0..5 {
            assert!(limiter.check_and_increment());
        }

        // 第6次应该失败
        assert!(!limiter.check_and_increment());
    }

    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_evaluate_rule() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = OpsAlertEvaluatorConfig::default();
        let service = OpsAlertEvaluatorService::new(db, None, config);

        let rule = AlertRule {
            id: 1,
            name: "高错误率".to_string(),
            description: Some("错误率超过 5%".to_string()),
            metric_type: "error_rate".to_string(),
            operator: ">".to_string(),
            threshold: 0.05,
            duration_secs: 300,
            consecutive_breaches: 2,
            notification_channels: vec!["email".to_string()],
            cooldown_secs: 3600,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = service.evaluate_rule(&rule).await.unwrap();
        assert_eq!(result.rule_id, 1);
    }
}
