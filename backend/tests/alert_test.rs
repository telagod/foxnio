#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 告警系统测试
//!
//! 测试覆盖：
//! - 规则引擎测试
//! - 告警发送测试
//! - 静默机制测试
//! - 历史记录测试
//! - 多通道支持测试

mod common;

use foxnio::alert::{
    history::{AlertHistory, AlertHistoryEntry, AlertHistoryFilter},
    manager::AlertManager,
    rules::{AlertCondition, AlertRule, MetricsSnapshot, RuleCheckResult, RuleState},
    Alert, AlertChannelType, AlertLevel, AlertSendResult, SilenceRule,
};
use std::time::Duration;

// ============ 告警基础测试 ============

#[test]
fn test_alert_level_conversion() {
    // 测试字符串转换
    assert_eq!(AlertLevel::parse("info"), Some(AlertLevel::Info));
    assert_eq!(AlertLevel::parse("WARNING"), Some(AlertLevel::Warning));
    assert_eq!(AlertLevel::parse("Error"), Some(AlertLevel::Error));
    assert_eq!(AlertLevel::parse("critical"), Some(AlertLevel::Critical));
    assert_eq!(AlertLevel::parse("invalid"), None);

    // 测试显示
    assert_eq!(AlertLevel::Info.as_str(), "info");
    assert_eq!(AlertLevel::Critical.as_str(), "critical");
}

#[test]
fn test_alert_level_priority() {
    assert!(!AlertLevel::Info.is_high_priority());
    assert!(!AlertLevel::Warning.is_high_priority());
    assert!(AlertLevel::Error.is_high_priority());
    assert!(AlertLevel::Critical.is_high_priority());
}

#[test]
fn test_alert_creation() {
    let alert = Alert::new(AlertLevel::Warning, "测试告警", "这是一条测试消息")
        .with_source("test_module")
        .with_label("environment", "production")
        .with_label("service", "api");

    assert_eq!(alert.level, AlertLevel::Warning);
    assert_eq!(alert.title, "测试告警");
    assert_eq!(alert.message, "这是一条测试消息");
    assert_eq!(alert.source, "test_module");
    assert_eq!(alert.labels.len(), 2);
}

#[test]
fn test_alert_formatting() {
    let alert = Alert::new(AlertLevel::Error, "系统错误", "数据库连接失败");

    let summary = alert.to_summary();
    assert!(summary.contains("ERROR"));
    assert!(summary.contains("系统错误"));

    let detailed = alert.to_detailed();
    assert!(detailed.contains("数据库连接失败"));
    assert!(detailed.contains("来源: foxnio"));

    let json = serde_json::to_value(&alert).unwrap();
    assert_eq!(json["level"], "error");
    assert_eq!(json["title"], "系统错误");
}

#[test]
fn test_alert_channel_type() {
    assert_eq!(
        AlertChannelType::parse("email"),
        Some(AlertChannelType::Email)
    );
    assert_eq!(
        AlertChannelType::parse("webhook"),
        Some(AlertChannelType::Webhook)
    );
    assert_eq!(
        AlertChannelType::parse("dingtalk"),
        Some(AlertChannelType::DingTalk)
    );
    assert_eq!(
        AlertChannelType::parse("dingding"),
        Some(AlertChannelType::DingTalk)
    );
    assert_eq!(
        AlertChannelType::parse("feishu"),
        Some(AlertChannelType::Feishu)
    );
    assert_eq!(
        AlertChannelType::parse("lark"),
        Some(AlertChannelType::Feishu)
    );
    assert_eq!(
        AlertChannelType::parse("slack"),
        Some(AlertChannelType::Slack)
    );
}

// ============ 规则引擎测试 ============

#[test]
fn test_condition_error_rate() {
    let condition = AlertCondition::ErrorRateAbove { threshold: 5.0 };

    let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);
    let metrics_boundary = MetricsSnapshot::new().with_error_rate(5.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
    assert!(!condition.evaluate(&metrics_boundary)); // 边界值不触发
}

#[test]
fn test_condition_latency() {
    let condition = AlertCondition::LatencyAbove { threshold_ms: 1000 };

    let metrics_ok = MetricsSnapshot::new().with_latency(500);
    let metrics_trigger = MetricsSnapshot::new().with_latency(2000);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_connections() {
    let condition = AlertCondition::ConnectionCountBelow { threshold: 10 };

    let metrics_ok = MetricsSnapshot::new().with_connections(20);
    let metrics_trigger = MetricsSnapshot::new().with_connections(5);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_balance() {
    let condition = AlertCondition::AccountBalanceBelow { threshold: 10000 }; // 100 元

    let metrics_ok = MetricsSnapshot::new().with_balance(50000);
    let metrics_trigger = MetricsSnapshot::new().with_balance(5000);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_cpu() {
    let condition = AlertCondition::CpuUsageAbove { threshold: 80.0 };

    let metrics_ok = MetricsSnapshot::new().with_cpu(50.0);
    let metrics_trigger = MetricsSnapshot::new().with_cpu(90.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_memory() {
    let condition = AlertCondition::MemoryUsageAbove { threshold: 85.0 };

    let metrics_ok = MetricsSnapshot::new().with_memory(70.0);
    let metrics_trigger = MetricsSnapshot::new().with_memory(90.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_disk() {
    let condition = AlertCondition::DiskUsageAbove { threshold: 90.0 };

    let metrics_ok = MetricsSnapshot::new().with_disk(80.0);
    let metrics_trigger = MetricsSnapshot::new().with_disk(95.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_request_rate() {
    let condition = AlertCondition::RequestRateAbove { threshold: 1000.0 };

    let metrics_ok = MetricsSnapshot::new().with_request_rate(500.0);
    let metrics_trigger = MetricsSnapshot::new().with_request_rate(1500.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));
}

#[test]
fn test_condition_custom() {
    let condition = AlertCondition::Custom {
        expression: "error_rate > 10".to_string(),
    };

    let metrics_ok = MetricsSnapshot::new().with_error_rate(5.0);
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(15.0);

    assert!(!condition.evaluate(&metrics_ok));
    assert!(condition.evaluate(&metrics_trigger));

    // 测试其他表达式
    let condition2 = AlertCondition::Custom {
        expression: "cpu >= 80".to_string(),
    };
    let metrics = MetricsSnapshot::new().with_cpu(80.0);
    assert!(condition2.evaluate(&metrics));

    let condition3 = AlertCondition::Custom {
        expression: "connections < 5".to_string(),
    };
    let metrics = MetricsSnapshot::new().with_connections(3);
    assert!(condition3.evaluate(&metrics));
}

#[test]
fn test_rule_creation() {
    let rule = AlertRule::new(
        "高错误率告警",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email, AlertChannelType::Slack],
    )
    .with_description("当错误率超过 5% 时触发告警")
    .with_duration(Duration::from_secs(60))
    .with_label("team", "platform");

    assert_eq!(rule.name, "高错误率告警");
    assert_eq!(rule.description, "当错误率超过 5% 时触发告警");
    assert_eq!(rule.level, AlertLevel::Warning);
    assert_eq!(rule.channels.len(), 2);
    assert_eq!(rule.duration, Duration::from_secs(60));
    assert!(rule.enabled);
    assert_eq!(rule.labels.get("team"), Some(&"platform".to_string()));
}

#[test]
fn test_rule_check_immediate() {
    let rule = AlertRule::new(
        "立即触发规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );

    let state = RuleState::new();
    let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

    // 正常情况
    assert_eq!(rule.check(&metrics_ok, &state), RuleCheckResult::Ok);

    // 触发告警
    assert_eq!(
        rule.check(&metrics_trigger, &state),
        RuleCheckResult::Triggered
    );
}

#[test]
fn test_rule_check_with_duration() {
    let rule = AlertRule::new(
        "持续触发规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    )
    .with_duration(Duration::from_secs(60));

    let state = RuleState::new();
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

    // 条件刚满足
    assert_eq!(
        rule.check(&metrics_trigger, &state),
        RuleCheckResult::ConditionStarted
    );

    // 条件持续 30 秒（未达到 60 秒）
    let mut state_30s = RuleState::new();
    state_30s.condition_start = Some(chrono::Utc::now() - chrono::Duration::seconds(30));
    assert_eq!(
        rule.check(&metrics_trigger, &state_30s),
        RuleCheckResult::ConditionOngoing
    );

    // 条件持续 70 秒（超过 60 秒）
    let mut state_70s = RuleState::new();
    state_70s.condition_start = Some(chrono::Utc::now() - chrono::Duration::seconds(70));
    assert_eq!(
        rule.check(&metrics_trigger, &state_70s),
        RuleCheckResult::Triggered
    );

    // 条件结束
    let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
    assert_eq!(
        rule.check(&metrics_ok, &state_70s),
        RuleCheckResult::ConditionEnded
    );
}

#[test]
fn test_rule_disabled() {
    let rule = AlertRule::new(
        "禁用规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    )
    .with_enabled(false);

    let state = RuleState::new();
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);

    assert_eq!(
        rule.check(&metrics_trigger, &state),
        RuleCheckResult::Disabled
    );
}

#[test]
fn test_rule_generate_alert() {
    let rule = AlertRule::new(
        "CPU 高负载",
        AlertCondition::CpuUsageAbove { threshold: 80.0 },
        AlertLevel::Error,
        vec![AlertChannelType::Slack],
    )
    .with_label("server", "prod-01");

    let metrics = MetricsSnapshot::new().with_cpu(90.0);
    let alert = rule.generate_alert(&metrics);

    assert_eq!(alert.level, AlertLevel::Error);
    assert!(alert.title.contains("CPU 高负载"));
    assert!(alert.message.contains("CPU 使用率 > 80%"));
}

// ============ 静默机制测试 ============

#[test]
fn test_silence_rule_creation() {
    let silence = SilenceRule {
        id: "silence-1".to_string(),
        rule_pattern: "db-*".to_string(),
        start_time: chrono::Utc::now() - chrono::Duration::hours(1),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "数据库维护".to_string(),
        created_by: Some("admin".to_string()),
    };

    assert!(silence.is_active());
    assert!(silence.matches("db-connection-error"));
    assert!(silence.matches("db-timeout"));
    assert!(!silence.matches("api-error"));
}

#[test]
fn test_silence_pattern_matching() {
    // 通配符匹配
    let silence_all = SilenceRule {
        id: "silence-all".to_string(),
        rule_pattern: "*".to_string(),
        start_time: chrono::Utc::now() - chrono::Duration::hours(1),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "全站维护".to_string(),
        created_by: None,
    };
    assert!(silence_all.matches("any-rule"));

    // 前缀匹配
    let silence_prefix = SilenceRule {
        id: "silence-prefix".to_string(),
        rule_pattern: "api-*".to_string(),
        start_time: chrono::Utc::now(),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "API 维护".to_string(),
        created_by: None,
    };
    assert!(silence_prefix.matches("api-timeout"));
    assert!(silence_prefix.matches("api-error"));
    assert!(!silence_prefix.matches("db-error"));

    // 后缀匹配
    let silence_suffix = SilenceRule {
        id: "silence-suffix".to_string(),
        rule_pattern: "*-critical".to_string(),
        start_time: chrono::Utc::now(),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "忽略关键告警".to_string(),
        created_by: None,
    };
    assert!(silence_suffix.matches("db-critical"));
    assert!(silence_suffix.matches("api-critical"));
    assert!(!silence_suffix.matches("db-warning"));

    // 包含匹配
    let silence_contains = SilenceRule {
        id: "silence-contains".to_string(),
        rule_pattern: "*test*".to_string(),
        start_time: chrono::Utc::now(),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "测试环境".to_string(),
        created_by: None,
    };
    assert!(silence_contains.matches("test-api"));
    assert!(silence_contains.matches("api-test-rule"));
    assert!(!silence_contains.matches("production-api"));
}

#[test]
fn test_silence_time_window() {
    // 过期的静默
    let expired = SilenceRule {
        id: "expired".to_string(),
        rule_pattern: "*".to_string(),
        start_time: chrono::Utc::now() - chrono::Duration::hours(2),
        end_time: chrono::Utc::now() - chrono::Duration::hours(1),
        reason: "已过期".to_string(),
        created_by: None,
    };
    assert!(!expired.is_active());

    // 未开始的静默
    let future = SilenceRule {
        id: "future".to_string(),
        rule_pattern: "*".to_string(),
        start_time: chrono::Utc::now() + chrono::Duration::hours(1),
        end_time: chrono::Utc::now() + chrono::Duration::hours(2),
        reason: "计划维护".to_string(),
        created_by: None,
    };
    assert!(!future.is_active());
}

#[test]
fn test_rule_state_silence() {
    let mut state = RuleState::new();

    assert!(!state.is_silenced());

    state.set_silence(chrono::Utc::now() + chrono::Duration::hours(1));
    assert!(state.is_silenced());

    state.clear_silence();
    assert!(!state.is_silenced());
}

// ============ 历史记录测试 ============

#[tokio::test]
async fn test_history_add_and_get() {
    let history = AlertHistory::new(100);

    let alert = Alert::new(AlertLevel::Warning, "测试告警", "测试消息");
    let entry = AlertHistoryEntry::new(
        alert,
        Some("rule-1".to_string()),
        Some("测试规则".to_string()),
    );

    let id = entry.id.clone();
    history.add(entry).await;

    let retrieved = history.get(&id).await;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.alert.title, "测试告警");
    assert_eq!(retrieved.rule_id, Some("rule-1".to_string()));
}

#[tokio::test]
async fn test_history_query() {
    let history = AlertHistory::new(100);

    // 添加多条记录
    for i in 0..5 {
        let level = if i % 2 == 0 {
            AlertLevel::Warning
        } else {
            AlertLevel::Error
        };
        let alert = Alert::new(level, format!("告警 {}", i), "测试");
        let entry = AlertHistoryEntry::new(alert, None, None);
        history.add(entry).await;
    }

    // 按级别过滤
    let filter = AlertHistoryFilter {
        level: Some(AlertLevel::Error),
        ..Default::default()
    };
    let results = history.query(&filter).await;
    assert_eq!(results.len(), 2);

    // 限制数量
    let filter = AlertHistoryFilter {
        limit: Some(3),
        ..Default::default()
    };
    let results = history.query(&filter).await;
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_history_stats() {
    let history = AlertHistory::new(100);

    // 添加记录
    history
        .add(AlertHistoryEntry::new(
            Alert::new(AlertLevel::Warning, "告警1", "test").with_source("api"),
            Some("rule-1".to_string()),
            Some("API规则".to_string()),
        ))
        .await;

    history
        .add(AlertHistoryEntry::new(
            Alert::new(AlertLevel::Error, "告警2", "test").with_source("db"),
            Some("rule-2".to_string()),
            Some("DB规则".to_string()),
        ))
        .await;

    let stats = history.stats(None, None).await;
    assert_eq!(stats.total_count, 2);
    assert_eq!(*stats.by_level.get("warning").unwrap_or(&0), 1);
    assert_eq!(*stats.by_level.get("error").unwrap_or(&0), 1);
}

#[tokio::test]
async fn test_history_max_entries() {
    let history = AlertHistory::new(3);

    for i in 0..5 {
        let alert = Alert::new(AlertLevel::Info, format!("告警 {}", i), "test");
        history.add(AlertHistoryEntry::new(alert, None, None)).await;
    }

    assert_eq!(history.len().await, 3);
}

#[tokio::test]
async fn test_history_cleanup() {
    let history = AlertHistory::new(100);

    let alert = Alert::new(AlertLevel::Info, "测试", "test");
    history.add(AlertHistoryEntry::new(alert, None, None)).await;
    assert_eq!(history.len().await, 1);

    // 清理所有记录
    let removed = history
        .cleanup(chrono::Utc::now() + chrono::Duration::hours(1))
        .await;
    assert_eq!(removed, 1);
    assert_eq!(history.len().await, 0);
}

// ============ 告警管理器测试 ============

#[tokio::test]
async fn test_manager_add_rule() {
    let manager = AlertManager::with_defaults();

    let rule = AlertRule::new(
        "测试规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );
    let rule_id = rule.id.clone();

    manager.add_rule(rule).await;

    let retrieved = manager.get_rule(&rule_id).await;
    assert!(retrieved.is_some());
}

#[tokio::test]
async fn test_manager_update_rule() {
    let manager = AlertManager::with_defaults();

    let rule = AlertRule::new(
        "原始规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );
    let rule_id = rule.id.clone();
    manager.add_rule(rule).await;

    let updated = AlertRule::new(
        "更新后规则",
        AlertCondition::LatencyAbove { threshold_ms: 1000 },
        AlertLevel::Error,
        vec![AlertChannelType::Slack],
    );
    let updated_id = updated.id.clone();
    manager.update_rule(&rule_id, updated).await;

    let retrieved = manager.get_rule(&updated_id).await.unwrap();
    assert_eq!(retrieved.name, "更新后规则");
    assert_eq!(retrieved.level, AlertLevel::Error);
}

#[tokio::test]
async fn test_manager_delete_rule() {
    let manager = AlertManager::with_defaults();

    let rule = AlertRule::new(
        "待删除规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );
    let rule_id = rule.id.clone();
    manager.add_rule(rule).await;

    assert!(manager.delete_rule(&rule_id).await);
    assert!(manager.get_rule(&rule_id).await.is_none());
}

#[tokio::test]
async fn test_manager_enable_disable_rule() {
    let manager = AlertManager::with_defaults();

    let rule = AlertRule::new(
        "开关规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );
    let rule_id = rule.id.clone();
    manager.add_rule(rule).await;

    manager.disable_rule(&rule_id).await;
    let retrieved = manager.get_rule(&rule_id).await.unwrap();
    assert!(!retrieved.enabled);

    manager.enable_rule(&rule_id).await;
    let retrieved = manager.get_rule(&rule_id).await.unwrap();
    assert!(retrieved.enabled);
}

#[tokio::test]
async fn test_manager_silence() {
    let manager = AlertManager::with_defaults();

    let silence = SilenceRule {
        id: "silence-1".to_string(),
        rule_pattern: "test-*".to_string(),
        start_time: chrono::Utc::now(),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "测试".to_string(),
        created_by: None,
    };

    manager.add_silence(silence).await;

    assert!(manager.is_silenced("test-alert").await);
    assert!(!manager.is_silenced("other-alert").await);

    manager.remove_silence("silence-1").await;
    assert!(!manager.is_silenced("test-alert").await);
}

#[tokio::test]
async fn test_manager_check_rules() {
    let manager = AlertManager::with_defaults();

    let rule = AlertRule::new(
        "错误率规则",
        AlertCondition::ErrorRateAbove { threshold: 5.0 },
        AlertLevel::Warning,
        vec![AlertChannelType::Email],
    );
    manager.add_rule(rule).await;

    // 正常指标
    let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
    let alerts = manager.check_rules(&metrics_ok).await;
    assert!(alerts.is_empty());

    // 触发指标
    let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);
    let alerts = manager.check_rules(&metrics_trigger).await;
    assert_eq!(alerts.len(), 1);
}

#[tokio::test]
async fn test_manager_send_alert_silenced() {
    let manager = AlertManager::with_defaults();

    // 添加静默规则
    let silence = SilenceRule {
        id: "silence-1".to_string(),
        rule_pattern: "*".to_string(),
        start_time: chrono::Utc::now(),
        end_time: chrono::Utc::now() + chrono::Duration::hours(1),
        reason: "全站静默".to_string(),
        created_by: None,
    };
    manager.add_silence(silence).await;

    let alert = Alert::new(AlertLevel::Warning, "测试告警", "测试消息");
    let entry = manager
        .send_alert(alert, None, Some("test-rule".to_string()))
        .await;

    assert!(entry.silenced);
}

// ============ 通道配置测试 ============

#[test]
fn test_email_channel_config() {
    let config = serde_json::json!({
        "smtp_host": "smtp.example.com",
        "smtp_port": 587,
        "smtp_user": "user@example.com",
        "smtp_password": "password",
        "from_address": "alerts@example.com",
        "recipients": ["admin@example.com", "ops@example.com"],
        "use_tls": true
    });

    let parsed: Result<foxnio::alert::channels::EmailChannelConfig, _> =
        serde_json::from_value(config);
    assert!(parsed.is_ok());
}

#[test]
fn test_webhook_channel_config() {
    let config = serde_json::json!({
        "url": "https://example.com/webhook",
        "method": "POST",
        "headers": {
            "Authorization": "Bearer token",
            "X-Custom": "value"
        },
        "timeout_secs": 30
    });

    let parsed: Result<foxnio::alert::channels::WebhookChannelConfig, _> =
        serde_json::from_value(config);
    assert!(parsed.is_ok());
}

#[test]
fn test_dingtalk_channel_config() {
    let config = serde_json::json!({
        "webhook_url": "https://oapi.dingtalk.com/robot/send?access_token=xxx",
        "secret": "my_secret_key",
        "at_mobiles": ["13800138000"],
        "at_all": false
    });

    let parsed: Result<foxnio::alert::channels::DingTalkChannelConfig, _> =
        serde_json::from_value(config);
    assert!(parsed.is_ok());
}

#[test]
fn test_feishu_channel_config() {
    let config = serde_json::json!({
        "webhook_url": "https://open.feishu.cn/open-apis/bot/v2/hook/xxx",
        "at_users": ["ou_xxx", "ou_yyy"],
        "at_all": false
    });

    let parsed: Result<foxnio::alert::channels::FeishuChannelConfig, _> =
        serde_json::from_value(config);
    assert!(parsed.is_ok());
}

#[test]
fn test_slack_channel_config() {
    let config = serde_json::json!({
        "webhook_url": "https://hooks.slack.com/services/xxx/yyy/zzz",
        "channel": "#alerts",
        "username": "AlertBot",
        "icon_emoji": ":warning:"
    });

    let parsed: Result<foxnio::alert::channels::SlackChannelConfig, _> =
        serde_json::from_value(config);
    assert!(parsed.is_ok());
}

// ============ 边界条件测试 ============

#[test]
fn test_condition_boundary_values() {
    // 边界值测试
    let condition = AlertCondition::ErrorRateAbove { threshold: 0.0 };
    assert!(condition.evaluate(&MetricsSnapshot::new().with_error_rate(0.1)));
    assert!(!condition.evaluate(&MetricsSnapshot::new().with_error_rate(0.0)));

    // ConnectionCountBelow 使用 < 比较，所以 threshold=0 时，只有 active_connections < 0 才为 true
    // 由于 active_connections 不可能为负数，所以 threshold=0 时永远为 false
    // 改用 threshold=1 来测试连接数为 0 的情况
    let condition = AlertCondition::ConnectionCountBelow { threshold: 1 };
    assert!(condition.evaluate(&MetricsSnapshot::new().with_connections(0)));
    assert!(!condition.evaluate(&MetricsSnapshot::new().with_connections(1)));
}

#[test]
fn test_metrics_snapshot_default() {
    let metrics = MetricsSnapshot::new();
    assert_eq!(metrics.error_rate, 0.0);
    assert_eq!(metrics.avg_latency_ms, 0);
    assert_eq!(metrics.active_connections, 0);
    assert_eq!(metrics.account_balance, 0);
    assert_eq!(metrics.cpu_usage, 0.0);
    assert_eq!(metrics.memory_usage, 0.0);
    assert_eq!(metrics.disk_usage, 0.0);
    assert_eq!(metrics.request_rate, 0.0);
}

#[test]
fn test_alert_empty_labels() {
    let alert = Alert::new(AlertLevel::Info, "标题", "消息");
    assert!(alert.labels.is_empty());

    let json = serde_json::to_value(&alert).unwrap();
    assert!(json["labels"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn test_history_entry_results() {
    let mut entry =
        AlertHistoryEntry::new(Alert::new(AlertLevel::Warning, "测试", "test"), None, None);

    assert!(entry.results.is_empty());
    assert!(!entry.is_all_success());

    // 添加成功结果
    entry.add_result(AlertSendResult::success(AlertChannelType::Email));
    assert!(entry.is_all_success());
    assert_eq!(entry.success_count(), 1);

    // 添加失败结果
    entry.add_result(AlertSendResult::failure(
        AlertChannelType::Slack,
        "Connection error",
    ));
    assert!(!entry.is_all_success());
    assert_eq!(entry.success_count(), 1);
    assert_eq!(entry.failure_count(), 1);
}

// ============ 性能测试 ============

#[tokio::test]
async fn test_history_performance() {
    let history = AlertHistory::new(10000);

    // 批量添加
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let alert = Alert::new(AlertLevel::Info, format!("告警 {}", i), "test");
        history.add(AlertHistoryEntry::new(alert, None, None)).await;
    }
    let add_duration = start.elapsed();

    // 查询
    let start = std::time::Instant::now();
    let filter = AlertHistoryFilter {
        limit: Some(100),
        ..Default::default()
    };
    let results = history.query(&filter).await;
    let query_duration = start.elapsed();

    // 统计
    let start = std::time::Instant::now();
    let stats = history.stats(None, None).await;
    let stats_duration = start.elapsed();

    println!("Add 1000 entries: {:?}", add_duration);
    println!("Query 100 entries: {:?}", query_duration);
    println!("Stats: {:?}", stats_duration);

    assert_eq!(results.len(), 100);
    assert_eq!(stats.total_count, 1000);

    // 性能断言（宽松）
    assert!(add_duration < std::time::Duration::from_secs(5));
    assert!(query_duration < std::time::Duration::from_secs(1));
    assert!(stats_duration < std::time::Duration::from_secs(1));
}
