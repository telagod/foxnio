//! 告警系统模块
//!
//! 提供完整的告警功能，包括：
//! - 告警级别定义
//! - 告警规则引擎
//! - 多通道告警发送
//! - 告警历史记录
//! - 告警静默机制
//!
//! 预留功能：告警系统（扩展功能）

#![allow(dead_code)]

pub mod channels;
pub mod history;
pub mod manager;
pub mod rules;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 告警级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AlertLevel {
    /// 信息级别 - 一般性通知
    #[default]
    Info,
    /// 警告级别 - 需要关注但不紧急
    Warning,
    /// 错误级别 - 需要立即处理的问题
    Error,
    /// 严重级别 - 系统关键故障
    Critical,
}

impl AlertLevel {
    /// 获取告警级别的显示名称
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// 从字符串解析告警级别
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    /// 获取告警级别的图标
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ️",
            Self::Warning => "⚠️",
            Self::Error => "❌",
            Self::Critical => "🔥",
        }
    }

    /// 获取告警级别的颜色（用于终端显示）
    pub fn color(&self) -> &'static str {
        match self {
            Self::Info => "\x1b[34m",     // 蓝色
            Self::Warning => "\x1b[33m",  // 黄色
            Self::Error => "\x1b[31m",    // 红色
            Self::Critical => "\x1b[35m", // 紫色
        }
    }

    /// 判断是否为高优先级告警
    pub fn is_high_priority(&self) -> bool {
        matches!(self, Self::Error | Self::Critical)
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 告警结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// 告警级别
    pub level: AlertLevel,
    /// 告警标题
    pub title: String,
    /// 告警消息内容
    pub message: String,
    /// 告警来源
    pub source: String,
    /// 告警时间戳
    pub timestamp: DateTime<Utc>,
    /// 告警标签
    pub labels: HashMap<String, String>,
}

impl Alert {
    /// 创建新的告警
    pub fn new(level: AlertLevel, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            title: title.into(),
            message: message.into(),
            source: "foxnio".to_string(),
            timestamp: Utc::now(),
            labels: HashMap::new(),
        }
    }

    /// 设置告警来源
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// 添加标签
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// 格式化为简洁字符串
    pub fn to_summary(&self) -> String {
        format!(
            "[{}] {} - {}",
            self.level.as_str().to_uppercase(),
            self.title,
            self.message
        )
    }

    /// 格式化为详细字符串
    pub fn to_detailed(&self) -> String {
        let mut labels_str = String::new();
        if !self.labels.is_empty() {
            labels_str = format!(
                " Labels: {}",
                self.labels
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        format!(
            "{} [{}] {}\n来源: {}\n时间: {}{}\n详情: {}",
            self.level.icon(),
            self.level.as_str().to_uppercase(),
            self.title,
            self.source,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            labels_str,
            self.message
        )
    }
}

/// 告警通道类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertChannelType {
    /// 邮件告警
    Email,
    /// HTTP Webhook
    Webhook,
    /// 钉钉机器人
    DingTalk,
    /// 飞书机器人
    Feishu,
    /// Slack Webhook
    Slack,
}

impl AlertChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Webhook => "webhook",
            Self::DingTalk => "dingtalk",
            Self::Feishu => "feishu",
            Self::Slack => "slack",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "email" => Some(Self::Email),
            "webhook" => Some(Self::Webhook),
            "dingtalk" | "ding_ding" | "dingding" => Some(Self::DingTalk),
            "feishu" | "lark" => Some(Self::Feishu),
            "slack" => Some(Self::Slack),
            _ => None,
        }
    }
}

impl std::fmt::Display for AlertChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 告警发送结果
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct AlertSendResult {
    /// 是否成功
    pub success: bool,
    /// 通道类型
    pub channel_type: AlertChannelType,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 发送时间
    pub timestamp: DateTime<Utc>,
}

impl AlertSendResult {
    pub fn success(channel_type: AlertChannelType) -> Self {
        Self {
            success: true,
            channel_type,
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn failure(channel_type: AlertChannelType, error: impl Into<String>) -> Self {
        Self {
            success: false,
            channel_type,
            error: Some(error.into()),
            timestamp: Utc::now(),
        }
    }
}

/// 静默规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilenceRule {
    /// 静默规则 ID
    pub id: String,
    /// 静默的规则名称匹配模式
    pub rule_pattern: String,
    /// 静默开始时间
    pub start_time: DateTime<Utc>,
    /// 静默结束时间
    pub end_time: DateTime<Utc>,
    /// 静默原因
    pub reason: String,
    /// 创建者
    pub created_by: Option<String>,
}

impl SilenceRule {
    /// 检查静默规则是否生效
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.start_time && now <= self.end_time
    }

    /// 检查是否匹配规则名称
    pub fn matches(&self, rule_name: &str) -> bool {
        // 支持简单的通配符匹配
        if self.rule_pattern == "*" {
            return true;
        }

        if self.rule_pattern.starts_with('*') && self.rule_pattern.ends_with('*') {
            let pattern = &self.rule_pattern[1..self.rule_pattern.len() - 1];
            return rule_name.contains(pattern);
        }

        if self.rule_pattern.starts_with('*') {
            let pattern = &self.rule_pattern[1..];
            return rule_name.ends_with(pattern);
        }

        if self.rule_pattern.ends_with('*') {
            let pattern = &self.rule_pattern[..self.rule_pattern.len() - 1];
            return rule_name.starts_with(pattern);
        }

        rule_name == self.rule_pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_level() {
        assert_eq!(AlertLevel::Info.as_str(), "info");
        assert_eq!(AlertLevel::parse("warning"), Some(AlertLevel::Warning));
        assert!(AlertLevel::Critical.is_high_priority());
        assert!(!AlertLevel::Info.is_high_priority());
    }

    #[test]
    fn test_alert_creation() {
        let alert = Alert::new(AlertLevel::Warning, "测试告警", "这是一个测试告警消息")
            .with_source("test_module")
            .with_label("environment", "production");

        assert_eq!(alert.level, AlertLevel::Warning);
        assert_eq!(alert.title, "测试告警");
        assert_eq!(alert.source, "test_module");
        assert_eq!(
            alert.labels.get("environment"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_alert_formatting() {
        let alert = Alert::new(AlertLevel::Error, "系统错误", "数据库连接失败");

        let summary = alert.to_summary();
        assert!(summary.contains("ERROR"));
        assert!(summary.contains("系统错误"));

        let detailed = alert.to_detailed();
        assert!(detailed.contains("数据库连接失败"));
    }

    #[test]
    fn test_silence_rule() {
        let silence = SilenceRule {
            id: "silence-1".to_string(),
            rule_pattern: "db-*".to_string(),
            start_time: Utc::now() - chrono::Duration::hours(1),
            end_time: Utc::now() + chrono::Duration::hours(1),
            reason: "维护窗口".to_string(),
            created_by: Some("admin".to_string()),
        };

        assert!(silence.is_active());
        assert!(silence.matches("db-connection-error"));
        assert!(!silence.matches("api-timeout"));
    }

    #[test]
    fn test_alert_channel_type() {
        assert_eq!(
            AlertChannelType::parse("feishu"),
            Some(AlertChannelType::Feishu)
        );
        assert_eq!(
            AlertChannelType::parse("lark"),
            Some(AlertChannelType::Feishu)
        );
        assert_eq!(
            AlertChannelType::parse("dingding"),
            Some(AlertChannelType::DingTalk)
        );
    }

    #[test]
    fn test_alert_send_result() {
        let success = AlertSendResult::success(AlertChannelType::Email);
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = AlertSendResult::failure(AlertChannelType::Slack, "Connection timeout");
        assert!(!failure.success);
        assert_eq!(failure.error, Some("Connection timeout".to_string()));
    }
}
