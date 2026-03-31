//! 邮件告警通道

use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use super::{AlertChannel, AlertSendResult, EmailChannelConfig};
use crate::alert::{Alert, AlertChannelType};

/// 邮件告警通道
pub struct EmailChannel {
    config: EmailChannelConfig,
    name: String,
}

impl EmailChannel {
    pub fn new(config: EmailChannelConfig) -> Self {
        let name = format!("Email:{}", config.from_address);
        Self { config, name }
    }

    /// 构建邮件消息
    fn build_message(&self, alert: &Alert) -> Result<Message, String> {
        let subject = format!(
            "[{}] {} - {}",
            alert.level.as_str().to_uppercase(),
            alert.source,
            alert.title
        );

        // 纯文本内容
        let text_body = alert.to_detailed();

        // HTML 内容
        let html_body = self.format_html(alert);

        let mut message = Message::builder()
            .from(
                self.config
                    .from_address
                    .parse()
                    .map_err(|e| format!("Invalid from address: {e}"))?,
            )
            .subject(subject);

        // 添加收件人
        for recipient in &self.config.recipients {
            message = message.to(recipient
                .parse()
                .map_err(|e| format!("Invalid recipient {recipient}: {e}"))?);
        }

        message
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )
            .map_err(|e| format!("Failed to build message: {e}"))
    }

    /// 格式化 HTML 邮件
    fn format_html(&self, alert: &Alert) -> String {
        let level_color = match alert.level {
            crate::alert::AlertLevel::Info => "#17a2b8",
            crate::alert::AlertLevel::Warning => "#ffc107",
            crate::alert::AlertLevel::Error => "#dc3545",
            crate::alert::AlertLevel::Critical => "#6f42c1",
        };

        let labels_html = if alert.labels.is_empty() {
            String::new()
        } else {
            let labels = alert
                .labels
                .iter()
                .map(|(k, v)| format!("<li><strong>{k}:</strong> {v}</li>"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r"<h3>标签</h3>
                <ul>{}</ul>",
                labels
            )
        };

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ background-color: {}; color: white; padding: 20px; border-radius: 8px 8px 0 0; }}
        .content {{ background-color: #f8f9fa; padding: 20px; border-radius: 0 0 8px 8px; }}
        .info {{ margin-bottom: 10px; }}
        .label {{ font-weight: bold; color: #6c757d; }}
        pre {{ background-color: #e9ecef; padding: 15px; border-radius: 4px; overflow-x: auto; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1 style="margin: 0;">{} {}</h1>
            <p style="margin: 10px 0 0 0;">{}</p>
        </div>
        <div class="content">
            <div class="info">
                <span class="label">级别:</span> {}
            </div>
            <div class="info">
                <span class="label">来源:</span> {}
            </div>
            <div class="info">
                <span class="label">时间:</span> {}
            </div>
            <h3>详细信息</h3>
            <pre>{}</pre>
            {}
        </div>
    </div>
</body>
</html>"#,
            level_color,
            alert.level.icon(),
            alert.title,
            alert.source,
            alert.level.as_str().to_uppercase(),
            alert.source,
            alert.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            alert.message,
            labels_html
        )
    }
}

#[async_trait]
impl AlertChannel for EmailChannel {
    async fn send(&self, alert: &Alert) -> AlertSendResult {
        // 构建邮件消息
        let message = match self.build_message(alert) {
            Ok(m) => m,
            Err(e) => return AlertSendResult::failure(AlertChannelType::Email, e),
        };

        // 创建 SMTP 传输
        let transport_result: Result<AsyncSmtpTransport<Tokio1Executor>, String> =
            if self.config.use_tls {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
                    .map(|t| {
                        t.credentials(lettre::transport::smtp::authentication::Credentials::new(
                            self.config.smtp_user.clone(),
                            self.config.smtp_password.clone(),
                        ))
                        .port(self.config.smtp_port)
                        .build()
                    })
                    .map_err(|e| format!("SMTP relay error: {e}"))
            } else {
                Ok(
                    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.smtp_host)
                        .credentials(lettre::transport::smtp::authentication::Credentials::new(
                            self.config.smtp_user.clone(),
                            self.config.smtp_password.clone(),
                        ))
                        .port(self.config.smtp_port)
                        .build(),
                )
            };

        let transport = match transport_result {
            Ok(t) => t,
            Err(e) => return AlertSendResult::failure(AlertChannelType::Email, e),
        };

        // 发送邮件
        match transport.send(message).await {
            Ok(_) => AlertSendResult::success(AlertChannelType::Email),
            Err(e) => AlertSendResult::failure(AlertChannelType::Email, format!("Send error: {e}")),
        }
    }

    fn channel_type(&self) -> AlertChannelType {
        AlertChannelType::Email
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        !self.config.smtp_host.is_empty()
            && !self.config.smtp_user.is_empty()
            && !self.config.recipients.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::AlertLevel;

    fn create_test_config() -> EmailChannelConfig {
        EmailChannelConfig {
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_user: "user@example.com".to_string(),
            smtp_password: "password".to_string(),
            from_address: "alerts@example.com".to_string(),
            recipients: vec!["admin@example.com".to_string()],
            use_tls: true,
        }
    }

    fn create_test_alert() -> Alert {
        Alert::new(AlertLevel::Warning, "测试告警", "这是一条测试告警消息")
            .with_source("test_module")
            .with_label("environment", "production")
    }

    #[test]
    fn test_email_channel_creation() {
        let config = create_test_config();
        let channel = EmailChannel::new(config);

        assert_eq!(channel.channel_type(), AlertChannelType::Email);
        assert!(channel.is_available());
    }

    #[test]
    fn test_build_message() {
        let config = create_test_config();
        let channel = EmailChannel::new(config);
        let alert = create_test_alert();

        let message = channel.build_message(&alert);
        assert!(message.is_ok());

        // Verify message was built successfully
        // Note: lettre::Message doesn't expose subject() getter
        // The subject is built with format: "[LEVEL] source - title"
        let _message = message.unwrap();
    }

    #[test]
    fn test_format_html() {
        let config = create_test_config();
        let channel = EmailChannel::new(config);
        let alert = create_test_alert();

        let html = channel.format_html(&alert);

        assert!(html.contains("测试告警"));
        assert!(html.contains("production"));
        assert!(html.contains("#ffc107")); // Warning color
    }

    #[test]
    fn test_channel_not_available() {
        let config = EmailChannelConfig {
            smtp_host: "".to_string(),
            smtp_port: 587,
            smtp_user: "user".to_string(),
            smtp_password: "pass".to_string(),
            from_address: "a@b.c".to_string(),
            recipients: vec![],
            use_tls: true,
        };

        let channel = EmailChannel::new(config);
        assert!(!channel.is_available());
    }
}
