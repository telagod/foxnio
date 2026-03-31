//! 告警通道模块
//!
//! 支持多种告警通知通道
//!
//! 预留功能：告警通道（扩展功能）

#![allow(dead_code)]

pub mod dingtalk;
pub mod email;
pub mod feishu;
pub mod slack;
pub mod webhook;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Alert, AlertChannelType, AlertSendResult};

pub use dingtalk::DingTalkChannel;
pub use email::EmailChannel;
pub use feishu::FeishuChannel;
pub use slack::SlackChannel;
pub use webhook::WebhookChannel;

/// 告警通道 trait

#[async_trait]
pub trait AlertChannel: Send + Sync {
    /// 发送告警
    async fn send(&self, alert: &Alert) -> AlertSendResult;

    /// 获取通道类型
    fn channel_type(&self) -> AlertChannelType;

    /// 获取通道名称
    fn name(&self) -> &str;

    /// 检查通道是否可用
    fn is_available(&self) -> bool {
        true
    }
}

/// 创建告警通道
pub fn create_channel(
    channel_type: AlertChannelType,
    config: &serde_json::Value,
) -> Result<Box<dyn AlertChannel>, String> {
    match channel_type {
        AlertChannelType::Email => {
            let config: EmailChannelConfig = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid email config: {e}"))?;
            Ok(Box::new(EmailChannel::new(config)))
        }
        AlertChannelType::Webhook => {
            let config: WebhookChannelConfig = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid webhook config: {e}"))?;
            Ok(Box::new(WebhookChannel::new(config)))
        }
        AlertChannelType::DingTalk => {
            let config: DingTalkChannelConfig = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid dingtalk config: {e}"))?;
            Ok(Box::new(DingTalkChannel::new(config)))
        }
        AlertChannelType::Feishu => {
            let config: FeishuChannelConfig = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid feishu config: {e}"))?;
            Ok(Box::new(FeishuChannel::new(config)))
        }
        AlertChannelType::Slack => {
            let config: SlackChannelConfig = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid slack config: {e}"))?;
            Ok(Box::new(SlackChannel::new(config)))
        }
    }
}

// ============ 配置结构体 ============

/// 邮件通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct EmailChannelConfig {
    /// SMTP 服务器地址
    pub smtp_host: String,
    /// SMTP 端口
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    /// SMTP 用户名
    pub smtp_user: String,
    /// SMTP 密码
    pub smtp_password: String,
    /// 发件人地址
    pub from_address: String,
    /// 收件人列表
    pub recipients: Vec<String>,
    /// 是否使用 TLS
    #[serde(default = "default_true")]
    pub use_tls: bool,
}
fn default_smtp_port() -> u16 {
    587
}
fn default_true() -> bool {
    true
}

/// Webhook 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct WebhookChannelConfig {
    /// Webhook URL
    pub url: String,
    /// HTTP 方法
    #[serde(default = "default_method")]
    pub method: String,
    /// 自定义请求头
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// 请求超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}
fn default_method() -> String {
    "POST".to_string()
}
fn default_timeout() -> u64 {
    30
}

/// 钉钉机器人配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkChannelConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// 加签密钥（可选）
    pub secret: Option<String>,
    /// @ 用户列表（手机号）
    #[serde(default)]
    pub at_mobiles: Vec<String>,
    /// @ 所有人
    #[serde(default)]
    pub at_all: bool,
}

/// 飞书机器人配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuChannelConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// 应用 ID（可选，用于发送更丰富的消息）
    pub app_id: Option<String>,
    /// 应用密钥（可选）
    pub app_secret: Option<String>,
    /// @ 用户列表（open_id）
    #[serde(default)]
    pub at_users: Vec<String>,
    /// @ 所有人
    #[serde(default)]
    pub at_all: bool,
}

/// Slack Webhook 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannelConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// 通道名称（可选，覆盖 webhook 默认通道）
    pub channel: Option<String>,
    /// 用户名（可选，覆盖 webhook 默认用户名）
    pub username: Option<String>,
    /// 图标 emoji（可选）
    pub icon_emoji: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_email_channel() {
        let config = serde_json::json!({
            "smtp_host": "smtp.example.com",
            "smtp_port": 587,
            "smtp_user": "user@example.com",
            "smtp_password": "password",
            "from_address": "alerts@example.com",
            "recipients": ["admin@example.com"]
        });

        let channel = create_channel(AlertChannelType::Email, &config);
        assert!(channel.is_ok());
    }

    #[test]
    fn test_create_webhook_channel() {
        let config = serde_json::json!({
            "url": "https://example.com/webhook",
            "method": "POST",
            "timeout_secs": 30
        });

        let channel = create_channel(AlertChannelType::Webhook, &config);
        assert!(channel.is_ok());
    }

    #[test]
    fn test_create_dingtalk_channel() {
        let config = serde_json::json!({
            "webhook_url": "https://oapi.dingtalk.com/robot/send?access_token=xxx",
            "secret": "mysecret",
            "at_all": false
        });

        let channel = create_channel(AlertChannelType::DingTalk, &config);
        assert!(channel.is_ok());
    }

    #[test]
    fn test_create_feishu_channel() {
        let config = serde_json::json!({
            "webhook_url": "https://open.feishu.cn/open-apis/bot/v2/hook/xxx"
        });

        let channel = create_channel(AlertChannelType::Feishu, &config);
        assert!(channel.is_ok());
    }

    #[test]
    fn test_create_slack_channel() {
        let config = serde_json::json!({
            "webhook_url": "https://hooks.slack.com/services/xxx/yyy/zzz",
            "username": "AlertBot",
            "icon_emoji": ":warning:"
        });

        let channel = create_channel(AlertChannelType::Slack, &config);
        assert!(channel.is_ok());
    }
}
