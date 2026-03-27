//! 故障转移测试

#[cfg(test)]
mod tests {
    use crate::gateway::failover::{FailoverConfig, FailoverError};

    #[test]
    fn test_failover_config_default() {
        let config = FailoverConfig::default();
        
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 100);
        assert_eq!(config.backoff_base, 2.0);
        assert_eq!(config.max_backoff_ms, 5000);
    }
    
    #[test]
    fn test_failover_config_custom() {
        let config = FailoverConfig {
            max_retries: 5,
            retry_delay_ms: 200,
            backoff_base: 1.5,
            max_backoff_ms: 10000,
        };
        
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.retry_delay_ms, 200);
    }
    
    #[test]
    fn test_failover_error_display() {
        let error = FailoverError {
            attempts: 3,
            last_status: 500,
            last_message: "Internal Server Error".to_string(),
            account_errors: vec![],
        };
        
        let display = format!("{}", error);
        assert!(display.contains("3 attempts"));
        assert!(display.contains("500"));
        assert!(display.contains("Internal Server Error"));
    }
    
    #[test]
    fn test_backoff_calculation() {
        // 模拟退避时间计算
        let base_ms = 100;
        let backoff_base = 2.0;
        
        // 第 1 次重试: 100ms
        let delay1 = base_ms as f64 * backoff_base.powi(1);
        assert_eq!(delay1 as u64, 200);
        
        // 第 2 次重试: 400ms
        let delay2 = base_ms as f64 * backoff_base.powi(2);
        assert_eq!(delay2 as u64, 400);
        
        // 第 3 次重试: 800ms
        let delay3 = base_ms as f64 * backoff_base.powi(3);
        assert_eq!(delay3 as u64, 800);
    }
}
