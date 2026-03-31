//! 飞书机器人告警通道

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{AlertChannel, AlertSendResult, FeishuChannelConfig};
use crate::alert::{Alert, AlertChannelType};

/// 飞书消息格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeishuMessageFormat {
    /// 交互式卡片（默认）
    #[default]
    Interactive,
    /// 富文本消息
    Post,
    /// 纯文本消息
    Text,
}

/// 飞书告警通道
pub struct FeishuChannel {
    config: FeishuChannelConfig,
    client: Client,
    name: String,
    /// 消息格式
    format: FeishuMessageFormat,
}

impl FeishuChannel {
    /// 创建新的飞书通道（使用默认交互式卡片格式）
    pub fn new(config: FeishuChannelConfig) -> Self {
        Self::with_format(config, FeishuMessageFormat::default())
    }

    /// 创建指定消息格式的飞书通道
    pub fn with_format(config: FeishuChannelConfig, format: FeishuMessageFormat) -> Self {
        let name = "Feishu".to_string();
        let client = Client::new();
        Self {
            config,
            client,
            name,
            format,
        }
    }

    /// 构建交互式卡片消息
    fn build_interactive_card(&self, alert: &Alert) -> serde_json::Value {
        let level_color = match alert.level {
            crate::alert::AlertLevel::Info => "blue",
            crate::alert::AlertLevel::Warning => "yellow",
            crate::alert::AlertLevel::Error => "red",
            crate::alert::AlertLevel::Critical => "purple",
        };

        // 构建标签内容
        let _tags_content = if alert.labels.is_empty() {
            String::new()
        } else {
            let tags: Vec<String> = alert
                .labels
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect();
            format!(
                r#",{{
                    "tag": "div",
                    "text": {{
                        "tag": "lark_md",
                        "content": "**标签:** {}"
                    }}
                }}"#,
                tags.join(" | ")
            )
        };

        // 构建 @ 用户内容
        let _at_content = if self.config.at_all {
            r#",{ "tag": "div", "text": { "tag": "lark_md", "content": "<at user_id=\"all\">所有人</at>" } }"#.to_string()
        } else if !self.config.at_users.is_empty() {
            let ats: Vec<String> = self
                .config
                .at_users
                .iter()
                .map(|user_id| format!("<at user_id=\"{}\"></at>", user_id))
                .collect();
            format!(
                r#",{{
                    "tag": "div",
                    "text": {{
                        "tag": "lark_md",
                        "content": "{}"
                    }}
                }}"#,
                ats.join(" ")
            )
        } else {
            String::new()
        };

        serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "config": {
                    "wide_screen_mode": true
                },
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": format!("{} {}", alert.level.icon(), alert.title)
                    },
                    "template": level_color
                },
                "elements": [
                    {
                        "tag": "div",
                        "fields": [
                            {
                                "is_short": true,
                                "text": {
                                    "tag": "lark_md",
                                    "content": format!("**级别:**\n{}", alert.level.as_str().to_uppercase())
                                }
                            },
                            {
                                "is_short": true,
                                "text": {
                                    "tag": "lark_md",
                                    "content": format!("**来源:**\n{}", alert.source)
                                }
                            }
                        ]
                    },
                    {
                        "tag": "div",
                        "text": {
                            "tag": "lark_md",
                            "content": format!("**时间:**\n{}", alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC"))
                        }
                    },
                    {
                        "tag": "div",
                        "text": {
                            "tag": "lark_md",
                            "content": format!("**详情:**\n{}", alert.message)
                        }
                    }
                ]
            }
        })
    }

    /// 构建富文本消息（备选格式）
    pub fn build_post_message(&self, alert: &Alert) -> serde_json::Value {
        let _level_color = match alert.level {
            crate::alert::AlertLevel::Info => "blue",
            crate::alert::AlertLevel::Warning => "yellow",
            crate::alert::AlertLevel::Error => "red",
            crate::alert::AlertLevel::Critical => "purple",
        };

        let mut content_lines = vec![
            vec![
                serde_json::json!({ "tag": "text", "text": format!("{} ", alert.level.icon()) }),
                serde_json::json!({ "tag": "text", "text": &alert.title }),
            ],
            vec![serde_json::json!({ "tag": "text", "text": "" })],
            vec![
                serde_json::json!({ "tag": "text", "text": format!("级别: {}", alert.level.as_str().to_uppercase()) }),
            ],
            vec![serde_json::json!({ "tag": "text", "text": format!("来源: {}", alert.source) })],
            vec![
                serde_json::json!({ "tag": "text", "text": format!("时间: {}", alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC")) }),
            ],
            vec![serde_json::json!({ "tag": "text", "text": "" })],
            vec![serde_json::json!({ "tag": "text", "text": format!("详情: {}", alert.message) })],
        ];

        // 添加标签
        if !alert.labels.is_empty() {
            content_lines.push(vec![serde_json::json!({ "tag": "text", "text": "" })]);
            let labels: Vec<String> = alert
                .labels
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            content_lines.push(vec![
                serde_json::json!({ "tag": "text", "text": format!("标签: {}", labels.join(", ")) }),
            ]);
        }

        // 添加 @
        if self.config.at_all {
            content_lines.push(vec![serde_json::json!({ "tag": "text", "text": "" })]);
            content_lines.push(vec![serde_json::json!({ "tag": "at", "user_id": "all" })]);
        } else if !self.config.at_users.is_empty() {
            content_lines.push(vec![serde_json::json!({ "tag": "text", "text": "" })]);
            let ats: Vec<serde_json::Value> = self
                .config
                .at_users
                .iter()
                .map(|user_id| serde_json::json!({ "tag": "at", "user_id": user_id }))
                .collect();
            content_lines.push(ats);
        }

        serde_json::json!({
            "msg_type": "post",
            "content": {
                "post": {
                    "zh_cn": {
                        "title": format!("告警通知 - {}", alert.title),
                        "content": content_lines
                    }
                }
            }
        })
    }

    /// 构建文本消息（备选格式）
    pub fn build_text_message(&self, alert: &Alert) -> serde_json::Value {
        let mut text = alert.to_detailed();

        if self.config.at_all {
            text.push_str("\n<at user_id=\"all\">所有人</at>");
        } else if !self.config.at_users.is_empty() {
            for user_id in &self.config.at_users {
                text.push_str(&format!("\n<at user_id=\"{}\"></at>", user_id));
            }
        }

        serde_json::json!({
            "msg_type": "text",
            "content": {
                "text": text
            }
        })
    }
}

#[async_trait]
impl AlertChannel for FeishuChannel {
    async fn send(&self, alert: &Alert) -> AlertSendResult {
        let body = match self.format {
            FeishuMessageFormat::Interactive => self.build_interactive_card(alert),
            FeishuMessageFormat::Post => self.build_post_message(alert),
            FeishuMessageFormat::Text => self.build_text_message(alert),
        };

        match self
            .client
            .post(&self.config.webhook_url)
            .json(&body)
            .send()
            .await
        {
            Ok(response) => match response.json::<FeishuResponse>().await {
                Ok(resp) => {
                    if resp.code == 0 {
                        AlertSendResult::success(AlertChannelType::Feishu)
                    } else {
                        AlertSendResult::failure(
                            AlertChannelType::Feishu,
                            format!("Error {}: {}", resp.code, resp.msg),
                        )
                    }
                }
                Err(e) => AlertSendResult::failure(
                    AlertChannelType::Feishu,
                    format!("Failed to parse response: {e}"),
                ),
            },
            Err(e) => {
                AlertSendResult::failure(AlertChannelType::Feishu, format!("Request failed: {e}"))
            }
        }
    }

    fn channel_type(&self) -> AlertChannelType {
        AlertChannelType::Feishu
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        !self.config.webhook_url.is_empty()
    }
}

/// 飞书响应
#[derive(Debug, serde::Deserialize)]
struct FeishuResponse {
    code: i64,
    msg: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::AlertLevel;

    fn create_test_config() -> FeishuChannelConfig {
        FeishuChannelConfig {
            webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/test".to_string(),
            app_id: None,
            app_secret: None,
            at_users: vec!["ou_xxx".to_string()],
            at_all: false,
        }
    }

    fn create_test_alert() -> Alert {
        Alert::new(AlertLevel::Warning, "性能警告", "响应时间超过阈值")
            .with_source("monitoring")
            .with_label("service", "api-gateway")
    }

    #[test]
    fn test_feishu_channel_creation() {
        let config = create_test_config();
        let channel = FeishuChannel::new(config);

        assert_eq!(channel.channel_type(), AlertChannelType::Feishu);
        assert!(channel.is_available());
    }

    #[test]
    fn test_build_interactive_card() {
        let config = create_test_config();
        let channel = FeishuChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_interactive_card(&alert);

        assert_eq!(body["msg_type"], "interactive");
        assert!(body["card"]["header"]["title"]["content"]
            .as_str()
            .unwrap()
            .contains("性能警告"));
    }

    #[test]
    fn test_build_post_message() {
        let config = create_test_config();
        let channel = FeishuChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_post_message(&alert);

        assert_eq!(body["msg_type"], "post");
        assert!(body["content"]["post"]["zh_cn"]["title"]
            .as_str()
            .unwrap()
            .contains("性能警告"));
    }

    #[test]
    fn test_build_text_message() {
        let config = create_test_config();
        let channel = FeishuChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_text_message(&alert);

        assert_eq!(body["msg_type"], "text");
        assert!(body["content"]["text"]
            .as_str()
            .unwrap()
            .contains("性能警告"));
    }

    #[test]
    fn test_at_all() {
        let mut config = create_test_config();
        config.at_all = true;
        let channel = FeishuChannel::new(config);
        let alert = create_test_alert();

        let body = channel.build_interactive_card(&alert);
        // 检查消息构建正常
        assert_eq!(body["msg_type"], "interactive");
    }

    #[test]
    fn test_channel_not_available() {
        let config = FeishuChannelConfig {
            webhook_url: "".to_string(),
            app_id: None,
            app_secret: None,
            at_users: vec![],
            at_all: false,
        };

        let channel = FeishuChannel::new(config);
        assert!(!channel.is_available());
    }
}
