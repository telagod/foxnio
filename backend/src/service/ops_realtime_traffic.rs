//! 运维实时流量监控 - Ops Realtime Traffic
//!
//! 提供实时流量监控和分析功能

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 流量统计窗口大小（秒）
const TRAFFIC_WINDOW_SIZE_SECS: i64 = 60;

/// 流量统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStats {
    pub timestamp: DateTime<Utc>,
    pub platform: String,
    pub requests_per_second: f64,
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
}

/// 实时流量数据点
#[derive(Debug, Clone)]
struct TrafficDataPoint {
    timestamp: DateTime<Utc>,
    platform: String,
    latency_ms: i64,
    success: bool,
}

/// 平台流量摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformTrafficSummary {
    pub platform: String,
    pub current_rps: f64,
    pub peak_rps: f64,
    pub total_today: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

/// 流量告警阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficAlertThreshold {
    pub platform: String,
    pub max_rps: f64,
    pub min_success_rate: f64,
    pub max_latency_ms: f64,
}

/// 实时流量监控器
pub struct RealtimeTrafficMonitor {
    // 流量数据缓冲区
    data_points: Arc<RwLock<Vec<TrafficDataPoint>>>,

    // 平台统计缓存
    platform_stats: Arc<RwLock<HashMap<String, TrafficStats>>>,

    // 告警阈值
    alert_thresholds: Arc<RwLock<HashMap<String, TrafficAlertThreshold>>>,

    // 配置
    window_size_secs: i64,
    max_data_points: usize,
}

impl RealtimeTrafficMonitor {
    /// 创建新的实时流量监控器
    pub fn new(window_size_secs: i64, max_data_points: usize) -> Self {
        Self {
            data_points: Arc::new(RwLock::new(Vec::new())),
            platform_stats: Arc::new(RwLock::new(HashMap::new())),
            alert_thresholds: Arc::new(RwLock::new(HashMap::new())),
            window_size_secs,
            max_data_points,
        }
    }

    /// 记录请求
    pub async fn record_request(
        &self,
        platform: &str,
        latency_ms: i64,
        success: bool,
    ) -> Result<()> {
        let point = TrafficDataPoint {
            timestamp: Utc::now(),
            platform: platform.to_string(),
            latency_ms,
            success,
        };

        // 添加数据点
        {
            let mut points = self.data_points.write().await;
            points.push(point);

            // 清理过期数据
            let cutoff = Utc::now() - Duration::seconds(self.window_size_secs);
            points.retain(|p| p.timestamp > cutoff);

            // 限制最大数据点数
            if points.len() > self.max_data_points {
                let excess = points.len() - self.max_data_points;
                points.drain(0..excess);
            }
        }

        // 更新统计
        self.update_platform_stats(platform).await?;

        Ok(())
    }

    /// 更新平台统计
    async fn update_platform_stats(&self, platform: &str) -> Result<()> {
        let points = self.data_points.read().await;

        // 筛选该平台的数据点
        let platform_points: Vec<_> = points.iter().filter(|p| p.platform == platform).collect();

        if platform_points.is_empty() {
            return Ok(());
        }

        // 计算统计信息
        let total_requests = platform_points.len() as i64;
        let successful_requests = platform_points.iter().filter(|p| p.success).count() as i64;
        let failed_requests = total_requests - successful_requests;

        let mut latencies: Vec<i64> = platform_points.iter().map(|p| p.latency_ms).collect();
        latencies.sort();

        let avg_latency_ms = latencies.iter().sum::<i64>() as f64 / latencies.len() as f64;
        let p50_latency_ms = percentile(&latencies, 50);
        let p95_latency_ms = percentile(&latencies, 95);
        let p99_latency_ms = percentile(&latencies, 99);

        // 计算 RPS
        let window_start = Utc::now() - Duration::seconds(self.window_size_secs);
        let window_points: Vec<_> = platform_points
            .iter()
            .filter(|p| p.timestamp > window_start)
            .collect();

        let requests_per_second = window_points.len() as f64 / self.window_size_secs as f64;

        let stats = TrafficStats {
            timestamp: Utc::now(),
            platform: platform.to_string(),
            requests_per_second,
            total_requests,
            successful_requests,
            failed_requests,
            avg_latency_ms,
            p50_latency_ms,
            p95_latency_ms,
            p99_latency_ms,
        };

        // 更新缓存
        let mut platform_stats = self.platform_stats.write().await;
        platform_stats.insert(platform.to_string(), stats);

        Ok(())
    }

    /// 获取平台统计
    pub async fn get_platform_stats(&self, platform: &str) -> Option<TrafficStats> {
        let stats = self.platform_stats.read().await;
        stats.get(platform).cloned()
    }

    /// 获取所有平台统计
    pub async fn get_all_platform_stats(&self) -> HashMap<String, TrafficStats> {
        self.platform_stats.read().await.clone()
    }

    /// 获取流量摘要
    pub async fn get_traffic_summary(&self, platform: &str) -> Option<PlatformTrafficSummary> {
        let stats = self.get_platform_stats(platform).await?;

        Some(PlatformTrafficSummary {
            platform: platform.to_string(),
            current_rps: stats.requests_per_second,
            peak_rps: stats.requests_per_second, // NOTE: 跟踪峰值
            total_today: stats.total_requests,
            success_rate: if stats.total_requests > 0 {
                stats.successful_requests as f64 / stats.total_requests as f64
            } else {
                0.0
            },
            avg_latency_ms: stats.avg_latency_ms,
        })
    }

    /// 设置告警阈值
    pub async fn set_alert_threshold(&self, threshold: TrafficAlertThreshold) {
        let mut thresholds = self.alert_thresholds.write().await;
        thresholds.insert(threshold.platform.clone(), threshold);
    }

    /// 检查告警
    pub async fn check_alerts(&self) -> Vec<TrafficAlert> {
        let mut alerts = Vec::new();

        let stats = self.platform_stats.read().await;
        let thresholds = self.alert_thresholds.read().await;

        for (platform, stat) in stats.iter() {
            if let Some(threshold) = thresholds.get(platform) {
                // 检查 RPS
                if stat.requests_per_second > threshold.max_rps {
                    alerts.push(TrafficAlert {
                        platform: platform.clone(),
                        alert_type: TrafficAlertType::HighRPS,
                        current_value: stat.requests_per_second,
                        threshold: threshold.max_rps,
                        triggered_at: Utc::now(),
                    });
                }

                // 检查成功率
                let success_rate = if stat.total_requests > 0 {
                    stat.successful_requests as f64 / stat.total_requests as f64
                } else {
                    1.0
                };

                if success_rate < threshold.min_success_rate {
                    alerts.push(TrafficAlert {
                        platform: platform.clone(),
                        alert_type: TrafficAlertType::LowSuccessRate,
                        current_value: success_rate,
                        threshold: threshold.min_success_rate,
                        triggered_at: Utc::now(),
                    });
                }

                // 检查延迟
                if stat.avg_latency_ms > threshold.max_latency_ms {
                    alerts.push(TrafficAlert {
                        platform: platform.clone(),
                        alert_type: TrafficAlertType::HighLatency,
                        current_value: stat.avg_latency_ms,
                        threshold: threshold.max_latency_ms,
                        triggered_at: Utc::now(),
                    });
                }
            }
        }

        alerts
    }

    /// 清理过期数据
    pub async fn cleanup_expired_data(&self) {
        let cutoff = Utc::now() - Duration::seconds(self.window_size_secs);

        let mut points = self.data_points.write().await;
        points.retain(|p| p.timestamp > cutoff);
    }
}

/// 流量告警类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficAlertType {
    HighRPS,
    LowSuccessRate,
    HighLatency,
}

/// 流量告警
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficAlert {
    pub platform: String,
    pub alert_type: TrafficAlertType,
    pub current_value: f64,
    pub threshold: f64,
    pub triggered_at: DateTime<Utc>,
}

/// 计算百分位数
fn percentile(sorted_values: &[i64], p: u32) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let idx = ((sorted_values.len() - 1) as f64 * p as f64 / 100.0).round() as usize;
    sorted_values[idx.min(sorted_values.len() - 1)] as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let values = vec![10, 20, 30, 40, 50];
        assert_eq!(percentile(&values, 50), 30.0);
        assert_eq!(percentile(&values, 0), 10.0);
        assert_eq!(percentile(&values, 100), 50.0);
    }

    #[tokio::test]
    async fn test_traffic_monitor() {
        let monitor = RealtimeTrafficMonitor::new(60, 1000);

        // 记录一些请求
        monitor.record_request("openai", 100, true).await.unwrap();
        monitor.record_request("openai", 150, true).await.unwrap();
        monitor.record_request("openai", 200, false).await.unwrap();

        let stats = monitor.get_platform_stats("openai").await.unwrap();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
    }
}
