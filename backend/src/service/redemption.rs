//! 兑换码系统

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 兑换码类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedemptionCodeType {
    Balance,      // 余额
    Subscription, // 订阅
    Quota,        // 配额
}

impl std::fmt::Display for RedemptionCodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedemptionCodeType::Balance => write!(f, "balance"),
            RedemptionCodeType::Subscription => write!(f, "subscription"),
            RedemptionCodeType::Quota => write!(f, "quota"),
        }
    }
}

/// 兑换码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionCode {
    pub id: Uuid,
    pub code: String,
    pub code_type: RedemptionCodeType,
    pub value: i64, // 余额(分)、订阅天数、配额数量
    pub plan_id: Option<Uuid>,
    pub max_uses: i32,
    pub current_uses: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub notes: Option<String>,
}

/// 兑换记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionRecord {
    pub id: Uuid,
    pub code_id: Uuid,
    pub user_id: Uuid,
    pub redeemed_at: DateTime<Utc>,
    pub value_received: i64,
}

/// 兑换码服务
pub struct RedemptionService {
    db: DatabaseConnection,
    code_prefix: String,
    code_length: usize,
}

impl RedemptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            code_prefix: "FOX".to_string(),
            code_length: 16,
        }
    }
    
    /// 生成兑换码
    pub fn generate_code(&self) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        
        let mut rng = rand::thread_rng();
        let code: String = (0..self.code_length)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();
        
        format!("{}-{}", self.code_prefix, code)
    }
    
    /// 创建余额兑换码
    pub async fn create_balance_code(
        &self,
        balance: i64,
        max_uses: i32,
        expires_at: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Result<RedemptionCode> {
        let code = RedemptionCode {
            id: Uuid::new_v4(),
            code: self.generate_code(),
            code_type: RedemptionCodeType::Balance,
            value: balance,
            plan_id: None,
            max_uses,
            current_uses: 0,
            expires_at,
            created_by,
            created_at: Utc::now(),
            is_active: true,
            notes,
        };
        
        // TODO: 保存到数据库
        Ok(code)
    }
    
    /// 创建订阅兑换码
    pub async fn create_subscription_code(
        &self,
        plan_id: Uuid,
        days: i64,
        max_uses: i32,
        expires_at: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Result<RedemptionCode> {
        let code = RedemptionCode {
            id: Uuid::new_v4(),
            code: self.generate_code(),
            code_type: RedemptionCodeType::Subscription,
            value: days,
            plan_id: Some(plan_id),
            max_uses,
            current_uses: 0,
            expires_at,
            created_by,
            created_at: Utc::now(),
            is_active: true,
            notes,
        };
        
        Ok(code)
    }
    
    /// 创建配额兑换码
    pub async fn create_quota_code(
        &self,
        quota: i64,
        max_uses: i32,
        expires_at: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Result<RedemptionCode> {
        let code = RedemptionCode {
            id: Uuid::new_v4(),
            code: self.generate_code(),
            code_type: RedemptionCodeType::Quota,
            value: quota,
            plan_id: None,
            max_uses,
            current_uses: 0,
            expires_at,
            created_by,
            created_at: Utc::now(),
            is_active: true,
            notes,
        };
        
        Ok(code)
    }
    
    /// 兑换
    pub async fn redeem(&self, code_str: &str, user_id: Uuid) -> Result<RedemptionResult> {
        // 1. 验证兑换码
        let code = self.find_code(code_str).await?
            .ok_or_else(|| anyhow::anyhow!("Invalid code"))?;
        
        // 2. 检查是否有效
        if !code.is_active {
            bail!("Code is inactive");
        }
        
        // 3. 检查使用次数
        if code.current_uses >= code.max_uses {
            bail!("Code has reached maximum uses");
        }
        
        // 4. 检查过期时间
        if let Some(expires_at) = code.expires_at {
            if Utc::now() > expires_at {
                bail!("Code has expired");
            }
        }
        
        // 5. 检查用户是否已使用过
        if self.has_user_redeemed(code.id, user_id).await? {
            bail!("You have already redeemed this code");
        }
        
        // 6. 执行兑换
        let result = match code.code_type {
            RedemptionCodeType::Balance => {
                // TODO: 增加用户余额
                RedemptionResult {
                    code_type: code.code_type.clone(),
                    value: code.value,
                    message: format!("Added {} yuan to your balance", code.value as f64 / 100.0),
                }
            }
            RedemptionCodeType::Subscription => {
                // TODO: 增加订阅天数
                RedemptionResult {
                    code_type: code.code_type.clone(),
                    value: code.value,
                    message: format!("Added {} days subscription", code.value),
                }
            }
            RedemptionCodeType::Quota => {
                // TODO: 增加配额
                RedemptionResult {
                    code_type: code.code_type.clone(),
                    value: code.value,
                    message: format!("Added {} tokens to your quota", code.value),
                }
            }
        };
        
        // 7. 更新使用次数
        self.increment_usage(code.id).await?;
        
        // 8. 记录兑换
        self.record_redemption(code.id, user_id, code.value).await?;
        
        Ok(result)
    }
    
    /// 查找兑换码
    async fn find_code(&self, _code: &str) -> Result<Option<RedemptionCode>> {
        // TODO: 从数据库查询
        Ok(None)
    }
    
    /// 检查用户是否已兑换
    async fn has_user_redeemed(&self, _code_id: Uuid, _user_id: Uuid) -> Result<bool> {
        // TODO: 从数据库查询
        Ok(false)
    }
    
    /// 增加使用次数
    async fn increment_usage(&self, _code_id: Uuid) -> Result<()> {
        // TODO: 更新数据库
        Ok(())
    }
    
    /// 记录兑换
    async fn record_redemption(&self, _code_id: Uuid, _user_id: Uuid, _value: i64) -> Result<()> {
        // TODO: 插入数据库
        Ok(())
    }
    
    /// 批量创建兑换码
    pub async fn create_batch(
        &self,
        code_type: RedemptionCodeType,
        value: i64,
        plan_id: Option<Uuid>,
        count: i32,
        max_uses_per_code: i32,
        expires_at: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Result<Vec<RedemptionCode>> {
        let mut codes = Vec::new();
        
        for _ in 0..count {
            let code = RedemptionCode {
                id: Uuid::new_v4(),
                code: self.generate_code(),
                code_type: code_type.clone(),
                value,
                plan_id,
                max_uses: max_uses_per_code,
                current_uses: 0,
                expires_at,
                created_by,
                created_at: Utc::now(),
                is_active: true,
                notes: notes.clone(),
            };
            codes.push(code);
        }
        
        Ok(codes)
    }
    
    /// 禁用兑换码
    pub async fn disable_code(&self, _code_id: Uuid) -> Result<()> {
        // TODO: 更新数据库
        Ok(())
    }
    
    /// 获取兑换统计
    pub async fn get_stats(&self, _code_id: Uuid) -> Result<RedemptionStats> {
        Ok(RedemptionStats {
            total_uses: 0,
            total_value: 0,
            unique_users: 0,
        })
    }
}

/// 兑换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionResult {
    pub code_type: RedemptionCodeType,
    pub value: i64,
    pub message: String,
}

/// 兑换统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionStats {
    pub total_uses: i32,
    pub total_value: i64,
    pub unique_users: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_redemption_code_type_display() {
        assert_eq!(RedemptionCodeType::Balance.to_string(), "balance");
        assert_eq!(RedemptionCodeType::Subscription.to_string(), "subscription");
        assert_eq!(RedemptionCodeType::Quota.to_string(), "quota");
    }
    
    #[test]
    fn test_generate_code() {
        let service = RedemptionService::new(DatabaseConnection::default());
        let code = service.generate_code();
        
        assert!(code.starts_with("FOX-"));
        assert_eq!(code.len(), 20); // "FOX-" (4) + 16
    }
    
    #[test]
    fn test_generate_unique_codes() {
        let service = RedemptionService::new(DatabaseConnection::default());
        
        let code1 = service.generate_code();
        let code2 = service.generate_code();
        
        assert_ne!(code1, code2);
    }
    
    #[test]
    fn test_redemption_code_creation() {
        let code = RedemptionCode {
            id: Uuid::new_v4(),
            code: "FOX-ABCD1234EFGH5678".to_string(),
            code_type: RedemptionCodeType::Balance,
            value: 1000,
            plan_id: None,
            max_uses: 10,
            current_uses: 0,
            expires_at: None,
            created_by: Uuid::new_v4(),
            created_at: Utc::now(),
            is_active: true,
            notes: Some("Test code".to_string()),
        };
        
        assert_eq!(code.value, 1000);
        assert_eq!(code.max_uses, 10);
        assert!(code.is_active);
    }
    
    #[test]
    fn test_redemption_result() {
        let result = RedemptionResult {
            code_type: RedemptionCodeType::Balance,
            value: 500,
            message: "Added 5.00 yuan to your balance".to_string(),
        };
        
        assert_eq!(result.value, 500);
        assert!(result.message.contains("5.00"));
    }
}
