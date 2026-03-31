//! OpenAI 隐私模式服务
//!
//! 自动关闭 OpenAI 账号的"改进模型"选项，保护用户隐私

#![allow(dead_code)]
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 隐私模式状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PrivacyMode {
    /// 成功关闭训练
    TrainingOff,
    /// 设置失败
    TrainingSetFailed,
    /// 被 Cloudflare 拦截
    TrainingSetCfBlocked,
    /// 未知状态
    #[default]
    Unknown,
}

impl std::fmt::Display for PrivacyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrainingOff => write!(f, "training_off"),
            Self::TrainingSetFailed => write!(f, "training_set_failed"),
            Self::TrainingSetCfBlocked => write!(f, "training_set_cf_blocked"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<&str> for PrivacyMode {
    fn from(s: &str) -> Self {
        match s {
            "training_off" => Self::TrainingOff,
            "training_set_failed" => Self::TrainingSetFailed,
            "training_set_cf_blocked" => Self::TrainingSetCfBlocked,
            _ => Self::Unknown,
        }
    }
}

/// ChatGPT 账号信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatGPTAccountInfo {
    pub plan_type: Option<String>,
    pub email: Option<String>,
}

/// OpenAI 隐私服务
pub struct OpenAIPrivacyService {
    http_client: Client,
}

impl OpenAIPrivacyService {
    /// 创建新的隐私服务
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client: client,
        }
    }

    /// 禁用 OpenAI 训练（关闭"改进模型"选项）
    ///
    /// # Arguments
    /// * `access_token` - OpenAI 访问令牌
    /// * `proxy_url` - 代理 URL（可选）
    ///
    /// # Returns
    /// 返回隐私模式状态
    pub async fn disable_training(
        &self,
        access_token: &str,
        proxy_url: Option<&str>,
    ) -> Result<PrivacyMode> {
        if access_token.is_empty() {
            return Ok(PrivacyMode::Unknown);
        }

        let url = "https://chatgpt.com/backend-api/settings/account_user_setting";

        let request = self
            .http_client
            .patch(url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Origin", "https://chatgpt.com")
            .header("Referer", "https://chatgpt.com/")
            .query(&[("feature", "training_allowed"), ("value", "false")]);

        // 如果有代理，设置代理
        if let Some(proxy) = proxy_url {
            // 注意：reqwest 的代理需要单独配置
            // 这里暂时忽略代理设置，实际使用时需要重构 HTTP client
            tracing::debug!("Proxy URL provided but not yet implemented: {}", proxy);
        }

        let response = request
            .send()
            .await
            .context("Failed to send privacy request")?;

        let status = response.status();

        // 检查是否被 Cloudflare 拦截
        if status.as_u16() == 403 || status.as_u16() == 503 {
            let body = response.text().await.unwrap_or_default();
            if body.contains("cloudflare") || body.contains("cf-") || body.contains("Just a moment")
            {
                tracing::warn!("OpenAI privacy request blocked by Cloudflare");
                return Ok(PrivacyMode::TrainingSetCfBlocked);
            }
            // 如果不是 Cloudflare 拦截，继续检查是否成功
            tracing::warn!(
                "OpenAI privacy request failed: status={}, body={}",
                status,
                truncate(&body, 200)
            );
            return Ok(PrivacyMode::TrainingSetFailed);
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(
                "OpenAI privacy request failed: status={}, body={}",
                status,
                truncate(&body, 200)
            );
            return Ok(PrivacyMode::TrainingSetFailed);
        }

        tracing::info!("OpenAI training successfully disabled");
        Ok(PrivacyMode::TrainingOff)
    }

    /// 获取 ChatGPT 账号信息
    ///
    /// # Arguments
    /// * `access_token` - OpenAI 访问令牌
    /// * `proxy_url` - 代理 URL（可选）
    /// * `org_id` - 组织 ID（可选，用于匹配正确的账号）
    ///
    /// # Returns
    /// 返回账号信息（最佳努力，失败返回 None）
    pub async fn fetch_account_info(
        &self,
        access_token: &str,
        proxy_url: Option<&str>,
        org_id: Option<&str>,
    ) -> Option<ChatGPTAccountInfo> {
        if access_token.is_empty() {
            return None;
        }

        let url = "https://chatgpt.com/backend-api/accounts/check/v4-2023-04-27";

        let request = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Origin", "https://chatgpt.com")
            .header("Referer", "https://chatgpt.com/")
            .header("Accept", "application/json");

        if let Some(proxy) = proxy_url {
            tracing::debug!("Proxy URL provided but not yet implemented: {}", proxy);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Failed to fetch ChatGPT account info: {}", e);
                return None;
            }
        };

        if !response.status().is_success() {
            tracing::debug!("ChatGPT account check failed: status={}", response.status());
            return None;
        }

        let result: serde_json::Value = match response.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!("Failed to parse ChatGPT account response: {}", e);
                return None;
            }
        };

        // 解析账号信息
        let accounts = match result.get("accounts").and_then(|a| a.as_object()) {
            Some(a) => a,
            None => {
                tracing::debug!("No accounts found in response");
                return None;
            }
        };

        let mut info = ChatGPTAccountInfo {
            plan_type: None,
            email: None,
        };

        // 优先匹配 org_id
        if let Some(org) = org_id {
            if let Some(acct) = accounts.get(org) {
                info.plan_type = extract_plan_type(acct);
            }
        }

        // 如果未匹配到，遍历所有账号
        if info.plan_type.is_none() {
            let mut default_plan = None;
            let mut paid_plan = None;
            let mut any_plan = None;

            for acct in accounts.values() {
                if let Some(plan) = extract_plan_type(acct) {
                    any_plan = Some(plan.clone());

                    // 检查是否是默认账号
                    if let Some(account) = acct.get("account").and_then(|a| a.as_object()) {
                        if account
                            .get("is_default")
                            .and_then(|d| d.as_bool())
                            .unwrap_or(false)
                        {
                            default_plan = Some(plan.clone());
                        }
                    }

                    // 检查是否是付费账号
                    if !plan.eq_ignore_ascii_case("free") && paid_plan.is_none() {
                        paid_plan = Some(plan);
                    }
                }
            }

            // 优先级：default > 非 free > 任意
            info.plan_type = default_plan.or(paid_plan).or(any_plan);
        }

        if info.plan_type.is_some() {
            tracing::info!(
                "ChatGPT account info retrieved: plan_type={:?}",
                info.plan_type
            );
        }

        Some(info)
    }

    /// 检查是否应该跳过隐私设置
    pub fn should_skip_privacy_ensure(extra: &serde_json::Value) -> bool {
        if let Some(mode) = extra.get("privacy_mode").and_then(|m| m.as_str()) {
            let mode = mode.trim();
            // 如果状态不是失败或被拦截，跳过
            mode != "training_set_failed" && mode != "training_set_cf_blocked"
        } else {
            false
        }
    }
}

impl Default for OpenAIPrivacyService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 从账号对象中提取 plan_type
fn extract_plan_type(acct: &serde_json::Value) -> Option<String> {
    // 尝试从 account.plan_type 提取
    if let Some(plan) = acct
        .get("account")
        .and_then(|a| a.get("plan_type"))
        .and_then(|p| p.as_str())
    {
        return Some(plan.to_string());
    }

    // 尝试从 entitlement.subscription_plan 提取
    if let Some(plan) = acct
        .get("entitlement")
        .and_then(|e| e.get("subscription_plan"))
        .and_then(|p| p.as_str())
    {
        return Some(plan.to_string());
    }

    None
}

/// 截断字符串
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...({} more)", &s[..max_len], s.len() - max_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_mode_display() {
        assert_eq!(PrivacyMode::TrainingOff.to_string(), "training_off");
        assert_eq!(
            PrivacyMode::TrainingSetFailed.to_string(),
            "training_set_failed"
        );
        assert_eq!(
            PrivacyMode::TrainingSetCfBlocked.to_string(),
            "training_set_cf_blocked"
        );
    }

    #[test]
    fn test_privacy_mode_from_str() {
        assert_eq!(PrivacyMode::from("training_off"), PrivacyMode::TrainingOff);
        assert_eq!(
            PrivacyMode::from("training_set_failed"),
            PrivacyMode::TrainingSetFailed
        );
        assert_eq!(PrivacyMode::from("unknown"), PrivacyMode::Unknown);
    }

    #[test]
    fn test_should_skip_privacy_ensure() {
        let extra = serde_json::json!({"privacy_mode": "training_off"});
        assert!(OpenAIPrivacyService::should_skip_privacy_ensure(&extra));

        let extra = serde_json::json!({"privacy_mode": "training_set_failed"});
        assert!(!OpenAIPrivacyService::should_skip_privacy_ensure(&extra));

        let extra = serde_json::json!({});
        assert!(!OpenAIPrivacyService::should_skip_privacy_ensure(&extra));
    }

    #[test]
    fn test_privacy_mode_serialization() {
        let mode = PrivacyMode::TrainingOff;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"training_off\"");

        let deserialized: PrivacyMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PrivacyMode::TrainingOff);
    }

    #[test]
    fn test_chatgpt_account_info_default() {
        let info = ChatGPTAccountInfo::default();
        assert!(info.plan_type.is_none());
        assert!(info.email.is_none());
    }

    #[test]
    fn test_service_creation() {
        let _service = OpenAIPrivacyService::new();
        // Service created successfully
    }

    #[test]
    fn test_service_default() {
        let _service = OpenAIPrivacyService::default();
        // Service created successfully
    }

    #[test]
    fn test_truncate_short_string() {
        let s = "short";
        let truncated = truncate(s, 10);
        assert_eq!(truncated, "short");
    }

    #[test]
    fn test_truncate_long_string() {
        let s = "this is a very long string that should be truncated";
        let truncated = truncate(s, 10);
        assert!(truncated.starts_with("this is a "));
        assert!(truncated.contains("more"));
    }

    #[test]
    fn test_privacy_mode_equality() {
        assert_eq!(PrivacyMode::TrainingOff, PrivacyMode::TrainingOff);
        assert_ne!(PrivacyMode::TrainingOff, PrivacyMode::TrainingSetFailed);
        assert_ne!(PrivacyMode::TrainingOff, PrivacyMode::TrainingSetCfBlocked);
    }

    #[test]
    fn test_should_skip_with_empty_string() {
        let extra = serde_json::json!({"privacy_mode": ""});
        // Empty string is not "training_set_failed" or "training_set_cf_blocked", so skip
        assert!(OpenAIPrivacyService::should_skip_privacy_ensure(&extra));
    }

    #[test]
    fn test_should_skip_with_whitespace() {
        let extra = serde_json::json!({"privacy_mode": "  training_off  "});
        assert!(OpenAIPrivacyService::should_skip_privacy_ensure(&extra));
    }

    #[test]
    fn test_privacy_mode_from_invalid() {
        assert_eq!(PrivacyMode::from("invalid"), PrivacyMode::Unknown);
        assert_eq!(PrivacyMode::from(""), PrivacyMode::Unknown);
        assert_eq!(PrivacyMode::from("TRAINING_OFF"), PrivacyMode::Unknown);
    }
}
