//! Proxy latency cache service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Proxy latency entry
#[derive(Debug, Clone)]
pub struct ProxyLatencyEntry {
    /// Proxy ID
    pub proxy_id: String,
    /// Average latency in ms
    pub avg_latency_ms: f64,
    /// Min latency
    pub min_latency_ms: u64,
    /// Max latency
    pub max_latency_ms: u64,
    /// Sample count
    pub sample_count: u64,
    /// Last updated
    pub last_updated: Instant,
    /// TTL
    pub ttl: Duration,
}

impl ProxyLatencyEntry {
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        self.last_updated.elapsed() > self.ttl
    }
}

/// Proxy latency cache
pub struct ProxyLatencyCache {
    /// Latency entries
    entries: Arc<RwLock<HashMap<String, ProxyLatencyEntry>>>,
    /// Default TTL
    default_ttl: Duration,
    /// Max samples per entry
    max_samples: u64,
}

impl Default for ProxyLatencyCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(300), 100)
    }
}

impl ProxyLatencyCache {
    /// Create a new cache
    pub fn new(default_ttl: Duration, max_samples: u64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
            max_samples,
        }
    }

    /// Record a latency measurement
    pub async fn record_latency(&self, proxy_id: &str, latency_ms: u64) {
        let mut entries = self.entries.write().await;

        use std::collections::hash_map::Entry;
        let entry = match entries.entry(proxy_id.to_string()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                // First measurement - don't increment sample_count
                e.insert(ProxyLatencyEntry {
                    proxy_id: proxy_id.to_string(),
                    avg_latency_ms: latency_ms as f64,
                    min_latency_ms: latency_ms,
                    max_latency_ms: latency_ms,
                    sample_count: 1,
                    last_updated: Instant::now(),
                    ttl: self.default_ttl,
                });
                return;
            }
        };

        // Update statistics for existing entry
        if entry.sample_count < self.max_samples {
            let total = entry.avg_latency_ms * entry.sample_count as f64;
            entry.sample_count += 1;
            entry.avg_latency_ms = (total + latency_ms as f64) / entry.sample_count as f64;
            entry.min_latency_ms = entry.min_latency_ms.min(latency_ms);
            entry.max_latency_ms = entry.max_latency_ms.max(latency_ms);
        } else {
            // Reset with new measurement
            entry.avg_latency_ms = latency_ms as f64;
            entry.min_latency_ms = latency_ms;
            entry.max_latency_ms = latency_ms;
            entry.sample_count = 1;
        }

        entry.last_updated = Instant::now();
    }

    /// Get latency for a proxy
    pub async fn get_latency(&self, proxy_id: &str) -> Option<ProxyLatencyEntry> {
        let entries = self.entries.read().await;
        entries.get(proxy_id).filter(|e| !e.is_expired()).cloned()
    }

    /// Get average latency
    pub async fn get_avg_latency(&self, proxy_id: &str) -> Option<f64> {
        self.get_latency(proxy_id).await.map(|e| e.avg_latency_ms)
    }

    /// Get fastest proxy
    pub async fn get_fastest(&self) -> Option<String> {
        let entries = self.entries.read().await;
        entries
            .values()
            .filter(|e| !e.is_expired())
            .min_by(|a, b| a.avg_latency_ms.partial_cmp(&b.avg_latency_ms).unwrap())
            .map(|e| e.proxy_id.clone())
    }

    /// Clear expired entries
    pub async fn clear_expired(&self) {
        let mut entries = self.entries.write().await;
        entries.retain(|_, e| !e.is_expired());
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        self.entries.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_get() {
        let cache = ProxyLatencyCache::new(Duration::from_secs(60), 100);

        cache.record_latency("proxy-1", 100).await;
        cache.record_latency("proxy-1", 200).await;

        let entry = cache.get_latency("proxy-1").await.unwrap();
        assert_eq!(entry.sample_count, 2);
        assert_eq!(entry.min_latency_ms, 100);
        assert_eq!(entry.max_latency_ms, 200);
    }

    #[tokio::test]
    async fn test_get_fastest() {
        let cache = ProxyLatencyCache::new(Duration::from_secs(60), 100);

        cache.record_latency("proxy-1", 200).await;
        cache.record_latency("proxy-2", 100).await;

        let fastest = cache.get_fastest().await.unwrap();
        assert_eq!(fastest, "proxy-2");
    }
}
