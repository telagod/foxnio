//! 调度服务测试

#[cfg(test)]
#[allow(clippy::all)]
#[allow(clippy::all)]
mod tests {
    use crate::service::scheduler::{AccountRuntimeState, SchedulingStrategy, StickySession};
    use chrono::Utc;

    #[test]
    fn test_scheduling_strategy_variants() {
        let strategies = vec![
            SchedulingStrategy::RoundRobin,
            SchedulingStrategy::LeastConnections,
            SchedulingStrategy::PriorityFirst,
            SchedulingStrategy::Random,
            SchedulingStrategy::WeightedRoundRobin,
            SchedulingStrategy::HealthAware,
            SchedulingStrategy::Smart,
        ];

        assert_eq!(strategies.len(), 7);
    }

    #[test]
    fn test_account_runtime_state() {
        let state = AccountRuntimeState {
            account_id: uuid::Uuid::nil(),
            current_connections: 5,
            total_requests: 100,
            total_errors: 2,
            last_used: Some(Utc::now()),
            is_available: true,
            health_score: 95.0,
        };

        assert_eq!(state.current_connections, 5);
        assert_eq!(state.total_requests, 100);
        assert_eq!(state.total_errors, 2);
        assert!(state.is_available);
        assert_eq!(state.health_score, 95.0);
    }

    #[test]
    fn test_sticky_session() {
        let session = StickySession {
            account_id: uuid::Uuid::nil(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            request_count: 5,
        };

        assert_eq!(session.request_count, 5);
    }

    #[test]
    fn test_sticky_session_expiry() {
        let now = Utc::now();
        let old_time = now - chrono::Duration::hours(2);

        let session = StickySession {
            account_id: uuid::Uuid::nil(),
            created_at: old_time,
            last_accessed: old_time,
            request_count: 1,
        };

        // 检查是否过期 (超过 1 小时)
        let expired = (now - session.last_accessed).num_seconds() > 3600;
        assert!(expired);
    }
}
