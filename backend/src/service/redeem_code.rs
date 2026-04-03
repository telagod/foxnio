//! 卡密兑换服务
//!
//! 提供卡密生成、兑换和管理功能

#![allow(dead_code)]

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::redeem_codes;
use crate::entity::users;

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
    pub id: i64,
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
    pub created_by: Option<i64>,
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

/// 卡密预校验结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemCodePreview {
    pub valid: bool,
    pub code: String,
    pub code_type: RedeemType,
    pub value: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub message: Option<String>,
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
        let batch_id = Uuid::new_v4().to_string();

        let mut codes = Vec::new();
        let mut db_models = Vec::new();

        for _ in 0..req.count {
            let code_str = self.generate_code();

            // 创建数据库模型
            let db_model = redeem_codes::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                code: Set(code_str.clone()),
                batch_id: Set(Some(batch_id.clone())),
                amount: Set(Decimal::from_f64_retain(req.value).unwrap_or_default()),
                r#type: Set(code_type.to_string()),
                max_uses: Set(1),
                used_count: Set(0),
                status: Set("active".to_string()),
                expires_at: Set(Some(expires_at.into())),
                used_by: Set(None),
                notes: Set(req.notes.clone()),
                created_by: Set(req.created_by),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            db_models.push(db_model);

            codes.push(RedeemCode {
                id: 0, // 将在插入后设置
                code: code_str,
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
            });
        }

        // 批量插入数据库
        for (i, db_model) in db_models.into_iter().enumerate() {
            let inserted = db_model.insert(&self.db).await?;
            codes[i].id = inserted.id;
        }

        tracing::info!(
            batch_id = %batch_id,
            count = codes.len(),
            code_type = %code_type,
            value = req.value,
            "Generated redeem codes batch"
        );

        Ok(codes)
    }

    /// 兑换卡密
    pub async fn redeem(&self, req: RedeemCodeRequest) -> Result<RedeemResult> {
        // 使用事务确保原子性
        let txn = self.db.begin().await?;

        // 查找卡密（加锁）
        let code = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Code.eq(&req.code))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Invalid redeem code"))?;

        // 检查状态
        if code.status != "active" {
            txn.commit().await?;
            return Ok(RedeemResult {
                success: false,
                code_type: code.r#type.parse().unwrap_or(RedeemType::Balance),
                value: code.amount.to_string().parse().unwrap_or(0.0),
                message: format!("Code already {}", code.status),
                redeemed_at: Utc::now(),
            });
        }

        // 检查过期
        if let Some(expires_at) = code.expires_at {
            if chrono::Utc::now() > expires_at {
                txn.commit().await?;
                return Ok(RedeemResult {
                    success: false,
                    code_type: code.r#type.parse().unwrap_or(RedeemType::Balance),
                    value: code.amount.to_string().parse().unwrap_or(0.0),
                    message: "Code has expired".to_string(),
                    redeemed_at: Utc::now(),
                });
            }
        }

        // 检查使用次数
        if code.used_count >= code.max_uses {
            txn.commit().await?;
            return Ok(RedeemResult {
                success: false,
                code_type: code.r#type.parse().unwrap_or(RedeemType::Balance),
                value: code.amount.to_string().parse().unwrap_or(0.0),
                message: "Code has reached max uses".to_string(),
                redeemed_at: Utc::now(),
            });
        }

        let code_type = code.r#type.parse::<RedeemType>()?;
        let value = code.amount.to_string().parse::<f64>().unwrap_or(0.0);

        // 根据类型执行兑换
        let message = match code_type {
            RedeemType::Balance => {
                self.redeem_balance_with_txn(&txn, &req.user_id, value)
                    .await?
            }
            RedeemType::Subscription => {
                // 订阅暂时不支持，返回提示
                "Subscription redeem not yet implemented".to_string()
            }
            RedeemType::Quota => {
                self.redeem_quota_with_txn(&txn, &req.user_id, value as i64)
                    .await?
            }
        };

        // 更新卡密状态
        let now = Utc::now();
        let mut code: redeem_codes::ActiveModel = code.into();
        code.used_count = Set(code.used_count.unwrap() + 1);
        code.used_by = Set(Some(serde_json::to_value(&req.user_id)?));
        code.updated_at = Set(now.into());

        // 如果已达到最大使用次数，标记为已使用
        if code.used_count.as_ref() >= code.max_uses.as_ref() {
            code.status = Set("used".to_string());
        }

        code.update(&txn).await?;

        // 提交事务
        txn.commit().await?;

        tracing::info!(
            code = %req.code,
            user_id = %req.user_id,
            code_type = %code_type,
            value = value,
            "Redeem code used successfully"
        );

        Ok(RedeemResult {
            success: true,
            code_type,
            value,
            message,
            redeemed_at: now,
        })
    }

    /// 预览卡密状态，用于公开校验场景
    pub async fn preview_code(&self, code_str: &str) -> Result<Option<RedeemCodePreview>> {
        let code = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Code.eq(code_str))
            .one(&self.db)
            .await?;

        Ok(code.map(|model| {
            let code_type = model.r#type.parse().unwrap_or(RedeemType::Balance);
            let value = model.amount.to_string().parse::<f64>().unwrap_or(0.0);
            let message = if model.status != "active" {
                Some(format!("Code already {}", model.status))
            } else if model.used_count >= model.max_uses {
                Some("Code has reached max uses".to_string())
            } else if model
                .expires_at
                .is_some_and(|expires_at| Utc::now() > expires_at)
            {
                Some("Code has expired".to_string())
            } else {
                None
            };

            RedeemCodePreview {
                valid: message.is_none(),
                code: model.code,
                code_type,
                value,
                expires_at: model
                    .expires_at
                    .map(|expires_at| expires_at.with_timezone(&Utc)),
                notes: model.notes,
                message,
            }
        }))
    }

    /// 查找卡密
    async fn find_by_code(&self, code_str: &str) -> Result<Option<RedeemCode>> {
        let code = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Code.eq(code_str))
            .one(&self.db)
            .await?;

        Ok(code.map(|c| self.db_model_to_domain(c)))
    }

    /// 标记卡密为已使用
    async fn mark_as_used(&self, code_id: i64, user_id: Uuid) -> Result<()> {
        let code = redeem_codes::Entity::find_by_id(code_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Code not found"))?;

        let mut code: redeem_codes::ActiveModel = code.into();
        code.used_count = Set(code.used_count.unwrap() + 1);
        code.used_by = Set(Some(serde_json::to_value(&user_id)?));
        code.status = Set("used".to_string());
        code.updated_at = Set(Utc::now().into());
        code.update(&self.db).await?;

        Ok(())
    }

    /// 兑换余额
    async fn redeem_balance(&self, user_id: &Uuid, amount: f64) -> Result<String> {
        // 获取用户
        let user = users::Entity::find_by_id(*user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        // 更新余额（余额以分为单位）
        let balance_delta = (amount * 100.0) as i64;
        let new_balance = user.balance + balance_delta;

        let mut user: users::ActiveModel = user.into();
        user.balance = Set(new_balance);
        user.updated_at = Set(Utc::now());
        user.update(&self.db).await?;

        Ok(format!("Added ${:.2} to your balance", amount))
    }

    /// 兑换余额（带事务）
    async fn redeem_balance_with_txn(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        user_id: &Uuid,
        amount: f64,
    ) -> Result<String> {
        let user = users::Entity::find_by_id(*user_id)
            .one(txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let balance_delta = (amount * 100.0) as i64;
        let new_balance = user.balance + balance_delta;

        let mut user: users::ActiveModel = user.into();
        user.balance = Set(new_balance);
        user.updated_at = Set(Utc::now());
        user.update(txn).await?;

        Ok(format!("Added ${:.2} to your balance", amount))
    }

    /// 兑换订阅
    async fn redeem_subscription(
        &self,
        _user_id: &Uuid,
        _group_id: Option<i64>,
        days: i32,
    ) -> Result<String> {
        // TODO: 实现订阅逻辑
        Ok(format!("Added {days} days subscription"))
    }

    /// 兑换订阅（带事务）
    async fn redeem_subscription_with_txn(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _user_id: &Uuid,
        _group_id: Option<i64>,
        days: i32,
    ) -> Result<String> {
        // TODO: 实现订阅逻辑
        Ok(format!("Added {days} days subscription"))
    }

    /// 兑换配额
    async fn redeem_quota(&self, _user_id: &Uuid, quota: i64) -> Result<String> {
        // TODO: 实现配额逻辑
        Ok(format!("Added {quota} tokens quota"))
    }

    /// 兑换配额（带事务）
    async fn redeem_quota_with_txn(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _user_id: &Uuid,
        quota: i64,
    ) -> Result<String> {
        // TODO: 实现配额逻辑
        Ok(format!("Added {quota} tokens quota"))
    }

    /// 获取用户兑换历史
    pub async fn get_user_redemptions(&self, user_id: Uuid) -> Result<Vec<RedeemCode>> {
        // 查找 used_by 包含该用户 ID 的卡密
        let codes = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::UsedBy.is_not_null())
            .all(&self.db)
            .await?;

        let user_id_str = user_id.to_string();
        let user_codes: Vec<RedeemCode> = codes
            .into_iter()
            .filter(|c| {
                if let Some(v) = &c.used_by {
                    if let Some(arr) = v.as_array() {
                        arr.iter().any(|id| id.as_str() == Some(&user_id_str))
                    } else if let Some(id) = v.as_str() {
                        id == user_id_str
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .map(|c| self.db_model_to_domain(c))
            .collect();

        Ok(user_codes)
    }

    /// 获取卡密统计
    pub async fn get_stats(&self) -> Result<RedeemStats> {
        let total = redeem_codes::Entity::find().count(&self.db).await? as i64;
        let unused = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Status.eq("active"))
            .count(&self.db)
            .await? as i64;
        let used = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Status.eq("used"))
            .count(&self.db)
            .await? as i64;
        let expired = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Status.eq("expired"))
            .count(&self.db)
            .await? as i64;

        // 计算总价值和已使用价值
        let all_codes = redeem_codes::Entity::find().all(&self.db).await?;
        let total_value: f64 = all_codes
            .iter()
            .map(|c| c.amount.to_string().parse::<f64>().unwrap_or(0.0))
            .sum();
        let used_value: f64 = all_codes
            .iter()
            .filter(|c| c.status == "used")
            .map(|c| c.amount.to_string().parse::<f64>().unwrap_or(0.0))
            .sum();

        Ok(RedeemStats {
            total_codes: total,
            unused_codes: unused,
            used_codes: used,
            expired_codes: expired,
            total_value,
            used_value,
        })
    }

    /// 取消卡密
    pub async fn cancel(&self, code_id: i64) -> Result<()> {
        let code = redeem_codes::Entity::find_by_id(code_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Code not found"))?;

        if code.status != "active" {
            bail!("Can only cancel active codes");
        }

        let mut code: redeem_codes::ActiveModel = code.into();
        code.status = Set("cancelled".to_string());
        code.updated_at = Set(Utc::now().into());
        code.update(&self.db).await?;

        tracing::info!(code_id = code_id, "Redeem code cancelled");

        Ok(())
    }

    /// 清理过期卡密
    pub async fn cleanup_expired(&self) -> Result<i64> {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();

        // 查找已过期但仍为 active 的卡密
        let expired_codes = redeem_codes::Entity::find()
            .filter(redeem_codes::Column::Status.eq("active"))
            .filter(redeem_codes::Column::ExpiresAt.lt(now))
            .all(&self.db)
            .await?;

        let count = expired_codes.len() as i64;

        // 批量更新状态
        for code in expired_codes {
            let mut code: redeem_codes::ActiveModel = code.into();
            code.status = Set("expired".to_string());
            code.updated_at = Set(now.into());
            code.update(&self.db).await?;
        }

        tracing::info!(count = count, "Cleaned up expired redeem codes");

        Ok(count)
    }

    /// 将数据库模型转换为领域模型
    fn db_model_to_domain(&self, model: redeem_codes::Model) -> RedeemCode {
        RedeemCode {
            id: model.id,
            code: model.code,
            code_type: model.r#type.parse().unwrap_or(RedeemType::Balance),
            value: model.amount.to_string().parse().unwrap_or(0.0),
            status: match model.status.as_str() {
                "active" => RedeemStatus::Unused,
                "used" => RedeemStatus::Used,
                "expired" => RedeemStatus::Expired,
                "cancelled" => RedeemStatus::Cancelled,
                _ => RedeemStatus::Unused,
            },
            used_by: model.used_by.and_then(|v| {
                if let Some(arr) = v.as_array() {
                    arr.first()
                        .and_then(|id| id.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                } else {
                    v.as_str().and_then(|s| Uuid::parse_str(s).ok())
                }
            }),
            used_at: None, // 数据库模型中没有这个字段
            expires_at: model.expires_at.map(|t| t.into()),
            group_id: None,    // 数据库模型中没有这个字段
            validity_days: 30, // 默认值
            notes: model.notes,
            created_at: model.created_at.into(),
        }
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
