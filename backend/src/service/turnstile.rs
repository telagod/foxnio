//! Cloudflare Turnstile 人机验证服务
//!
//! 实现 Cloudflare Turnstile 验证码验证，用于注册、登录等场景的安全防护

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Turnstile 验证服务配置
#[derive(Debug, Clone)]
pub struct TurnstileConfig {
    /// Site Key (Public Key)
    pub site_key: String,
    /// Secret Key (Private Key)
    pub secret_key: String,
    /// 是否启用验证
    pub enabled: bool,
    /// 验证 API URL
    pub verify_url: String,
    /// 超时时间（秒）
    pub timeout_secs: u64,
}

impl TurnstileConfig {
    /// 创建新的配置
    pub fn new(site_key: String, secret_key: String) -> Self {
        Self {
            site_key,
            secret_key,
            enabled: true,
            verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string(),
            timeout_secs: 10,
        }
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let site_key = std::env::var("TURNSTILE_SITE_KEY")
            .context("TURNSTILE_SITE_KEY environment variable not set")?;
        let secret_key = std::env::var("TURNSTILE_SECRET_KEY")
            .context("TURNSTILE_SECRET_KEY environment variable not set")?;

        let enabled = std::env::var("TURNSTILE_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);

        Ok(Self {
            site_key,
            secret_key,
            enabled,
            verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string(),
            timeout_secs: 10,
        })
    }

    /// 设置是否启用
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for TurnstileConfig {
    fn default() -> Self {
        Self {
            site_key: String::new(),
            secret_key: String::new(),
            enabled: false,
            verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string(),
            timeout_secs: 10,
        }
    }
}

/// Turnstile 验证请求
#[derive(Debug, Clone, Serialize)]
struct VerifyRequest {
    secret: String,
    response: String,
    remoteip: Option<String>,
}

/// Turnstile 验证响应
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyResponse {
    /// 验证是否成功
    pub success: bool,
    /// 失败时的错误代码
    #[serde(rename = "error-codes", default)]
    pub error_codes: Vec<String>,
    /// 挑战时间戳
    #[serde(rename = "challenge_ts", default)]
    pub challenge_ts: Option<String>,
    /// 主机名
    #[serde(default)]
    pub hostname: Option<String>,
    /// 动作
    #[serde(default)]
    pub action: Option<String>,
    /// 客户端数据
    #[serde(default)]
    pub cdata: Option<String>,
}

/// Turnstile 验证服务
pub struct TurnstileService {
    config: TurnstileConfig,
    http_client: Client,
}

impl TurnstileService {
    /// 创建新的验证服务
    pub fn new(config: TurnstileConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// 从环境变量创建服务
    pub fn from_env() -> Result<Self> {
        let config = TurnstileConfig::from_env()?;
        Self::new(config)
    }

    /// 验证 Turnstile Token
    ///
    /// # 参数
    /// - `token`: 前端传递的 Turnstile token
    /// - `remote_ip`: 可选的用户 IP 地址
    /// - `action`: 可选的动作标识（如 "login", "register"）
    ///
    /// # 返回
    /// 验证是否成功
    pub async fn verify(
        &self,
        token: &str,
        remote_ip: Option<&str>,
        action: Option<&str>,
    ) -> Result<bool> {
        // 如果未启用，直接返回成功
        if !self.config.enabled {
            return Ok(true);
        }

        // 验证 token 不为空
        if token.trim().is_empty() {
            bail!("Turnstile token is empty");
        }

        // 构建验证请求
        let request = VerifyRequest {
            secret: self.config.secret_key.clone(),
            response: token.to_string(),
            remoteip: remote_ip.map(|s| s.to_string()),
        };

        // 发送验证请求
        let response = self
            .http_client
            .post(&self.config.verify_url)
            .form(&request)
            .send()
            .await
            .context("Failed to send Turnstile verification request")?;

        // 检查 HTTP 状态
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Turnstile verification failed: HTTP {} - {}", status, body);
        }

        // 解析响应
        let verify_response: VerifyResponse = response
            .json()
            .await
            .context("Failed to parse Turnstile verification response")?;

        // 验证 action（如果配置了）
        if let (Some(expected_action), Some(received_action)) =
            (action, verify_response.action.as_deref())
        {
            if expected_action != received_action {
                bail!(
                    "Turnstile action mismatch: expected {}, got {}",
                    expected_action,
                    received_action
                );
            }
        }

        if !verify_response.success {
            let errors = verify_response.error_codes.join(", ");
            bail!("Turnstile verification failed: {}", errors);
        }

        Ok(true)
    }

    /// 获取 Site Key（用于前端渲染）
    pub fn site_key(&self) -> &str {
        &self.config.site_key
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 验证并返回详细信息
    pub async fn verify_with_details(
        &self,
        token: &str,
        remote_ip: Option<&str>,
        action: Option<&str>,
    ) -> Result<VerifyResponse> {
        // 如果未启用，返回成功的模拟响应
        if !self.config.enabled {
            return Ok(VerifyResponse {
                success: true,
                error_codes: vec![],
                challenge_ts: None,
                hostname: None,
                action: action.map(|s| s.to_string()),
                cdata: None,
            });
        }

        // 验证 token 不为空
        if token.trim().is_empty() {
            bail!("Turnstile token is empty");
        }

        // 构建验证请求
        let request = VerifyRequest {
            secret: self.config.secret_key.clone(),
            response: token.to_string(),
            remoteip: remote_ip.map(|s| s.to_string()),
        };

        // 发送验证请求
        let response = self
            .http_client
            .post(&self.config.verify_url)
            .form(&request)
            .send()
            .await
            .context("Failed to send Turnstile verification request")?;

        // 检查 HTTP 状态
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Turnstile verification failed: HTTP {} - {}", status, body);
        }

        // 解析响应
        let verify_response: VerifyResponse = response
            .json()
            .await
            .context("Failed to parse Turnstile verification response")?;

        // 验证 action（如果配置了）
        if let (Some(expected_action), Some(received_action)) =
            (action, verify_response.action.as_deref())
        {
            if expected_action != received_action {
                bail!(
                    "Turnstile action mismatch: expected {}, got {}",
                    expected_action,
                    received_action
                );
            }
        }

        Ok(verify_response)
    }
}

/// Turnstile 验证中间件参数
#[derive(Debug, Clone, Deserialize)]
pub struct TurnstileVerifyParams {
    /// Turnstile token
    #[serde(rename = "cf-turnstile-response")]
    pub token: String,
    /// 可选的动作标识
    pub action: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config =
            TurnstileConfig::new("test_site_key".to_string(), "test_secret_key".to_string());

        assert_eq!(config.site_key, "test_site_key");
        assert_eq!(config.secret_key, "test_secret_key");
        assert!(config.enabled);
    }

    #[test]
    fn test_config_default() {
        let config = TurnstileConfig::default();

        assert!(config.site_key.is_empty());
        assert!(config.secret_key.is_empty());
        assert!(!config.enabled);
    }

    #[test]
    fn test_service_creation_disabled() {
        let config = TurnstileConfig::default();
        let service = TurnstileService::new(config).unwrap();

        assert!(!service.is_enabled());
    }

    #[tokio::test]
    async fn test_verify_when_disabled() {
        let config = TurnstileConfig::default();
        let service = TurnstileService::new(config).unwrap();

        // 当服务未启用时，验证应该直接返回成功
        let result = service.verify("test_token", None, None).await;
        assert!(result.unwrap());
    }

    #[test]
    fn test_site_key() {
        let config = TurnstileConfig::new("my_site_key".to_string(), "my_secret_key".to_string());
        let service = TurnstileService::new(config).unwrap();

        assert_eq!(service.site_key(), "my_site_key");
    }
}
