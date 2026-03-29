//! Antigravity 平台增强功能
//!
//! 提供 Tier 信息查询、配额管理、计费支持等高级功能

use crate::service::oauth::antigravity::AntigravityOAuthProvider;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Tier 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierInfo {
    /// Tier ID (free-tier, g1-pro-tier, g1-ultra-tier)
    pub id: String,
    /// 显示名称
    #[serde(default)]
    pub name: String,
    /// 描述
    #[serde(default)]
    pub description: String,
}

/// 可用额度信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableCredit {
    /// 额度类型
    #[serde(default)]
    pub credit_type: String,
    /// 额度数量
    #[serde(default)]
    pub credit_amount: String,
    /// 使用的最小额度
    #[serde(default)]
    pub minimum_credit_amount_for_usage: String,
}

impl AvailableCredit {
    /// 获取额度数量（浮点数）
    pub fn get_amount(&self) -> f64 {
        self.credit_amount.parse().unwrap_or(0.0)
    }

    /// 获取最小使用额度（浮点数）
    pub fn get_minimum_amount(&self) -> f64 {
        self.minimum_credit_amount_for_usage.parse().unwrap_or(0.0)
    }
}

/// 付费 Tier 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaidTierInfo {
    /// Tier ID
    pub id: String,
    /// 显示名称
    #[serde(default)]
    pub name: String,
    /// 描述
    #[serde(default)]
    pub description: String,
    /// 可用额度列表
    #[serde(default)]
    pub available_credits: Vec<AvailableCredit>,
}

/// 不符合条件的 Tier 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IneligibleTier {
    /// Tier 信息
    pub tier: Option<TierInfo>,
    /// 原因代码
    #[serde(default)]
    pub reason_code: String,
    /// 原因描述
    #[serde(default)]
    pub reason_message: String,
}

/// LoadCodeAssist 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCodeAssistResponse {
    /// Cloud AI Companion Project ID
    #[serde(rename = "cloudaicompanionProject")]
    pub cloud_ai_companion_project: String,
    /// 当前 Tier
    #[serde(rename = "currentTier")]
    pub current_tier: Option<TierInfo>,
    /// 付费 Tier
    #[serde(rename = "paidTier")]
    pub paid_tier: Option<PaidTierInfo>,
    /// 不符合条件的 Tier 列表
    #[serde(rename = "ineligibleTiers", default)]
    pub ineligible_tiers: Vec<IneligibleTier>,
    /// 允许的 Tier 列表
    #[serde(rename = "allowedTiers", default)]
    pub allowed_tiers: Vec<AllowedTier>,
}

/// 允许的 Tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedTier {
    /// Tier ID
    pub id: String,
    /// 是否为默认
    #[serde(default)]
    pub is_default: bool,
    /// Tier 信息
    #[serde(default)]
    pub tier: Option<TierInfo>,
}

/// LoadCodeAssist 请求
#[derive(Debug, Clone, Serialize)]
struct LoadCodeAssistRequest {
    metadata: LoadCodeAssistMetadata,
}

#[derive(Debug, Clone, Serialize)]
struct LoadCodeAssistMetadata {
    #[serde(rename = "ideType")]
    ide_type: String,
    #[serde(rename = "ideVersion")]
    ide_version: String,
    #[serde(rename = "ideName")]
    ide_name: String,
}

/// OnboardUser 请求
#[derive(Debug, Clone, Serialize)]
struct OnboardUserRequest {
    #[serde(rename = "tierId")]
    tier_id: String,
    metadata: OnboardUserMetadata,
}

#[derive(Debug, Clone, Serialize)]
struct OnboardUserMetadata {
    #[serde(rename = "ideType")]
    ide_type: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    plugin_type: String,
}

/// OnboardUser 响应
#[derive(Debug, Clone, Deserialize)]
pub struct OnboardUserResponse {
    #[serde(rename = "cloudaicompanionProject")]
    pub cloud_ai_companion_project: String,
}

/// 订阅信息（用于计费）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravitySubscription {
    /// 订阅类型
    pub plan_type: String,
    /// 是否为付费用户
    pub is_paid: bool,
    /// 可用额度
    pub credits: Option<f64>,
    /// 额度单位
    pub credit_unit: Option<String>,
}

/// Antigravity 增强功能实现
impl AntigravityOAuthProvider {
    /// 加载 CodeAssist 信息（获取 project_id 和 tier 信息）
    pub async fn load_code_assist(&mut self, access_token: &str) -> Result<LoadCodeAssistResponse> {
        let request = LoadCodeAssistRequest {
            metadata: LoadCodeAssistMetadata {
                ide_type: "VSCode".to_string(),
                ide_version: "1.85.0".to_string(),
                ide_name: "Visual Studio Code".to_string(),
            },
        };

        let response = self
            .call_api(
                access_token,
                "/v1internal:loadCodeAssist",
                Some(&serde_json::to_value(request)?),
            )
            .await?;

        let load_response: LoadCodeAssistResponse =
            serde_json::from_value(response).context("Failed to parse LoadCodeAssist response")?;

        Ok(load_response)
    }

    /// Onboard 用户（创建 project_id）
    pub async fn onboard_user(&mut self, access_token: &str, tier_id: &str) -> Result<String> {
        let request = OnboardUserRequest {
            tier_id: tier_id.to_string(),
            metadata: OnboardUserMetadata {
                ide_type: "VSCode".to_string(),
                platform: "windows".to_string(),
                plugin_type: "google-cloud-code".to_string(),
            },
        };

        let response = self
            .call_api(
                access_token,
                "/v1internal:onboardUser",
                Some(&serde_json::to_value(request)?),
            )
            .await?;

        let onboard_response: OnboardUserResponse =
            serde_json::from_value(response).context("Failed to parse OnboardUser response")?;

        Ok(onboard_response.cloud_ai_companion_project)
    }

    /// 获取默认 Tier ID
    pub fn get_default_tier_id(load_response: &LoadCodeAssistResponse) -> Option<String> {
        for tier in &load_response.allowed_tiers {
            if tier.is_default {
                return Some(tier.id.clone());
            }
        }
        None
    }

    /// 获取订阅信息（用于计费）
    pub async fn get_subscription_info(
        &mut self,
        access_token: &str,
    ) -> Result<AntigravitySubscription> {
        let load_response = self.load_code_assist(access_token).await?;

        let plan_type = load_response
            .current_tier
            .as_ref()
            .map(|t| t.id.clone())
            .unwrap_or_else(|| "free-tier".to_string());

        let is_paid = load_response.paid_tier.is_some();

        let credits = load_response
            .paid_tier
            .as_ref()
            .and_then(|pt| pt.available_credits.first())
            .map(|c: &AvailableCredit| c.get_amount());

        let credit_unit = load_response
            .paid_tier
            .as_ref()
            .and_then(|pt| pt.available_credits.first())
            .map(|_| "credits".to_string());

        Ok(AntigravitySubscription {
            plan_type,
            is_paid,
            credits,
            credit_unit,
        })
    }

    /// 检查账户是否可用
    pub async fn check_account_status(&mut self, access_token: &str) -> Result<AccountStatus> {
        let load_response = self.load_code_assist(access_token).await?;

        let has_project = !load_response.cloud_ai_companion_project.is_empty();
        let has_tier = load_response.current_tier.is_some();
        let is_paid = load_response.paid_tier.is_some();

        let available_credits = load_response
            .paid_tier
            .as_ref()
            .and_then(|pt| pt.available_credits.first())
            .map(|c: &AvailableCredit| c.get_amount());

        Ok(AccountStatus {
            has_project,
            has_tier,
            is_paid,
            available_credits,
            project_id: load_response.cloud_ai_companion_project,
        })
    }
}

/// 账户状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStatus {
    /// 是否有 project_id
    pub has_project: bool,
    /// 是否有 tier
    pub has_tier: bool,
    /// 是否为付费用户
    pub is_paid: bool,
    /// 可用额度
    pub available_credits: Option<f64>,
    /// Project ID
    pub project_id: String,
}

/// 配额查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaInfo {
    /// 总配额
    pub total: u64,
    /// 已使用
    pub used: u64,
    /// 剩余
    pub remaining: u64,
    /// 重置时间
    pub reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Antigravity 配额管理
impl AntigravityOAuthProvider {
    /// 查询配额使用情况
    ///
    /// 注意：Antigravity API 没有直接的配额查询接口，
    /// 这里通过 LoadCodeAssist 获取可用额度来推断
    pub async fn query_quota(&mut self, access_token: &str) -> Result<QuotaInfo> {
        let subscription = self.get_subscription_info(access_token).await?;

        // 如果是付费用户，返回可用额度
        if let Some(credits) = subscription.credits {
            return Ok(QuotaInfo {
                total: (credits * 100.0) as u64, // 假设单位转换
                used: 0,                         // API 不提供已使用量
                remaining: (credits * 100.0) as u64,
                reset_at: None,
            });
        }

        // 免费用户返回默认值
        Ok(QuotaInfo {
            total: 1000,
            used: 0,
            remaining: 1000,
            reset_at: None,
        })
    }

    /// 检查是否有足够配额
    pub async fn check_quota(&mut self, access_token: &str, required: u64) -> Result<bool> {
        let quota = self.query_quota(access_token).await?;
        Ok(quota.remaining >= required)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_credit_amount() {
        let credit = AvailableCredit {
            credit_type: "ai_credits".to_string(),
            credit_amount: "100.5".to_string(),
            minimum_credit_amount_for_usage: "1.0".to_string(),
        };

        assert_eq!(credit.get_amount(), 100.5);
        assert_eq!(credit.get_minimum_amount(), 1.0);
    }

    #[test]
    fn test_available_credit_empty() {
        let credit = AvailableCredit {
            credit_type: "ai_credits".to_string(),
            credit_amount: String::new(),
            minimum_credit_amount_for_usage: String::new(),
        };

        assert_eq!(credit.get_amount(), 0.0);
        assert_eq!(credit.get_minimum_amount(), 0.0);
    }

    #[test]
    fn test_account_status() {
        let status = AccountStatus {
            has_project: true,
            has_tier: true,
            is_paid: false,
            available_credits: Some(100.0),
            project_id: "test-project".to_string(),
        };

        assert!(status.has_project);
        assert!(status.has_tier);
        assert!(!status.is_paid);
    }

    #[test]
    fn test_quota_info() {
        let quota = QuotaInfo {
            total: 1000,
            used: 300,
            remaining: 700,
            reset_at: None,
        };

        assert_eq!(quota.total, 1000);
        assert_eq!(quota.used, 300);
        assert_eq!(quota.remaining, 700);
    }
}
