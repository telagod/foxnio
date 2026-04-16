//! 账号健康评分服务
//!
//! 提供多维度账号健康评分，用于智能调度决策

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 健康分数 (0-100)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub account_id: Uuid,
    pub score: f64,
    pub factors: HealthFactors,
    pub last_updated: DateTime<Utc>,
    pub sample_count: u64,
}

/// 健康因素分解
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFactors {
    /// 成功率得分 (权重 40%)
    pub success_rate_score: f64,
    /// 平均延迟得分 (权重 20%)
    pub avg_latency_score: f64,
    /// 错误率得分 (权重 20%)
    pub error_rate_score: f64,
    /// 限流率得分 (权重 10%)
    pub rate_limit_score: f64,
    /// 可用性得分 (权重 10%)
    pub availability_score: f64,
}

/// 滑动窗口统计数据
#[derive(Debug, Clone, Default)]
pub struct WindowStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub total_latency_ms: u64,
    pub availability_checks: u64,
    pub availability_failures: u64,
}

/// 健康评分配置
#[derive(Debug, Clone)]
pub struct HealthScorerConfig {
    /// 统计窗口大小（秒）
    pub window_seconds: i64,
    /// 成功率阈值
    pub success_rate_threshold: f64,
    /// 延迟阈值（毫秒）
    pub latency_threshold_ms: u64,
    /// 错误率阈值
    pub error_rate_threshold: f64,
    /// 限流率阈值
    pub rate_limit_threshold: f64,
    /// 最小样本数
    pub min_sample_count: u64,
}

impl Default for HealthScorerConfig {
    fn default() -> Self {
        Self {
            window_seconds: 300, // 5 分钟
            success_rate_threshold: 0.99,
            latency_threshold_ms: 3000,
            error_rate_threshold: 0.05,
            rate_limit_threshold: 0.1,
            min_sample_count: 10,
        }
    }
}

/// 健康评分服务
pub struct HealthScorer {
    config: HealthScorerConfig,
    /// 每个账号的滑动窗口统计
    window_stats: Arc<RwLock<HashMap<Uuid, WindowStats>>>,
    /// 缓存的评分结果
    cached_scores: Arc<RwLock<HashMap<Uuid, HealthScore>>>,
    /// 窗口开始时间
    window_start: Arc<RwLock<DateTime<Utc>>>,
}

impl HealthScorer {
    pub fn new(config: HealthScorerConfig) -> Self {
        Self {
            config,
            window_stats: Arc::new(RwLock::new(HashMap::new())),
            cached_scores: Arc::new(RwLock::new(HashMap::new())),
            window_start: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// 记录请求结果
    pub async fn record_request(
        &self,
        account_id: Uuid,
        success: bool,
        latency_ms: u64,
        is_rate_limited: bool,
    ) {
        let mut stats = self.window_stats.write().await;
        let stat = stats.entry(account_id).or_insert_with(WindowStats::default);

        stat.total_requests += 1;
        if success {
            stat.successful_requests += 1;
        } else {
            stat.failed_requests += 1;
        }
        if is_rate_limited {
            stat.rate_limited_requests += 1;
        }
        stat.total_latency_ms += latency_ms;

        // 清除缓存，下次获取时重新计算
        let mut scores = self.cached_scores.write().await;
        scores.remove(&account_id);
    }

    /// 记录可用性检查
    pub async fn record_availability(&self, account_id: Uuid, available: bool) {
        let mut stats = self.window_stats.write().await;
        let stat = stats.entry(account_id).or_insert_with(WindowStats::default);

        stat.availability_checks += 1;
        if !available {
            stat.availability_failures += 1;
        }

        // 清除缓存
        let mut scores = self.cached_scores.write().await;
        scores.remove(&account_id);
    }

    /// 计算账号健康分数
    pub async fn calculate(&self, account_id: Uuid) -> Result<HealthScore> {
        // 检查缓存
        {
            let scores = self.cached_scores.read().await;
            if let Some(score) = scores.get(&account_id) {
                return Ok(score.clone());
            }
        }

        let stats = self.window_stats.read().await;
        let stat = stats.get(&account_id).cloned().unwrap_or_default();

        let score = self.compute_score(account_id, &stat);

        // 缓存结果
        let mut scores = self.cached_scores.write().await;
        scores.insert(account_id, score.clone());

        Ok(score)
    }

    /// 批量计算所有账号的健康分数
    pub async fn calculate_all(&self) -> Result<Vec<HealthScore>> {
        let stats = self.window_stats.read().await;
        let mut results = Vec::new();

        for account_id in stats.keys() {
            let stat = stats.get(account_id).cloned().unwrap_or_default();
            let score = self.compute_score(*account_id, &stat);
            results.push(score);
        }

        // 更新缓存
        let mut scores = self.cached_scores.write().await;
        for score in &results {
            scores.insert(score.account_id, score.clone());
        }

        Ok(results)
    }

    /// 计算分数核心逻辑
    fn compute_score(&self, account_id: Uuid, stat: &WindowStats) -> HealthScore {
        // 如果样本数不足，返回默认高分
        if stat.total_requests < self.config.min_sample_count {
            return HealthScore {
                account_id,
                score: 100.0,
                factors: HealthFactors {
                    success_rate_score: 100.0,
                    avg_latency_score: 100.0,
                    error_rate_score: 100.0,
                    rate_limit_score: 100.0,
                    availability_score: 100.0,
                },
                last_updated: Utc::now(),
                sample_count: stat.total_requests,
            };
        }

        // 计算成功率得分 (权重 40%)
        let success_rate = if stat.total_requests > 0 {
            stat.successful_requests as f64 / stat.total_requests as f64
        } else {
            1.0
        };
        let success_rate_score = self.calculate_success_rate_score(success_rate);

        // 计算延迟得分 (权重 20%)
        let avg_latency = if stat.successful_requests > 0 {
            stat.total_latency_ms / stat.successful_requests
        } else {
            0
        };
        let avg_latency_score = self.calculate_latency_score(avg_latency);

        // 计算错误率得分 (权重 20%)
        let error_rate = if stat.total_requests > 0 {
            stat.failed_requests as f64 / stat.total_requests as f64
        } else {
            0.0
        };
        let error_rate_score = self.calculate_error_rate_score(error_rate);

        // 计算限流率得分 (权重 10%)
        let rate_limit_rate = if stat.total_requests > 0 {
            stat.rate_limited_requests as f64 / stat.total_requests as f64
        } else {
            0.0
        };
        let rate_limit_score = self.calculate_rate_limit_score(rate_limit_rate);

        // 计算可用性得分 (权重 10%)
        let availability_rate = if stat.availability_checks > 0 {
            1.0 - (stat.availability_failures as f64 / stat.availability_checks as f64)
        } else {
            1.0
        };
        let availability_score = availability_rate * 100.0;

        // 加权计算总分
        let total_score = success_rate_score * 0.4
            + avg_latency_score * 0.2
            + error_rate_score * 0.2
            + rate_limit_score * 0.1
            + availability_score * 0.1;

        let score = total_score.clamp(0.0, 100.0);

        HealthScore {
            account_id,
            score,
            factors: HealthFactors {
                success_rate_score,
                avg_latency_score,
                error_rate_score,
                rate_limit_score,
                availability_score,
            },
            last_updated: Utc::now(),
            sample_count: stat.total_requests,
        }
    }

    /// 成功率评分：99% → 100, 95% → 50, <90% → 0
    fn calculate_success_rate_score(&self, rate: f64) -> f64 {
        if rate >= self.config.success_rate_threshold {
            100.0
        } else if rate >= 0.95 {
            (rate - 0.95) / (self.config.success_rate_threshold - 0.95) * 50.0 + 50.0
        } else if rate >= 0.90 {
            (rate - 0.90) / 0.05 * 50.0
        } else {
            0.0
        }
    }

    /// 延迟评分：<1s → 100, 1-3s → 线性, >3s → 0
    fn calculate_latency_score(&self, latency_ms: u64) -> f64 {
        if latency_ms <= 1000 {
            100.0
        } else if latency_ms <= self.config.latency_threshold_ms {
            let range = (self.config.latency_threshold_ms - 1000) as f64;
            let progress = (latency_ms - 1000) as f64;
            (1.0 - progress / range) * 100.0
        } else {
            0.0
        }
    }

    /// 错误率评分：<1% → 100, 1-5% → 线性, >5% → 0
    fn calculate_error_rate_score(&self, rate: f64) -> f64 {
        if rate <= 0.01 {
            100.0
        } else if rate <= self.config.error_rate_threshold {
            (1.0 - (rate - 0.01) / (self.config.error_rate_threshold - 0.01)) * 100.0
        } else {
            0.0
        }
    }

    /// 限流率评分：<5% → 100, 5-10% → 线性, >10% → 0
    fn calculate_rate_limit_score(&self, rate: f64) -> f64 {
        if rate <= 0.05 {
            100.0
        } else if rate <= self.config.rate_limit_threshold {
            (1.0 - (rate - 0.05) / (self.config.rate_limit_threshold - 0.05)) * 100.0
        } else {
            0.0
        }
    }

    /// 获取账号的健康分数（简化接口）
    pub async fn get_score(&self, account_id: Uuid) -> f64 {
        self.calculate(account_id)
            .await
            .map(|s| s.score)
            .unwrap_or(100.0)
    }

    /// 获取所有账号的评分摘要
    pub async fn get_summary(&self) -> HashMap<Uuid, f64> {
        let scores = self.cached_scores.read().await;
        scores
            .iter()
            .map(|(id, score)| (*id, score.score))
            .collect()
    }

    /// 重置窗口统计
    pub async fn reset_window(&self) {
        let mut stats = self.window_stats.write().await;
        stats.clear();

        let mut scores = self.cached_scores.write().await;
        scores.clear();

        let mut start = self.window_start.write().await;
        *start = Utc::now();
    }

    /// 清理过期数据
    pub async fn cleanup(&self) {
        // 简单实现：保留当前窗口数据
        // 生产环境可以基于时间戳清理
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_scorer_basic() {
        let scorer = HealthScorer::new(HealthScorerConfig::default());
        let account_id = Uuid::new_v4();

        // 记录一些请求
        for _ in 0..20 {
            scorer.record_request(account_id, true, 500, false).await;
        }

        let score = scorer.calculate(account_id).await.unwrap();
        assert!(score.score > 90.0);
        assert_eq!(score.sample_count, 20);
    }

    #[tokio::test]
    async fn test_health_scorer_with_failures() {
        let scorer = HealthScorer::new(HealthScorerConfig::default());
        let account_id = Uuid::new_v4();

        // 记录失败的请求
        for _ in 0..10 {
            scorer.record_request(account_id, true, 500, false).await;
        }
        for _ in 0..10 {
            scorer.record_request(account_id, false, 0, false).await;
        }

        let score = scorer.calculate(account_id).await.unwrap();
        assert!(score.score < 100.0);
        assert!(score.factors.error_rate_score < 100.0);
    }

    #[tokio::test]
    async fn test_health_scorer_min_samples() {
        let config = HealthScorerConfig {
            min_sample_count: 10,
            ..Default::default()
        };
        let scorer = HealthScorer::new(config);
        let account_id = Uuid::new_v4();

        // 样本不足时返回满分
        for _ in 0..5 {
            scorer.record_request(account_id, false, 0, false).await;
        }

        let score = scorer.calculate(account_id).await.unwrap();
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn test_success_rate_score() {
        let scorer = HealthScorer::new(HealthScorerConfig::default());

        assert_eq!(scorer.calculate_success_rate_score(0.99), 100.0);
        assert_eq!(scorer.calculate_success_rate_score(1.0), 100.0);
        assert!(scorer.calculate_success_rate_score(0.97) > 50.0);
        assert!(scorer.calculate_success_rate_score(0.85) < 10.0);
    }

    #[test]
    fn test_latency_score() {
        let scorer = HealthScorer::new(HealthScorerConfig::default());

        assert_eq!(scorer.calculate_latency_score(500), 100.0);
        assert_eq!(scorer.calculate_latency_score(1000), 100.0);
        assert!(scorer.calculate_latency_score(2000) > 0.0);
        assert!(scorer.calculate_latency_score(2000) < 100.0);
        assert_eq!(scorer.calculate_latency_score(3000), 0.0);
    }
}
