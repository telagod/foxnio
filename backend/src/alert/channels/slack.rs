//! Slack Webhook 告警通道

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{AlertChannel, AlertSendResult, SlackChannelConfig};
use crate::alert::{Alert, AlertChannelType};

/// Slack 消息格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackMessageFormat {
    /// Block Kit 格式（默认）
    #[default]
    Blocks,
    /// 附件格式
    Attachment,
}

/// Slack 告警通道
pub struct SlackChannel {
    config: SlackChannelConfig,
    client: Client,
    name: String,
    /// 消息格式
    format: SlackMessageFormat,
}

impl SlackChannel {
    /// 创建新的 Slack 通道（使用默认 Block Kit 格式）
    pub fn new(config: SlackChannelConfig) -> Self {
        Self::with_format(config, SlackMessageFormat::default())
    }

    /// 创建指定消息格式的 Slack 通道
    pub fn with_format(config: SlackChannelConfig, format: SlackMessageFormat) -> Self {
        let name = "Slack".to_string();
        let client = Client::new();
        Self {
            config,
            client,
            name,
            format,
        }
    }

    /// 获取级别对应的颜色
    fn get_level_color(&self, level: &crate::alert::AlertLevel) -> &'static str {
        match level {
            crate::alert::AlertLevel::Info => "#2196F3",
            crate::alert::AlertLevel::Warning => "#FFC107",
            crate::alert::AlertLevel::Error => "#F44336",
            crate::alert::AlertLevel::Critical => "#9C27B0",
        }
    }

    /// 构建 Slack Block Kit 消息
    fn build_blocks_message(&self, alert: &Alert) -> serde_json::Value {
        let level_color = self.get_level_color(&alert.level);

        // 构建标签字段
        let mut fields = vec![
            serde_json::json!({
                "type": "mrkdwn",
                "text": format!("*级别:*\n{}", alert.level.as_str().to_uppercase())
            }),
            serde_json::json!({
                "type": "mrkdwn",
                "text": format!("*来源:*\n{}", alert.source)
            }),
            serde_json::json!({
                "type": "mrkdwn",
                "text": format!("*时间:*\n{}", alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC"))
            }),
        ];

        // 添加标签
        if !alert.labels.is_empty() {
            let labels: Vec<String> = alert
                .labels
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            fields.push(serde_json::json!({
                "type": "mrkdwn",
                "text": format!("*标签:*\n{}", labels.join(", "))
            }));
        }

        let mut blocks = vec![
            // 标题区块
            serde_json::json!({
                "type": "header",
                "text": {
                    "type": "plain_text",
                    "text": format!("{} {}", alert.level.icon(), alert.title),
                    "emoji": true
                }
            }),
            // 分割线
            serde_json::json!({
                "type": "divider"
            }),
            // 字段区块
            serde_json::json!({
                "type": "section",
                "fields": fields
            }),
        ];

        // 消息详情区块
        blocks.push(serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": format!("*详情:*\n```{}```", alert.message)
            }
        }));

        // 页脚区块
        blocks.push(serde_json::json!({
            "type": "context",
            "elements": [
                {
                    "type": "mrkdwn",
                    "text": format!("🦊 FoxNIO Alert System | {}", alert.timestamp.format("%Y-%m-%d"))
                }
            ]
        }));

        let mut body = serde_json::json!({
            "attachments": [
                {
                    "color": level_color,
                    "blocks": blocks
                }
            ]
        });

        // 可选的通道和用户名覆盖
        if let Some(channel) = &self.config.channel {
            body["channel"] = serde_json::json!(channel);
        }
        if let Some(username) = &self.config.username {
            body["username"] = serde_json::json!(username);
        }
        if let Some(icon) = &self.config.icon_emoji {
            body["icon_emoji"] = serde_json::json!(icon);
        }

        body
    }

    /// 构建简单附件消息（备选格式）
    pub fn build_attachment_message(&self, alert: &Alert) -> serde_json::Value {
        let level_color = self.get_level_color(&alert.level);

        // 构建字段
        let mut fields = vec![
            {
                let title = "级别";
                let value = alert.level.as_str().to_uppercase();
                serde_json::json!({
                    "title": title,
                    "value": value,
                    "short": true
                })
            },
            {
                let title = "来源";
                let value = alert.source.clone();
                serde_json::json!({
                    "title": title,
                    "value": value,
                    "short": true
                })
            },
            {
                let title = "时间";
                let value = alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                serde_json::json!({
                    "title": title,
                    "value": value,
                    "short": true
                })
            },
        ];

        // 添加标签字段
        if !alert.labels.is_empty() {
            let labels: Vec<String> = alert
                .labels
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            fields.push(serde_json::json!({
                "title": "标签",
                "value": labels.join(", "),
                "short": true
            }));
        }

        let mut body = serde_json::json!({
            "attachments": [
                {
                    "fallback": alert.to_summary(),
                    "color": level_color,
                    "pretext": format!("{} {}", alert.level.icon(), alert.level.as_str().to_uppercase()),
                    "title": alert.title,
                    "text": alert.message,
                    "fields": fields,
                    "footer": "FoxNIO Alert System",
                    "ts": alert.timestamp.timestamp()
                }
            ]
        });

        // 可选配置
        if let Some(channel) = &self.config.channel {
            body["channel"] = serde_json::json!(channel);
        }
        if let Some(username) = &self.config.username {
            body["username"] = serde_json::json!(username);
        }
        if let Some(icon) = &self.config.icon_emoji {
            body["icon_emoji"] = serde_json::json!(icon);
        }

        body
    }
}

#[async_trait]
impl AlertChannel for SlackChannel {
    async fn send(&self, alert: &Alert) -> AlertSendResult {
        let body = match self.format {
            SlackMessageFormat::Blocks => self.build_blocks_message(alert),
            SlackMessageFormat::Attachment => self.build_attachment_message(alert),
        };

        match self
            .client
            .post(&self.config.webhook_url)
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    // Slack webhook 成功返回 "ok"
                    match response.text().await {
                        Ok(text) if text == "ok" => {
                            AlertSendResult::success(AlertChannelType::Slack)
                        }
                        Ok(text) => AlertSendResult::failure(
                            AlertChannelType::Slack,
                            format!("Unexpected response: {text}"),
                        ),
                        Err(e) => AlertSendResult::failure(
                            AlertChannelType::Slack,
                            format!("Failed to read response: {e}"),
                        ),
                    }
                } else {
                    AlertSendResult::failure(
                        AlertChannelType::Slack,
                        format!(
                            "HTTP {}: {}",
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("Unknown")
                        ),
                    )
                }
            }
            Err(e) => {
                AlertSendResult::failure(AlertChannelType::Slack, format!("Request failed: {e}"))
            }
        }
    }

    fn channel_type(&self) -> AlertChannelType {
        AlertChannelType::Slack
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        !self.config.webhook_url.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::AlertLevel;

    fn create_test_config() -> SlackChannelConfig {
        SlackChannelConfig {
            webhook_url: "https://hooks.slack.com/services/xxx/yyy/zzz".to_string(),
            channel: Some("#alerts".to_string()),
            username: Some("AlertBot".to_string()),
            icon_emoji: Some(":warning:".to_string()),
        }
    }

    fn create_test_alert() -> Alert {
        Alert::new(AlertLevel::Error, "API 错误", "服务不可用")
            .with_source("api-gateway")
            .with_label("service", "payment")
    }

    #[test]
    fn test_slack_channel_creation() {
        let config = create_test_config();
        let channel = SlackChannel::new(config);

        assert_eq!(channel.channel_type(), AlertChannelType::Slack);
        assert!(channel.is_available());
    }

    #[test]
    fn test_build_blocks_message() {
        let config = create_test_config();
        let channel = SlackChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_blocks_message(&alert);

        // 检查结构
        assert!(body["attachments"].is_array());
        let attachment = &body["attachments"][0];
        assert_eq!(attachment["color"], "#F44336"); // Error color
        assert!(attachment["blocks"].is_array());
    }

    #[test]
    fn test_build_attachment_message() {
        let config = create_test_config();
        let channel = SlackChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_attachment_message(&alert);

        assert!(body["attachments"].is_array());
        let attachment = &body["attachments"][0];
        assert_eq!(attachment["title"], "API 错误");
        assert_eq!(attachment["color"], "#F44336");
    }

    #[test]
    fn test_level_colors() {
        let config = create_test_config();
        let channel = SlackChannel::new(config);

        assert_eq!(channel.get_level_color(&AlertLevel::Info), "#2196F3");
        assert_eq!(channel.get_level_color(&AlertLevel::Warning), "#FFC107");
        assert_eq!(channel.get_level_color(&AlertLevel::Error), "#F44336");
        assert_eq!(channel.get_level_color(&AlertLevel::Critical), "#9C27B0");
    }

    #[test]
    fn test_optional_config() {
        let config = create_test_config();
        let channel = SlackChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_blocks_message(&alert);

        assert_eq!(body["channel"], "#alerts");
        assert_eq!(body["username"], "AlertBot");
        assert_eq!(body["icon_emoji"], ":warning:");
    }

    #[test]
    fn test_channel_not_available() {
        let config = SlackChannelConfig {
            webhook_url: "".to_string(),
            channel: None,
            username: None,
            icon_emoji: None,
        };

        let channel = SlackChannel::new(config);
        assert!(!channel.is_available());
    }
}
