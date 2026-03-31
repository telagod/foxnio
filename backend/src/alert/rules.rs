//! 告警规则引擎
//!
//! 定义告警规则和条件评估逻辑
//!
//! 预留功能：告警规则（扩展功能）

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use super::{Alert, AlertChannelType, AlertLevel};

/// 告警条件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertCondition {
    /// 错误率超过阈值
    ErrorRateAbove {
        /// 错误率阈值（百分比，如 5.0 表示 5%）
        threshold: f64,
    },
    /// 延迟超过阈值
    LatencyAbove {
        /// 延迟阈值（毫秒）
        threshold_ms: u64,
    },
    /// 连接数低于阈值
    ConnectionCountBelow {
        /// 连接数阈值
        threshold: u32,
    },
    /// 账户余额低于阈值
    AccountBalanceBelow {
        /// 余额阈值（分）
        threshold: i64,
    },
    /// CPU 使用率超过阈值
    CpuUsageAbove {
        /// 使用率阈值（百分比）
        threshold: f64,
    },
    /// 内存使用率超过阈值
    MemoryUsageAbove {
        /// 使用率阈值（百分比）
        threshold: f64,
    },
    /// 磁盘使用率超过阈值
    DiskUsageAbove {
        /// 使用率阈值（百分比）
        threshold: f64,
    },
    /// 请求频率超过阈值
    RequestRateAbove {
        /// 每秒请求数阈值
        threshold: f64,
    },
    /// 自定义条件（表达式）
    Custom {
        /// 表达式（支持简单比较）
        expression: String,
    },
}

impl AlertCondition {
    /// 获取条件的描述
    pub fn description(&self) -> String {
        match self {
            Self::ErrorRateAbove { threshold } => {
                format!("错误率 > {threshold}%")
            }
            Self::LatencyAbove { threshold_ms } => {
                format!("延迟 > {threshold_ms}ms")
            }
            Self::ConnectionCountBelow { threshold } => {
                format!("连接数 < {threshold}")
            }
            Self::AccountBalanceBelow { threshold } => {
                format!("余额 < ¥{:.2}", *threshold as f64 / 100.0)
            }
            Self::CpuUsageAbove { threshold } => {
                format!("CPU 使用率 > {threshold}%")
            }
            Self::MemoryUsageAbove { threshold } => {
                format!("内存使用率 > {threshold}%")
            }
            Self::DiskUsageAbove { threshold } => {
                format!("磁盘使用率 > {threshold}%")
            }
            Self::RequestRateAbove { threshold } => {
                format!("请求频率 > {threshold}/s")
            }
            Self::Custom { expression } => {
                format!("自定义: {expression}")
            }
        }
    }

    /// 评估条件是否满足
    pub fn evaluate(&self, metrics: &MetricsSnapshot) -> bool {
        match self {
            Self::ErrorRateAbove { threshold } => metrics.error_rate > *threshold,
            Self::LatencyAbove { threshold_ms } => metrics.avg_latency_ms > *threshold_ms,
            Self::ConnectionCountBelow { threshold } => metrics.active_connections < *threshold,
            Self::AccountBalanceBelow { threshold } => metrics.account_balance < *threshold,
            Self::CpuUsageAbove { threshold } => metrics.cpu_usage > *threshold,
            Self::MemoryUsageAbove { threshold } => metrics.memory_usage > *threshold,
            Self::DiskUsageAbove { threshold } => metrics.disk_usage > *threshold,
            Self::RequestRateAbove { threshold } => metrics.request_rate > *threshold,
            Self::Custom { expression } => self.evaluate_custom(expression, metrics),
        }
    }

    /// 评估自定义表达式
    fn evaluate_custom(&self, expression: &str, metrics: &MetricsSnapshot) -> bool {
        // 简单的表达式解析，支持基本比较
        // 格式: metric > value, metric < value, metric >= value, metric <= value
        let expr = expression.trim();

        // 解析比较操作符
        let (op, parts): (&str, Vec<&str>) = if expr.contains(">=") {
            (">=", expr.split(">=").collect())
        } else if expr.contains("<=") {
            ("<=", expr.split("<=").collect())
        } else if expr.contains(">") {
            (">", expr.split('>').collect())
        } else if expr.contains("<") {
            ("<", expr.split('<').collect())
        } else if expr.contains("==") {
            ("==", expr.split("==").collect())
        } else {
            return false;
        };

        if parts.len() != 2 {
            return false;
        }

        let metric_name = parts[0].trim();
        let value_str = parts[1].trim();

        let metric_value = match metric_name {
            "error_rate" => metrics.error_rate,
            "latency_ms" | "avg_latency_ms" => metrics.avg_latency_ms as f64,
            "connections" | "active_connections" => metrics.active_connections as f64,
            "balance" | "account_balance" => metrics.account_balance as f64,
            "cpu" | "cpu_usage" => metrics.cpu_usage,
            "memory" | "memory_usage" => metrics.memory_usage,
            "disk" | "disk_usage" => metrics.disk_usage,
            "request_rate" => metrics.request_rate,
            _ => return false,
        };

        let value = value_str.parse::<f64>().unwrap_or(0.0);

        match op {
            ">" => metric_value > value,
            "<" => metric_value < value,
            ">=" => metric_value >= value,
            "<=" => metric_value <= value,
            "==" => (metric_value - value).abs() < f64::EPSILON,
            _ => false,
        }
    }
}

/// 指标快照
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// 错误率（百分比）
    pub error_rate: f64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: u64,
    /// 活跃连接数
    pub active_connections: u32,
    /// 账户余额（分）
    pub account_balance: i64,
    /// CPU 使用率（百分比）
    pub cpu_usage: f64,
    /// 内存使用率（百分比）
    pub memory_usage: f64,
    /// 磁盘使用率（百分比）
    pub disk_usage: f64,
    /// 请求频率（每秒）
    pub request_rate: f64,
    /// 自定义指标
    #[serde(default)]
    pub custom: std::collections::HashMap<String, f64>,
    /// 快照时间
    pub timestamp: DateTime<Utc>,
}

impl MetricsSnapshot {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// 设置错误率
    pub fn with_error_rate(mut self, rate: f64) -> Self {
        self.error_rate = rate;
        self
    }

    /// 设置延迟
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.avg_latency_ms = latency_ms;
        self
    }

    /// 设置连接数
    pub fn with_connections(mut self, count: u32) -> Self {
        self.active_connections = count;
        self
    }

    /// 设置余额
    pub fn with_balance(mut self, balance: i64) -> Self {
        self.account_balance = balance;
        self
    }

    /// 设置 CPU 使用率
    pub fn with_cpu(mut self, usage: f64) -> Self {
        self.cpu_usage = usage;
        self
    }

    /// 设置内存使用率
    pub fn with_memory(mut self, usage: f64) -> Self {
        self.memory_usage = usage;
        self
    }

    /// 设置磁盘使用率
    pub fn with_disk(mut self, usage: f64) -> Self {
        self.disk_usage = usage;
        self
    }

    /// 设置请求频率
    pub fn with_request_rate(mut self, rate: f64) -> Self {
        self.request_rate = rate;
        self
    }

    /// 设置自定义指标
    pub fn with_custom(mut self, name: impl Into<String>, value: f64) -> Self {
        self.custom.insert(name.into(), value);
        self
    }
}

/// 告警规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// 规则 ID
    #[serde(default = "new_uuid")]
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 规则描述
    #[serde(default)]
    pub description: String,
    /// 告警条件
    pub condition: AlertCondition,
    /// 持续时间（条件持续满足多久才触发告警）
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    /// 告警级别
    #[serde(default)]
    pub level: AlertLevel,
    /// 告警通道列表
    pub channels: Vec<AlertChannelType>,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 规则标签
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
    /// 创建时间
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// 更新时间
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    /// 触发次数
    #[serde(default)]
    pub trigger_count: u64,
    /// 最后触发时间
    pub last_triggered_at: Option<DateTime<Utc>>,
}

fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn default_enabled() -> bool {
    true
}

/// Duration 序列化模块
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

impl AlertRule {
    /// 创建新的告警规则
    pub fn new(
        name: impl Into<String>,
        condition: AlertCondition,
        level: AlertLevel,
        channels: Vec<AlertChannelType>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            condition,
            duration: Duration::from_secs(0),
            level,
            channels,
            enabled: true,
            labels: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            trigger_count: 0,
            last_triggered_at: None,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// 设置持续时间
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// 添加标签
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// 设置启用状态
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 检查规则是否应该触发告警
    pub fn check(&self, metrics: &MetricsSnapshot, state: &RuleState) -> RuleCheckResult {
        if !self.enabled {
            return RuleCheckResult::Disabled;
        }

        let condition_met = self.condition.evaluate(metrics);

        match (condition_met, state.condition_start) {
            (true, None) => {
                // 条件刚满足
                if self.duration.is_zero() {
                    RuleCheckResult::Triggered
                } else {
                    RuleCheckResult::ConditionStarted
                }
            }
            (true, Some(start)) => {
                // 条件持续满足
                let elapsed = Utc::now() - start;
                if elapsed.to_std().unwrap_or_default() >= self.duration {
                    RuleCheckResult::Triggered
                } else {
                    RuleCheckResult::ConditionOngoing
                }
            }
            (false, Some(_)) => {
                // 条件不再满足
                RuleCheckResult::ConditionEnded
            }
            (false, None) => {
                // 条件未满足
                RuleCheckResult::Ok
            }
        }
    }

    /// 生成告警
    pub fn generate_alert(&self, _metrics: &MetricsSnapshot) -> Alert {
        let message = format!(
            "规则 '{}' 触发告警。条件: {}",
            self.name,
            self.condition.description()
        );

        let mut labels = self.labels.clone();
        labels.insert("rule_id".to_string(), self.id.clone());
        labels.insert("rule_name".to_string(), self.name.clone());

        Alert::new(self.level, self.name.clone(), message)
            .with_source("alert_engine")
            .with_label("rule_id", &self.id)
    }

    /// 记录触发
    pub fn record_trigger(&mut self) {
        self.trigger_count += 1;
        self.last_triggered_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// 规则状态（用于跟踪持续时间条件）
#[derive(Debug, Clone, Default)]
pub struct RuleState {
    /// 条件开始满足的时间
    pub condition_start: Option<DateTime<Utc>>,
    /// 最后检查时间
    pub last_check: Option<DateTime<Utc>>,
    /// 是否在静默中
    pub silenced: bool,
    /// 静默结束时间
    pub silence_until: Option<DateTime<Utc>>,
}

impl RuleState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 开始条件满足
    pub fn start_condition(&mut self) {
        self.condition_start = Some(Utc::now());
        self.last_check = Some(Utc::now());
    }

    /// 结束条件满足
    pub fn end_condition(&mut self) {
        self.condition_start = None;
        self.last_check = Some(Utc::now());
    }

    /// 设置静默
    pub fn set_silence(&mut self, until: DateTime<Utc>) {
        self.silenced = true;
        self.silence_until = Some(until);
    }

    /// 清除静默
    pub fn clear_silence(&mut self) {
        self.silenced = false;
        self.silence_until = None;
    }

    /// 检查静默状态
    pub fn is_silenced(&self) -> bool {
        if !self.silenced {
            return false;
        }

        if let Some(until) = self.silence_until {
            if Utc::now() > until {
                return false;
            }
        }

        true
    }
}

/// 规则检查结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCheckResult {
    /// 正常
    Ok,
    /// 规则已禁用
    Disabled,
    /// 条件开始满足
    ConditionStarted,
    /// 条件持续满足中（未达到持续时间）
    ConditionOngoing,
    /// 条件结束
    ConditionEnded,
    /// 触发告警
    Triggered,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_condition_error_rate() {
        let condition = AlertCondition::ErrorRateAbove { threshold: 5.0 };

        let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
        let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

        assert!(!condition.evaluate(&metrics_ok));
        assert!(condition.evaluate(&metrics_trigger));
    }

    #[test]
    fn test_alert_condition_latency() {
        let condition = AlertCondition::LatencyAbove { threshold_ms: 1000 };

        let metrics_ok = MetricsSnapshot::new().with_latency(500);
        let metrics_trigger = MetricsSnapshot::new().with_latency(2000);

        assert!(!condition.evaluate(&metrics_ok));
        assert!(condition.evaluate(&metrics_trigger));
    }

    #[test]
    fn test_alert_condition_custom() {
        let condition = AlertCondition::Custom {
            expression: "error_rate > 10".to_string(),
        };

        let metrics_ok = MetricsSnapshot::new().with_error_rate(5.0);
        let metrics_trigger = MetricsSnapshot::new().with_error_rate(15.0);

        assert!(!condition.evaluate(&metrics_ok));
        assert!(condition.evaluate(&metrics_trigger));
    }

    #[test]
    fn test_alert_rule_creation() {
        let rule = AlertRule::new(
            "高错误率告警",
            AlertCondition::ErrorRateAbove { threshold: 5.0 },
            AlertLevel::Warning,
            vec![AlertChannelType::Email, AlertChannelType::Slack],
        )
        .with_duration(Duration::from_secs(60))
        .with_description("当错误率超过 5% 时触发");

        assert_eq!(rule.name, "高错误率告警");
        assert_eq!(rule.level, AlertLevel::Warning);
        assert_eq!(rule.channels.len(), 2);
        assert!(rule.enabled);
    }

    #[test]
    fn test_rule_check_immediate() {
        let rule = AlertRule::new(
            "测试规则",
            AlertCondition::ErrorRateAbove { threshold: 5.0 },
            AlertLevel::Warning,
            vec![AlertChannelType::Email],
        );

        let state = RuleState::new();
        let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
        let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

        assert_eq!(rule.check(&metrics_ok, &state), RuleCheckResult::Ok);
        assert_eq!(
            rule.check(&metrics_trigger, &state),
            RuleCheckResult::Triggered
        );
    }

    #[test]
    fn test_rule_check_with_duration() {
        let rule = AlertRule::new(
            "测试规则",
            AlertCondition::ErrorRateAbove { threshold: 5.0 },
            AlertLevel::Warning,
            vec![AlertChannelType::Email],
        )
        .with_duration(Duration::from_secs(60));

        let state = RuleState::new();
        let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

        // 条件刚满足，应该返回 ConditionStarted
        assert_eq!(
            rule.check(&metrics_trigger, &state),
            RuleCheckResult::ConditionStarted
        );

        // 条件开始满足
        let mut state_with_condition = RuleState::new();
        state_with_condition.condition_start = Some(Utc::now() - chrono::Duration::seconds(30));

        // 30 秒，未达到 60 秒
        assert_eq!(
            rule.check(&metrics_trigger, &state_with_condition),
            RuleCheckResult::ConditionOngoing
        );

        // 条件满足 60 秒
        let mut state_long = RuleState::new();
        state_long.condition_start = Some(Utc::now() - chrono::Duration::seconds(70));

        assert_eq!(
            rule.check(&metrics_trigger, &state_long),
            RuleCheckResult::Triggered
        );
    }

    #[test]
    fn test_rule_state_silence() {
        let mut state = RuleState::new();

        assert!(!state.is_silenced());

        state.set_silence(Utc::now() + chrono::Duration::hours(1));
        assert!(state.is_silenced());

        state.clear_silence();
        assert!(!state.is_silenced());
    }

    #[test]
    fn test_rule_generate_alert() {
        let rule = AlertRule::new(
            "CPU 高负载",
            AlertCondition::CpuUsageAbove { threshold: 80.0 },
            AlertLevel::Error,
            vec![AlertChannelType::Slack],
        );

        let metrics = MetricsSnapshot::new().with_cpu(85.0);
        let alert = rule.generate_alert(&metrics);

        assert_eq!(alert.level, AlertLevel::Error);
        assert!(alert.title.contains("CPU 高负载"));
    }

    #[test]
    fn test_condition_description() {
        let conditions = vec![
            AlertCondition::ErrorRateAbove { threshold: 5.0 },
            AlertCondition::LatencyAbove { threshold_ms: 1000 },
            AlertCondition::ConnectionCountBelow { threshold: 10 },
            AlertCondition::AccountBalanceBelow { threshold: 10000 },
            AlertCondition::CpuUsageAbove { threshold: 80.0 },
        ];

        for condition in conditions {
            let desc = condition.description();
            assert!(!desc.is_empty());
        }
    }
}
