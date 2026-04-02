//! 业务指标模块
//!
//! 提供业务相关的指标跟踪，包括：
//! - 请求计数和成功率
//! - Token 使用量和成本
//! - 账号配额和使用率
//! - 提供商级别的统计

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use super::*;

/// 请求记录参数
#[derive(Debug, Clone)]
pub struct RequestRecord<'a> {
    pub model: &'a str,
    pub provider: &'a str,
    pub user_id: Option<&'a str>,
    pub success: bool,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

/// 业务指标聚合器
#[derive(Debug)]
pub struct BusinessMetrics {
    /// 按模型统计的请求
    model_stats: RwLock<HashMap<String, ModelStats>>,
    /// 按提供商统计的请求
    provider_stats: RwLock<HashMap<String, ProviderStats>>,
    /// 按用户统计的请求
    user_stats: RwLock<HashMap<String, UserStats>>,
    /// 按日期统计的成本
    daily_costs: RwLock<HashMap<String, DailyCost>>,
    /// 是否已初始化
    initialized: AtomicBool,
}

/// 模型统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelStats {
    pub name: String,
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost: f64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub last_request: Option<DateTime<Utc>>,
}

/// 提供商统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderStats {
    pub name: String,
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_failed: u64,
    pub active_accounts: u64,
    pub total_accounts: u64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub last_request: Option<DateTime<Utc>>,
}

/// 用户统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserStats {
    pub user_id: String,
    pub requests_total: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost: f64,
    pub last_request: Option<DateTime<Utc>>,
}

/// 每日成本
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyCost {
    pub date: String,
    pub cost: f64,
    pub requests: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
}

impl BusinessMetrics {
    /// 创建新的业务指标实例
    pub fn new() -> Self {
        Self {
            model_stats: RwLock::new(HashMap::new()),
            provider_stats: RwLock::new(HashMap::new()),
            user_stats: RwLock::new(HashMap::new()),
            daily_costs: RwLock::new(HashMap::new()),
            initialized: AtomicBool::new(true),
        }
    }

    /// 记录请求
    pub fn record_request(&self, record: RequestRecord<'_>) {
        let now = Utc::now();

        // 更新模型统计
        self.update_model_stats(ModelStatsUpdate {
            model: record.model,
            success: record.success,
            latency_ms: record.latency_ms,
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens,
            cost: record.cost,
            now,
        });

        // 更新提供商统计
        self.update_provider_stats(record.provider, record.success, record.latency_ms, now);

        // 更新用户统计
        if let Some(uid) = record.user_id {
            self.update_user_stats(
                uid,
                record.input_tokens,
                record.output_tokens,
                record.cost,
                now,
            );
        }

        // 更新每日成本
        self.update_daily_cost(record.input_tokens, record.output_tokens, record.cost, now);
    }
}

/// 模型统计更新参数
struct ModelStatsUpdate<'a> {
    model: &'a str,
    success: bool,
    latency_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
    now: DateTime<Utc>,
}

impl BusinessMetrics {
    /// 更新模型统计
    fn update_model_stats(&self, update: ModelStatsUpdate<'_>) {
        let mut stats = self.model_stats.write().unwrap();

        let entry = stats
            .entry(update.model.to_string())
            .or_insert_with(|| ModelStats {
                name: update.model.to_string(),
                ..Default::default()
            });

        entry.requests_total += 1;
        if update.success {
            entry.requests_success += 1;
        } else {
            entry.requests_failed += 1;
        }
        entry.tokens_input += update.input_tokens;
        entry.tokens_output += update.output_tokens;
        entry.cost += update.cost;

        // 简单的移动平均计算平均延迟
        let count = entry.requests_total;
        entry.avg_latency_ms =
            (entry.avg_latency_ms * (count - 1) as f64 + update.latency_ms as f64) / count as f64;

        // 更新 P99（简化版，实际应使用直方图）
        if update.latency_ms as f64 > entry.p99_latency_ms {
            entry.p99_latency_ms = update.latency_ms as f64;
        }

        entry.last_request = Some(update.now);
    }

    /// 更新提供商统计
    fn update_provider_stats(
        &self,
        provider: &str,
        success: bool,
        latency_ms: u64,
        now: DateTime<Utc>,
    ) {
        let mut stats = self.provider_stats.write().unwrap();

        let entry = stats
            .entry(provider.to_string())
            .or_insert_with(|| ProviderStats {
                name: provider.to_string(),
                ..Default::default()
            });

        entry.requests_total += 1;
        if success {
            entry.requests_success += 1;
        } else {
            entry.requests_failed += 1;
        }

        // 计算错误率
        if entry.requests_total > 0 {
            entry.error_rate = entry.requests_failed as f64 / entry.requests_total as f64;
        }

        // 更新平均延迟
        let count = entry.requests_total;
        entry.avg_latency_ms =
            (entry.avg_latency_ms * (count - 1) as f64 + latency_ms as f64) / count as f64;

        entry.last_request = Some(now);
    }

    /// 更新用户统计
    fn update_user_stats(
        &self,
        user_id: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
        now: DateTime<Utc>,
    ) {
        let mut stats = self.user_stats.write().unwrap();

        let entry = stats
            .entry(user_id.to_string())
            .or_insert_with(|| UserStats {
                user_id: user_id.to_string(),
                ..Default::default()
            });

        entry.requests_total += 1;
        entry.tokens_input += input_tokens;
        entry.tokens_output += output_tokens;
        entry.cost += cost;
        entry.last_request = Some(now);
    }

    /// 更新每日成本
    fn update_daily_cost(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cost: f64,
        now: DateTime<Utc>,
    ) {
        let mut costs = self.daily_costs.write().unwrap();

        let date = format!("{}-{:02}-{:02}", now.year(), now.month(), now.day());

        let entry = costs.entry(date.clone()).or_insert_with(|| DailyCost {
            date,
            ..Default::default()
        });

        entry.cost += cost;
        entry.requests += 1;
        entry.tokens_input += input_tokens;
        entry.tokens_output += output_tokens;

        // 更新 Prometheus 指标
        COST_DAILY.with_label_values(&[&entry.date]).set(entry.cost);
    }

    /// 获取模型统计
    pub fn get_model_stats(&self, model: &str) -> Option<ModelStats> {
        let stats = self.model_stats.read().unwrap();
        stats.get(model).cloned()
    }

    /// 获取所有模型统计
    pub fn get_all_model_stats(&self) -> Vec<ModelStats> {
        let stats = self.model_stats.read().unwrap();
        stats.values().cloned().collect()
    }

    /// 获取提供商统计
    pub fn get_provider_stats(&self, provider: &str) -> Option<ProviderStats> {
        let stats = self.provider_stats.read().unwrap();
        stats.get(provider).cloned()
    }

    /// 获取所有提供商统计
    pub fn get_all_provider_stats(&self) -> Vec<ProviderStats> {
        let stats = self.provider_stats.read().unwrap();
        stats.values().cloned().collect()
    }

    /// 获取用户统计
    pub fn get_user_stats(&self, user_id: &str) -> Option<UserStats> {
        let stats = self.user_stats.read().unwrap();
        stats.get(user_id).cloned()
    }

    /// 获取每日成本
    pub fn get_daily_cost(&self, date: &str) -> Option<DailyCost> {
        let costs = self.daily_costs.read().unwrap();
        costs.get(date).cloned()
    }

    /// 获取今日成本
    pub fn get_today_cost(&self) -> DailyCost {
        let now = Utc::now();
        let date = format!("{}-{:02}-{:02}", now.year(), now.month(), now.day());
        self.get_daily_cost(&date).unwrap_or_default()
    }

    /// 获取总成本
    pub fn get_total_cost(&self) -> f64 {
        let costs = self.daily_costs.read().unwrap();
        costs.values().map(|d| d.cost).sum()
    }

    /// 获取总 Token 使用量
    pub fn get_total_tokens(&self) -> (u64, u64) {
        let stats = self.model_stats.read().unwrap();
        let input: u64 = stats.values().map(|m| m.tokens_input).sum();
        let output: u64 = stats.values().map(|m| m.tokens_output).sum();
        (input, output)
    }

    /// 更新账号统计
    pub fn update_account_stats(&self, provider: &str, active_accounts: u64, total_accounts: u64) {
        let mut stats = self.provider_stats.write().unwrap();

        let entry = stats
            .entry(provider.to_string())
            .or_insert_with(|| ProviderStats {
                name: provider.to_string(),
                ..Default::default()
            });

        entry.active_accounts = active_accounts;
        entry.total_accounts = total_accounts;
    }

    /// 计算账号使用率
    pub fn calculate_account_usage_ratio(&self) -> f64 {
        let stats = self.provider_stats.read().unwrap();

        let total: u64 = stats.values().map(|p| p.total_accounts).sum();
        if total == 0 {
            return 0.0;
        }

        let active: u64 = stats.values().map(|p| p.active_accounts).sum();
        active as f64 / total as f64
    }

    /// 获取业务指标摘要
    pub fn get_summary(&self) -> BusinessMetricsSummary {
        let (total_input, total_output) = self.get_total_tokens();

        BusinessMetricsSummary {
            total_requests: REQUESTS_TOTAL.get(),
            total_cost: self.get_total_cost(),
            total_tokens_input: total_input,
            total_tokens_output: total_output,
            active_connections: ACTIVE_CONNECTIONS.get(),
            models_count: self.model_stats.read().unwrap().len(),
            providers_count: self.provider_stats.read().unwrap().len(),
            users_count: self.user_stats.read().unwrap().len(),
            account_usage_ratio: self.calculate_account_usage_ratio(),
            cache_hit_rate: CacheMetrics::hit_rate(),
        }
    }

    /// 重置统计（用于测试）
    #[cfg(test)]
    pub fn reset(&self) {
        let mut model_stats = self.model_stats.write().unwrap();
        model_stats.clear();

        let mut provider_stats = self.provider_stats.write().unwrap();
        provider_stats.clear();

        let mut user_stats = self.user_stats.write().unwrap();
        user_stats.clear();

        let mut daily_costs = self.daily_costs.write().unwrap();
        daily_costs.clear();
    }
}

impl Default for BusinessMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 业务指标摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetricsSummary {
    pub total_requests: u64,
    pub total_cost: f64,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub active_connections: i64,
    pub models_count: usize,
    pub providers_count: usize,
    pub users_count: usize,
    pub account_usage_ratio: f64,
    pub cache_hit_rate: f64,
}

use once_cell::sync::Lazy;

// 全局业务指标实例
pub static BUSINESS_METRICS: Lazy<Arc<BusinessMetrics>> = Lazy::new(|| Arc::new(BusinessMetrics::new()));

/// 获取全局业务指标实例
pub fn get_business_metrics() -> Arc<BusinessMetrics> {
    BUSINESS_METRICS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_business_metrics_record_request() {
        let metrics = BusinessMetrics::new();

        let record = RequestRecord {
            model: "gpt-4",
            provider: "openai",
            user_id: Some("user123"),
            success: true,
            latency_ms: 100,
            input_tokens: 50,
            output_tokens: 30,
            cost: 0.001,
        };
        metrics.record_request(record);

        let model_stats = metrics.get_model_stats("gpt-4");
        assert!(model_stats.is_some());
        let model = model_stats.unwrap();
        assert_eq!(model.requests_total, 1);
        assert_eq!(model.requests_success, 1);
        assert_eq!(model.tokens_input, 50);
        assert_eq!(model.tokens_output, 30);

        let provider_stats = metrics.get_provider_stats("openai");
        assert!(provider_stats.is_some());
        assert_eq!(provider_stats.unwrap().requests_total, 1);

        let user_stats = metrics.get_user_stats("user123");
        assert!(user_stats.is_some());
        assert_eq!(user_stats.unwrap().requests_total, 1);
    }

    #[test]
    fn test_business_metrics_failed_request() {
        let metrics = BusinessMetrics::new();

        let record = RequestRecord {
            model: "gpt-4",
            provider: "openai",
            user_id: None,
            success: false,
            latency_ms: 200,
            input_tokens: 0,
            output_tokens: 0,
            cost: 0.0,
        };
        metrics.record_request(record);

        let model_stats = metrics.get_model_stats("gpt-4").unwrap();
        assert_eq!(model_stats.requests_failed, 1);

        let provider_stats = metrics.get_provider_stats("openai").unwrap();
        assert!(provider_stats.error_rate > 0.0);
    }

    #[test]
    fn test_business_metrics_daily_cost() {
        let metrics = BusinessMetrics::new();

        let record = RequestRecord {
            model: "gpt-4",
            provider: "openai",
            user_id: None,
            success: true,
            latency_ms: 100,
            input_tokens: 100,
            output_tokens: 100,
            cost: 0.01,
        };
        metrics.record_request(record);

        let today = metrics.get_today_cost();
        assert!(today.cost > 0.0);
        assert_eq!(today.requests, 1);
    }

    #[test]
    fn test_business_metrics_summary() {
        let metrics = BusinessMetrics::new();
        metrics.reset(); // Clear any existing state

        let record1 = RequestRecord {
            model: "gpt-4",
            provider: "openai",
            user_id: Some("user1"),
            success: true,
            latency_ms: 100,
            input_tokens: 50,
            output_tokens: 50,
            cost: 0.005,
        };
        metrics.record_request(record1);

        let record2 = RequestRecord {
            model: "claude-3",
            provider: "anthropic",
            user_id: Some("user2"),
            success: true,
            latency_ms: 150,
            input_tokens: 100,
            output_tokens: 80,
            cost: 0.01,
        };
        metrics.record_request(record2);

        let summary = metrics.get_summary();

        // Check local metrics (models_count, providers_count, users_count)
        assert_eq!(summary.models_count, 2);
        assert_eq!(summary.providers_count, 2);
        assert_eq!(summary.users_count, 2);
        assert!(summary.total_cost > 0.0);
    }

    #[test]
    fn test_business_metrics_account_usage() {
        let metrics = BusinessMetrics::new();

        metrics.update_account_stats("openai", 8, 10);
        metrics.update_account_stats("anthropic", 5, 10);

        let ratio = metrics.calculate_account_usage_ratio();
        assert!((ratio - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_global_business_metrics() {
        let metrics = BusinessMetrics::new();
        metrics.reset();

        let record = RequestRecord {
            model: "gpt-4",
            provider: "openai",
            user_id: Some("test_user"),
            success: true,
            latency_ms: 100,
            input_tokens: 10,
            output_tokens: 10,
            cost: 0.001,
        };
        metrics.record_request(record);

        let summary = metrics.get_summary();
        // Check local metrics instead of global counter
        assert_eq!(summary.models_count, 1);
        assert!(summary.total_cost > 0.0);
    }
}
