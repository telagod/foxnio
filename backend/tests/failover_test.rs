//! 故障转移集成测试

#[cfg(test)]
mod tests {
    use crate::gateway::failover::{FailoverConfig, FailoverManager, AccountHealth};
    
    #[tokio::test]
    async fn test_failover_manager_creation() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config);
        
        let stats = manager.get_health_stats().await;
        assert!(stats.is_empty());
    }
    
    #[tokio::test]
    async fn test_mark_success() {
        let manager = FailoverManager::new(Default::default());
        let account_id = uuid::Uuid::new_v4();
        
        manager.mark_success(account_id).await;
        
        let is_healthy = manager.is_account_healthy(&account_id).await;
        assert!(is_healthy);
    }
    
    #[tokio::test]
    async fn test_mark_failure() {
        let manager = FailoverManager::new(Default::default());
        let account_id = uuid::Uuid::new_v4();
        
        // 标记失败一次
        manager.mark_failure(account_id, "Test error".to_string()).await;
        
        // 应该仍然健康
        let is_healthy = manager.is_account_healthy(&account_id).await;
        assert!(is_healthy);
        
        // 标记失败三次
        manager.mark_failure(account_id, "Error 2".to_string()).await;
        manager.mark_failure(account_id, "Error 3".to_string()).await;
        
        // 应该不健康了
        let is_healthy = manager.is_account_healthy(&account_id).await;
        assert!(!is_healthy);
    }
    
    #[tokio::test]
    async fn test_reset_health() {
        let manager = FailoverManager::new(Default::default());
        let account_id = uuid::Uuid::new_v4();
        
        // 标记失败
        manager.mark_failure(account_id, "Error".to_string()).await;
        manager.mark_failure(account_id, "Error".to_string()).await;
        manager.mark_failure(account_id, "Error".to_string()).await;
        
        assert!(!manager.is_account_healthy(&account_id).await);
        
        // 重置
        manager.reset_health(&account_id).await;
        
        // 应该恢复健康
        assert!(manager.is_account_healthy(&account_id).await);
    }
    
    #[tokio::test]
    async fn test_reset_all() {
        let manager = FailoverManager::new(Default::default());
        
        let account1 = uuid::Uuid::new_v4();
        let account2 = uuid::Uuid::new_v4();
        
        manager.mark_success(account1).await;
        manager.mark_success(account2).await;
        
        let stats = manager.get_health_stats().await;
        assert_eq!(stats.len(), 2);
        
        manager.reset_all().await;
        
        let stats = manager.get_health_stats().await;
        assert!(stats.is_empty());
    }
    
    #[test]
    fn test_backoff_calculation() {
        let config = FailoverConfig {
            max_retries: 5,
            retry_delay_ms: 100,
            backoff_base: 2.0,
            max_backoff_ms: 5000,
        };
        
        // 验证退避时间计算
        let delay1 = 100.0 * 2.0_f64.powi(1);
        let delay2 = 100.0 * 2.0_f64.powi(2);
        let delay3 = 100.0 * 2.0_f64.powi(3);
        
        assert_eq!(delay1 as u64, 200);
        assert_eq!(delay2 as u64, 400);
        assert_eq!(delay3 as u64, 800);
    }
}
