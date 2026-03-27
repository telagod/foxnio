//! 邮件发送服务

use anyhow::{bail, Result};
use chrono::Datelike;
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use serde::Deserialize;
use std::sync::Arc;

/// 邮件配置
#[derive(Debug, Clone, Deserialize)]
pub struct EmailConfig {
    /// SMTP 服务器地址
    pub host: String,
    /// SMTP 端口
    pub port: u16,
    /// SMTP 用户名
    pub user: String,
    /// SMTP 密码
    pub password: String,
    /// 发件人地址
    pub from_address: String,
    /// 发件人名称
    #[serde(default = "default_from_name")]
    pub from_name: String,
    /// 是否启用邮件发送（测试环境可关闭）
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 密码重置 URL 基础路径
    #[serde(default)]
    pub reset_url_base: String,
}

fn default_from_name() -> String {
    "FoxNIO".to_string()
}

fn default_enabled() -> bool {
    true
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            host: "smtp.example.com".to_string(),
            port: 587,
            user: String::new(),
            password: String::new(),
            from_address: "noreply@example.com".to_string(),
            from_name: default_from_name(),
            enabled: false, // 默认关闭，需要在配置中启用
            reset_url_base: String::new(),
        }
    }
}

/// 邮件发送器 trait（用于测试时 mock）
pub trait EmailSender: Send + Sync {
    fn send_password_reset_email(&self, to: &str, reset_url: &str) -> Result<()>;
}

/// SMTP 邮件发送器
pub struct SmtpEmailSender {
    config: EmailConfig,
    mailer: Option<SmtpTransport>,
}

impl SmtpEmailSender {
    /// 创建新的 SMTP 邮件发送器
    pub fn new(config: EmailConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                mailer: None,
            });
        }

        let creds = Credentials::new(config.user.clone(), config.password.clone());

        let mailer = SmtpTransport::relay(&config.host)?
            .credentials(creds)
            .port(config.port)
            .build();

        Ok(Self {
            config,
            mailer: Some(mailer),
        })
    }

    /// 发送邮件
    fn send_email(&self, to: &str, subject: &str, html_body: &str, text_body: &str) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("[Email Mock] To: {}, Subject: {}", to, subject);
            return Ok(());
        }

        let mailer = self
            .mailer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Email sender not configured"))?;

        let from = format!("{} <{}>", self.config.from_name, self.config.from_address);

        let email = Message::builder()
            .from(from.parse()?)
            .to(to.parse()?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body.to_string()),
                    ),
            )?;

        mailer.send(&email)?;
        Ok(())
    }
}

impl EmailSender for SmtpEmailSender {
    /// 发送密码重置邮件
    fn send_password_reset_email(&self, to: &str, reset_url: &str) -> Result<()> {
        let subject = "【FoxNIO】密码重置请求";

        let html_body = format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .header {{ background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px; text-align: center; border-radius: 10px 10px 0 0; }}
        .content {{ background: #f9f9f9; padding: 30px; border-radius: 0 0 10px 10px; }}
        .button {{ display: inline-block; background: #667eea; color: white; padding: 15px 30px; text-decoration: none; border-radius: 5px; margin: 20px 0; }}
        .footer {{ text-align: center; color: #999; font-size: 12px; margin-top: 20px; }}
        .warning {{ background: #fff3cd; border: 1px solid #ffc107; padding: 15px; border-radius: 5px; margin: 20px 0; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🦊 FoxNIO</h1>
            <p>密码重置请求</p>
        </div>
        <div class="content">
            <p>您好，</p>
            <p>我们收到了您的密码重置请求。请点击下方按钮重置您的密码：</p>
            <p style="text-align: center;">
                <a href="{}" class="button">重置密码</a>
            </p>
            <p>或者复制以下链接到浏览器：</p>
            <p style="word-break: break-all; background: #eee; padding: 10px; border-radius: 5px; font-size: 14px;">{}</p>
            <div class="warning">
                <strong>⚠️ 重要提示：</strong>
                <ul style="margin: 10px 0; padding-left: 20px;">
                    <li>此链接将在 <strong>1 小时</strong> 后失效</li>
                    <li>每个链接只能使用一次</li>
                    <li>如果您没有请求重置密码，请忽略此邮件</li>
                </ul>
            </div>
        </div>
        <div class="footer">
            <p>此邮件由系统自动发送，请勿直接回复。</p>
            <p>© {} FoxNIO. All rights reserved.</p>
        </div>
    </div>
</body>
</html>
"#,
            reset_url,
            reset_url,
            chrono::Utc::now().year()
        );

        let text_body = format!(
            r#"
FoxNIO 密码重置请求

您好，

我们收到了您的密码重置请求。请访问以下链接重置您的密码：

{}

重要提示：
- 此链接将在 1 小时后失效
- 每个链接只能使用一次
- 如果您没有请求重置密码，请忽略此邮件

此邮件由系统自动发送，请勿直接回复。
© {} FoxNIO. All rights reserved.
"#,
            reset_url,
            chrono::Utc::now().year()
        );

        self.send_email(to, subject, &html_body, &text_body)
    }
}

/// Mock 邮件发送器（用于测试）
pub struct MockEmailSender {
    pub sent_emails: std::sync::Mutex<Vec<(String, String)>>,
}

impl MockEmailSender {
    pub fn new() -> Self {
        Self {
            sent_emails: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn get_sent_emails(&self) -> Vec<(String, String)> {
        self.sent_emails.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.sent_emails.lock().unwrap().clear();
    }
}

impl Default for MockEmailSender {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailSender for MockEmailSender {
    fn send_password_reset_email(&self, to: &str, reset_url: &str) -> Result<()> {
        tracing::info!("[Mock Email] To: {}, Reset URL: {}", to, reset_url);
        self.sent_emails
            .lock()
            .unwrap()
            .push((to.to_string(), reset_url.to_string()));
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_default() {
        let config = EmailConfig::default();
        assert_eq!(config.host, "smtp.example.com");
        assert_eq!(config.port, 587);
        assert!(!config.enabled);
    }

    #[test]
    fn test_mock_email_sender() {
        let sender = MockEmailSender::new();
        let result = sender.send_password_reset_email(
            "test@example.com",
            "https://example.com/reset?token=abc123",
        );
        assert!(result.is_ok());

        let emails = sender.get_sent_emails();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].0, "test@example.com");
        assert!(emails[0].1.contains("token=abc123"));
    }

    #[test]
    fn test_mock_email_sender_clear() {
        let sender = MockEmailSender::new();
        sender
            .send_password_reset_email("test@example.com", "https://example.com/reset")
            .unwrap();
        assert_eq!(sender.get_sent_emails().len(), 1);

        sender.clear();
        assert_eq!(sender.get_sent_emails().len(), 0);
    }

    #[test]
    fn test_smtp_email_sender_disabled() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };

        let sender = SmtpEmailSender::new(config).unwrap();
        let result =
            sender.send_password_reset_email("test@example.com", "https://example.com/reset");
        assert!(result.is_ok());
    }
}
