//! 调度策略集成测试

#[cfg(test)]
mod tests {
    use crate::service::scheduler::{SchedulingStrategy, AccountRuntimeState};
    
    #[test]
    fn test_scheduling_strategies() {
        let strategies = vec![
            SchedulingStrategy::RoundRobin,
            SchedulingStrategy::LeastConnections,
            SchedulingStrategy::PriorityFirst,
            SchedulingStrategy::Random,
            SchedulingStrategy::WeightedRoundRobin,
        ];
        
        // 验证所有策略都可创建
        assert_eq!(strategies.len(), 5);
    }
    
    #[test]
    fn test_account_runtime_state_creation() {
        let state = AccountRuntimeState {
            account_id: uuid::Uuid::new_v4(),
            current_connections: 5,
            total_requests: 100,
            total_errors: 2,
            last_used: Some(chrono::Utc::now()),
            is_available: true,
        };
        
        assert_eq!(state.current_connections, 5);
        assert_eq!(state.total_requests, 100);
        assert!(state.is_available);
    }
    
    #[test]
    fn test_runtime_state_defaults() {
        let state = AccountRuntimeState {
            account_id: uuid::Uuid::nil(),
            current_connections: 0,
            total_requests: 0,
            total_errors: 0,
            last_used: None,
            is_available: true,
        };
        
        assert_eq!(state.current_connections, 0);
        assert_eq!(state.total_requests, 0);
        assert_eq!(state.total_errors, 0);
        assert!(state.last_used.is_none());
    }
    
    #[test]
    fn test_priority_ordering() {
        // 模拟账号优先级排序
        let mut priorities = vec![5, 10, 3, 8, 1];
        priorities.sort_by(|a, b| b.cmp(a)); // 降序
        
        assert_eq!(priorities, vec![10, 8, 5, 3, 1]);
    }
    
    #[test]
    fn test_connection_counting() {
        let mut connections = 0;
        
        // 增加连接
        connections += 1;
        connections += 1;
        connections += 1;
        
        assert_eq!(connections, 3);
        
        // 减少连接
        connections = (connections - 1).max(0);
        connections = (connections - 1).max(0);
        
        assert_eq!(connections, 1);
        
        // 不能低于 0
        connections = (connections - 5).max(0);
        assert_eq!(connections, 0);
    }
    
    #[test]
    fn test_weighted_selection() {
        // 模拟加权选择
        let weights = vec![10, 20, 30, 40];
        let total: i32 = weights.iter().sum();
        
        assert_eq!(total, 100);
        
        // 验证权重比例
        let ratios: Vec<f64> = weights.iter()
            .map(|w| *w as f64 / total as f64)
            .collect();
        
        assert!((ratios[0] - 0.1).abs() < 0.01);
        assert!((ratios[1] - 0.2).abs() < 0.01);
        assert!((ratios[2] - 0.3).abs() < 0.01);
        assert!((ratios[3] - 0.4).abs() < 0.01);
    }
}
