//! Dashboard aggregation service

use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Aggregated metrics
#[derive(Debug, Clone)]
pub struct AggregatedMetrics {
    pub timestamp: DateTime<Utc>,
    pub requests_per_minute: f64,
    pub avg_response_time_ms: f64,
    pub error_rate: f64,
    pub tokens_per_second: f64,
}

/// Dashboard aggregation service
pub struct DashboardAggregationService {
    metrics: Arc<RwLock<Vec<AggregatedMetrics>>>,
    max_entries: usize,
}

impl Default for DashboardAggregationService {
    fn default() -> Self {
        Self::new(1440) // 24 hours at 1-minute intervals
    }
}

impl DashboardAggregationService {
    pub fn new(max_entries: usize) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
            max_entries,
        }
    }

    pub async fn record(&self, metrics: AggregatedMetrics) {
        let mut data = self.metrics.write().await;
        data.push(metrics);

        if data.len() > self.max_entries {
            data.remove(0);
        }
    }

    pub async fn get_recent(&self, minutes: usize) -> Vec<AggregatedMetrics> {
        let data = self.metrics.read().await;
        data.iter().rev().take(minutes).cloned().collect()
    }

    pub async fn get_average(&self, minutes: usize) -> Option<AggregatedMetrics> {
        let data = self.get_recent(minutes).await;

        if data.is_empty() {
            return None;
        }

        let count = data.len() as f64;
        Some(AggregatedMetrics {
            timestamp: Utc::now(),
            requests_per_minute: data.iter().map(|m| m.requests_per_minute).sum::<f64>() / count,
            avg_response_time_ms: data.iter().map(|m| m.avg_response_time_ms).sum::<f64>() / count,
            error_rate: data.iter().map(|m| m.error_rate).sum::<f64>() / count,
            tokens_per_second: data.iter().map(|m| m.tokens_per_second).sum::<f64>() / count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aggregation() {
        let service = DashboardAggregationService::new(100);

        service
            .record(AggregatedMetrics {
                timestamp: Utc::now(),
                requests_per_minute: 100.0,
                avg_response_time_ms: 50.0,
                error_rate: 0.01,
                tokens_per_second: 1000.0,
            })
            .await;

        let recent = service.get_recent(1).await;
        assert_eq!(recent.len(), 1);
    }
}
