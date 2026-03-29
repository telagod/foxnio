//! Dashboard service

use serde::{Deserialize, Serialize};

/// Dashboard stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_users: u64,
    pub active_users: u64,
    pub total_requests: u64,
    pub total_tokens: u64,
    pub revenue: f64,
}

/// Dashboard service
pub struct DashboardService {
    stats: std::sync::Arc<tokio::sync::RwLock<DashboardStats>>,
}

impl Default for DashboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardService {
    pub fn new() -> Self {
        Self {
            stats: std::sync::Arc::new(tokio::sync::RwLock::new(DashboardStats {
                total_users: 0,
                active_users: 0,
                total_requests: 0,
                total_tokens: 0,
                revenue: 0.0,
            })),
        }
    }

    pub async fn get_stats(&self) -> DashboardStats {
        self.stats.read().await.clone()
    }

    pub async fn update_stats(&self, stats: DashboardStats) {
        let mut current = self.stats.write().await;
        *current = stats;
    }

    pub async fn increment_requests(&self, count: u64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += count;
    }

    pub async fn increment_tokens(&self, count: u64) {
        let mut stats = self.stats.write().await;
        stats.total_tokens += count;
    }

    pub async fn add_revenue(&self, amount: f64) {
        let mut stats = self.stats.write().await;
        stats.revenue += amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard() {
        let service = DashboardService::new();

        service.increment_requests(10).await;
        service.add_revenue(5.0).await;

        let stats = service.get_stats().await;
        assert_eq!(stats.total_requests, 10);
        assert_eq!(stats.revenue, 5.0);
    }
}
