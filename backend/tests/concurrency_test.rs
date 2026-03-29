#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 并发控制集成测试

#[cfg(test)]
mod tests {
    use foxnio::service::concurrency::{
        ConcurrencyConfig, ConcurrencyController, ConcurrencyError,
    };

    fn test_config() -> ConcurrencyConfig {
        ConcurrencyConfig {
            user_max_concurrent: 5,
            account_max_concurrent: 10,
            api_key_max_concurrent: 5,
            global_max_concurrent: 100,
            enable_dynamic_adjustment: false,
            adjustment_interval_seconds: 60,
            high_load_threshold: 0.8,
            low_load_threshold: 0.3,
        }
    }

    #[tokio::test]
    async fn test_concurrency_controller_creation() {
        let config = ConcurrencyConfig::default();
        let controller = ConcurrencyController::new(config);

        let stats = controller.get_stats().await;

        assert_eq!(stats.global_available, 1000);
        assert_eq!(stats.total_users, 0);
    }

    #[tokio::test]
    async fn test_acquire_success() {
        let config = test_config();
        let controller = ConcurrencyController::new(config);

        let result = controller.try_acquire("user1", "account1", "key1").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_acquire_user_limit() {
        let config = ConcurrencyConfig {
            user_max_concurrent: 2,
            account_max_concurrent: 10,
            api_key_max_concurrent: 10,
            global_max_concurrent: 100,
            enable_dynamic_adjustment: false,
            adjustment_interval_seconds: 60,
            high_load_threshold: 0.8,
            low_load_threshold: 0.3,
        };

        let controller = ConcurrencyController::new(config);

        // 获取两个槽位
        let _slot1 = controller
            .try_acquire("user1", "account1", "key1")
            .await
            .unwrap();
        let _slot2 = controller
            .try_acquire("user1", "account2", "key2")
            .await
            .unwrap();

        // 第三个应该失败
        let result = controller.try_acquire("user1", "account3", "key3").await;

        assert!(matches!(result, Err(ConcurrencyError::User)));
    }

    #[tokio::test]
    async fn test_acquire_global_limit() {
        let config = ConcurrencyConfig {
            user_max_concurrent: 100,
            account_max_concurrent: 100,
            api_key_max_concurrent: 100,
            global_max_concurrent: 2,
            enable_dynamic_adjustment: false,
            adjustment_interval_seconds: 60,
            high_load_threshold: 0.8,
            low_load_threshold: 0.3,
        };

        let controller = ConcurrencyController::new(config);

        // 获取两个槽位
        let _slot1 = controller
            .try_acquire("user1", "account1", "key1")
            .await
            .unwrap();
        let _slot2 = controller
            .try_acquire("user2", "account2", "key2")
            .await
            .unwrap();

        // 第三个应该失败（全局限制）
        let result = controller.try_acquire("user3", "account3", "key3").await;

        assert!(matches!(result, Err(ConcurrencyError::Global)));
    }

    #[tokio::test]
    async fn test_concurrency_stats_update() {
        let config = ConcurrencyConfig::default();
        let controller = ConcurrencyController::new(config);

        // 初始状态
        let stats = controller.get_stats().await;
        assert_eq!(stats.total_users, 0);

        // 获取槽位（会创建用户信号量）
        let _slot = controller
            .try_acquire("user1", "account1", "key1")
            .await
            .unwrap();

        let stats = controller.get_stats().await;
        assert_eq!(stats.total_users, 1);
        assert_eq!(stats.total_accounts, 1);
        assert_eq!(stats.total_api_keys, 1);
    }

    #[test]
    fn test_concurrency_error_display() {
        let errors = vec![
            ConcurrencyError::Global,
            ConcurrencyError::User,
            ConcurrencyError::Account,
            ConcurrencyError::ApiKey,
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(!display.is_empty());
        }
    }
}
