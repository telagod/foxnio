//! 兑换码系统测试

#[cfg(test)]
#[allow(clippy::all)]
mod tests {
    use crate::service::redemption::{
        RedemptionCode, RedemptionCodeType, RedemptionResult, RedemptionStats,
    };

    #[test]
    fn test_redemption_code_types() {
        let types = vec![
            RedemptionCodeType::Balance,
            RedemptionCodeType::Subscription,
            RedemptionCodeType::Quota,
        ];

        assert_eq!(types.len(), 3);
    }

    #[test]
    fn test_redemption_code_creation() {
        let code = RedemptionCode {
            id: uuid::Uuid::new_v4(),
            code: "FOX-ABCD1234EFGH5678".to_string(),
            code_type: RedemptionCodeType::Balance,
            value: 1000, // 10 yuan
            plan_id: None,
            max_uses: 10,
            current_uses: 0,
            expires_at: None,
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            is_active: true,
            notes: Some("Test code".to_string()),
        };

        assert!(code.code.starts_with("FOX-"));
        assert_eq!(code.value, 1000);
        assert_eq!(code.max_uses, 10);
    }

    #[test]
    fn test_code_validity_check() {
        let code = RedemptionCode {
            id: uuid::Uuid::new_v4(),
            code: "FOX-TEST".to_string(),
            code_type: RedemptionCodeType::Balance,
            value: 500,
            plan_id: None,
            max_uses: 5,
            current_uses: 3,
            expires_at: None,
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            is_active: true,
            notes: None,
        };

        // 检查使用次数
        assert!(code.current_uses < code.max_uses);

        // 检查是否激活
        assert!(code.is_active);
    }

    #[test]
    fn test_code_expiry() {
        let now = chrono::Utc::now();
        let expired_time = now - chrono::Duration::hours(1);
        let future_time = now + chrono::Duration::days(30);

        // 已过期的码
        let expired = expired_time < now;
        assert!(expired);

        // 未过期的码
        let valid = future_time > now;
        assert!(valid);
    }

    #[test]
    fn test_redemption_result() {
        let result = RedemptionResult {
            code_type: RedemptionCodeType::Balance,
            value: 1000,
            message: "Added 10.00 yuan to your balance".to_string(),
        };

        assert_eq!(result.code_type, RedemptionCodeType::Balance);
        assert_eq!(result.value, 1000);
    }

    #[test]
    fn test_redemption_stats() {
        let stats = RedemptionStats {
            total_uses: 50,
            total_value: 50000,
            unique_users: 25,
        };

        assert_eq!(stats.total_uses, 50);
        assert_eq!(stats.unique_users, 25);

        // 平均每人使用次数
        let avg_uses = stats.total_uses as f64 / stats.unique_users as f64;
        assert_eq!(avg_uses, 2.0);
    }

    #[test]
    fn test_code_format() {
        // 验证兑换码格式
        let codes = vec![
            "FOX-ABCD1234EFGH5678",
            "FOX-XXXXXXXXXXXXXXXX",
            "FOX-1234567890ABCDEF",
        ];

        for code in codes {
            assert!(code.starts_with("FOX-"));
            assert_eq!(code.len(), 20);
        }
    }

    #[test]
    fn test_balance_code_value() {
        // 余额码：value 表示分
        let value = 1000; // 分
        let yuan = value as f64 / 100.0;

        assert_eq!(yuan, 10.0);
    }

    #[test]
    fn test_subscription_code_value() {
        // 订阅码：value 表示天数
        let days = 30;

        assert!(days > 0);
        assert!(days <= 365);
    }
}
