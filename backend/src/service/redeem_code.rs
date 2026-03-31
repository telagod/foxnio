//! 卡密兑换服务
//!
//! 提供卡密生成、兑换和管理功能

#![allow(dead_code)]

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 卡密类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedeemType {
    Balance,      // 余额
    Subscription, // 订阅
    Quota,        // 配额
}

impl std::fmt::Display for RedeemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedeemType::Balance => write!(f, "balance"),
            RedeemType::Subscription => write!(f, "subscription"),
            RedeemType::Quota => write!(f, "quota"),
        }
    }
}

impl std::str::FromStr for RedeemType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "balance" => Ok(RedeemType::Balance),
            "subscription" => Ok(RedeemType::Subscription),
            "quota" => Ok(RedeemType::Quota),
            _ => bail!("Invalid redeem type: {}", s),
        }
    }
}

/// 卡密状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedeemStatus {
    Unused,
    Used,
    Expired,
    Cancelled,
}

impl std::fmt::Display for RedeemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedeemStatus::Unused => write!(f, "unused"),
            RedeemStatus::Used => write!(f, "used"),
            RedeemStatus::Expired => write!(f, "expired"),
            RedeemStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 卡密配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemCode {
    pub id: Uuid,
    pub code: String,
    pub code_type: RedeemType,
    pub value: f64,
    pub status: RedeemStatus,
    pub used_by: Option<Uuid>,
    pub used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub group_id: Option<i64>,
    pub validity_days: i32,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 批量生成卡密请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCodesRequest {
    pub code_type: String,
    pub value: f64,
    pub count: i32,
    pub validity_days: i32,
    pub group_id: Option<i64>,
    pub notes: Option<String>,
}

/// 兑换卡密请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemCodeRequest {
    pub code: String,
    pub user_id: Uuid,
}

/// 兑换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemResult {
    pub success: bool,
    pub code_type: RedeemType,
    pub value: f64,
    pub message: String,
    pub redeemed_at: DateTime<Utc>,
}

/// 卡密统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemStats {
    pub total_codes: i64,
    pub unused_codes: i64,
    pub used_codes: i64,
    pub expired_codes: i64,
    pub total_value: f64,
    pub used_value: f64,
}

/// 卡密服务
pub struct RedeemCodeService {
    db: DatabaseConnection,
    code_length: usize,
}

impl RedeemCodeService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            code_length: 16,
        }
    }

    /// 生成随机卡密
    fn generate_code(&self) -> String {
        let mut rng = rand::thread_rng();
        let chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // 去除易混淆字符

        (0..self.code_length)
            .map(|_| {
                let idx = rng.gen_range(0..chars.len());
                chars.chars().nth(idx).unwrap()
            })
            .collect()
    }

    /// 批量生成卡密
    pub async fn generate_batch(&self, req: GenerateCodesRequest) -> Result<Vec<RedeemCode>> {
        let code_type = req.code_type.parse::<RedeemType>()?;
        let now = Utc::now();
        let expires_at = now + Duration::days(req.validity_days as i64);

        let mut codes = Vec::new();

        for _ in 0..req.count {
            let code = RedeemCode {
                id: Uuid::new_v4(),
                code: self.generate_code(),
                code_type: code_type.clone(),
                value: req.value,
                status: RedeemStatus::Unused,
                used_by: None,
                used_at: None,
                expires_at: Some(expires_at),
                group_id: req.group_id,
                validity_days: req.validity_days,
                notes: req.notes.clone(),
                created_at: now,
            };
            codes.push(code);
        }

        // TODO: 批量插入数据库

        Ok(codes)
    }

    /// 兑换卡密
    pub async fn redeem(&self, req: RedeemCodeRequest) -> Result<RedeemResult> {
        // 查找卡密
        let code = self
            .find_by_code(&req.code)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid redeem code"))?;

        // 检查状态
        if code.status != RedeemStatus::Unused {
            return Ok(RedeemResult {
                success: false,
                code_type: code.code_type,
                value: code.value,
                message: format!("Code already {}", code.status),
                redeemed_at: Utc::now(),
            });
        }

        // 检查过期
        if let Some(expires_at) = code.expires_at {
            if Utc::now() > expires_at {
                return Ok(RedeemResult {
                    success: false,
                    code_type: code.code_type,
                    value: code.value,
                    message: "Code has expired".to_string(),
                    redeemed_at: Utc::now(),
                });
            }
        }

        // 根据类型执行兑换
        let message = match code.code_type {
            RedeemType::Balance => self.redeem_balance(&req.user_id, code.value).await?,
            RedeemType::Subscription => {
                self.redeem_subscription(&req.user_id, code.group_id, code.validity_days)
                    .await?
            }
            RedeemType::Quota => self.redeem_quota(&req.user_id, code.value as i64).await?,
        };

        // 标记为已使用
        self.mark_as_used(code.id, req.user_id).await?;

        Ok(RedeemResult {
            success: true,
            code_type: code.code_type,
            value: code.value,
            message,
            redeemed_at: Utc::now(),
        })
    }

    /// 查找卡密
    async fn find_by_code(&self, _code: &str) -> Result<Option<RedeemCode>> {
        // TODO: 从数据库查询
        Ok(None)
    }

    /// 标记卡密为已使用
    async fn mark_as_used(&self, _code_id: Uuid, _user_id: Uuid) -> Result<()> {
        // TODO: 更新数据库
        Ok(())
    }

    /// 兑换余额
    async fn redeem_balance(&self, _user_id: &Uuid, amount: f64) -> Result<String> {
        // TODO: 增加用户余额
        Ok(format!("Added ${:.2} to your balance", amount))
    }

    /// 兑换订阅
    async fn redeem_subscription(
        &self,
        _user_id: &Uuid,
        _group_id: Option<i64>,
        days: i32,
    ) -> Result<String> {
        // TODO: 创建订阅
        Ok(format!("Added {days} days subscription"))
    }

    /// 兑换配额
    async fn redeem_quota(&self, _user_id: &Uuid, quota: i64) -> Result<String> {
        // TODO: 增加用户配额
        Ok(format!("Added {quota} tokens quota"))
    }

    /// 获取用户兑换历史
    pub async fn get_user_redemptions(&self, _user_id: Uuid) -> Result<Vec<RedeemCode>> {
        // TODO: 从数据库查询
        Ok(vec![])
    }

    /// 获取卡密统计
    pub async fn get_stats(&self) -> Result<RedeemStats> {
        // TODO: 从数据库统计
        Ok(RedeemStats {
            total_codes: 0,
            unused_codes: 0,
            used_codes: 0,
            expired_codes: 0,
            total_value: 0.0,
            used_value: 0.0,
        })
    }

    /// 取消卡密
    pub async fn cancel(&self, _code_id: Uuid) -> Result<()> {
        // TODO: 更新状态为已取消
        Ok(())
    }

    /// 清理过期卡密
    pub async fn cleanup_expired(&self) -> Result<i64> {
        // TODO: 更新过期卡密状态
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redeem_type_parse() {
        assert_eq!(
            "balance".parse::<RedeemType>().unwrap(),
            RedeemType::Balance
        );
        assert_eq!(
            "subscription".parse::<RedeemType>().unwrap(),
            RedeemType::Subscription
        );
        assert_eq!("quota".parse::<RedeemType>().unwrap(), RedeemType::Quota);
    }
}
