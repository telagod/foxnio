use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Window statistics for operations monitoring
pub struct OpsWindowStats {
    window_size_seconds: u64,
    stats: WindowStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowStatistics {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub request_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: i64,
    pub value: f64,
}

impl OpsWindowStats {
    pub fn new(window_size_seconds: u64) -> Self {
        let now = Utc::now();
        Self {
            window_size_seconds,
            stats: WindowStatistics {
                window_start: now - chrono::Duration::seconds(window_size_seconds as i64),
                window_end: now,
                request_count: 0,
                success_count: 0,
                error_count: 0,
                avg_latency_ms: 0.0,
                min_latency_ms: 0,
                max_latency_ms: 0,
                p50_latency_ms: 0,
                p95_latency_ms: 0,
                p99_latency_ms: 0,
            },
        }
    }

    /// Record latency
    pub fn record(&mut self, latency_ms: u64, success: bool) {
        self.stats.request_count += 1;

        if success {
            self.stats.success_count += 1;
        } else {
            self.stats.error_count += 1;
        }

        // Update min/max
        if self.stats.request_count == 1 {
            self.stats.min_latency_ms = latency_ms;
            self.stats.max_latency_ms = latency_ms;
        } else {
            self.stats.min_latency_ms = self.stats.min_latency_ms.min(latency_ms);
            self.stats.max_latency_ms = self.stats.max_latency_ms.max(latency_ms);
        }

        // Update average
        let total = self.stats.avg_latency_ms * (self.stats.request_count - 1) as f64;
        self.stats.avg_latency_ms = (total + latency_ms as f64) / self.stats.request_count as f64;
    }

    /// Get error rate
    pub fn error_rate(&self) -> f64 {
        if self.stats.request_count == 0 {
            0.0
        } else {
            self.stats.error_count as f64 / self.stats.request_count as f64
        }
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        1.0 - self.error_rate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_stats() {
        let mut stats = OpsWindowStats::new(60);

        stats.record(100, true);
        stats.record(200, true);
        stats.record(150, false);

        assert_eq!(stats.stats.request_count, 3);
        assert_eq!(stats.stats.error_count, 1);
        assert!(stats.error_rate() > 0.0);
    }
}
