//! 成本优化器模块
//!
//! 提供成本计算、预算控制和成本优化策略

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::metrics::AccountMetrics;

/// 成本配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    /// 月度预算（分）
    pub monthly_budget_cents: u64,
    /// 日预算（分）
    pub daily_budget_cents: u64,
    /// 预算警告阈值（百分比）
    pub warning_threshold_percent: u8,
    /// 预算停止阈值（百分比）
    pub stop_threshold_percent: u8,
    /// 成本优化权重（0.0-1.0）
    pub optimization_weight: f64,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            monthly_budget_cents: 100_000_00, // $100,000
            daily_budget_cents: 3_300_00,     // $3,300
            warning_threshold_percent: 80,
            stop_threshold_percent: 95,
            optimization_weight: 0.3,
        }
    }
}

/// 提供商定价配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// 提供商名称
    pub provider: String,
    /// 模型定价（每百万 token，单位：分）
    pub model_pricing: HashMap<String, ModelPricing>,
    /// 默认定价（未知模型使用）
    pub default_pricing: ModelPricing,
}

/// 模型定价
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// 输入 token 价格（每百万，单位：分）
    pub input_price_per_million: u64,
    /// 输出 token 价格（每百万，单位：分）
    pub output_price_per_million: u64,
}

impl ModelPricing {
    /// 计算成本（分）
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        let input_cost = (input_tokens * self.input_price_per_million) / 1_000_000;
        let output_cost = (output_tokens * self.output_price_per_million) / 1_000_000;
        input_cost + output_cost
    }
}

/// 预算状态
pub struct BudgetStatus {
    /// 当月已使用（分）
    pub month_used_cents: AtomicU64,
    /// 当日已使用（分）
    pub day_used_cents: AtomicU64,
    /// 当前日期（用于重置日预算）
    current_day: RwLock<u32>,
    /// 当前月份（用于重置月预算）
    current_month: RwLock<u32>,
}

impl Clone for BudgetStatus {
    fn clone(&self) -> Self {
        Self {
            month_used_cents: AtomicU64::new(self.month_used_cents.load(Ordering::SeqCst)),
            day_used_cents: AtomicU64::new(self.day_used_cents.load(Ordering::SeqCst)),
            current_day: RwLock::new(*self.current_day.blocking_read()),
            current_month: RwLock::new(*self.current_month.blocking_read()),
        }
    }
}

impl BudgetStatus {
    pub fn new() -> Self {
        Self {
            month_used_cents: AtomicU64::new(0),
            day_used_cents: AtomicU64::new(0),
            current_day: RwLock::new(Utc::now().day()),
            current_month: RwLock::new(Utc::now().month()),
        }
    }

    /// 记录消费
    pub async fn record_cost(&self, cost_cents: u64) {
        self.check_and_reset().await;
        self.month_used_cents
            .fetch_add(cost_cents, Ordering::SeqCst);
        self.day_used_cents.fetch_add(cost_cents, Ordering::SeqCst);
    }

    /// 检查并重置过期的预算周期
    async fn check_and_reset(&self) {
        let now = Utc::now();

        // 检查日期是否变化
        {
            let mut current_day = self.current_day.write().await;
            if *current_day != now.day() {
                *current_day = now.day();
                self.day_used_cents.store(0, Ordering::SeqCst);
            }
        }

        // 检查月份是否变化
        {
            let mut current_month = self.current_month.write().await;
            if *current_month != now.month() {
                *current_month = now.month();
                self.month_used_cents.store(0, Ordering::SeqCst);
            }
        }
    }

    /// 获取月度使用量
    pub fn get_month_used(&self) -> u64 {
        self.month_used_cents.load(Ordering::SeqCst)
    }

    /// 获取日使用量
    pub fn get_day_used(&self) -> u64 {
        self.day_used_cents.load(Ordering::SeqCst)
    }

    /// 获取月度使用百分比
    pub fn get_month_usage_percent(&self, budget: u64) -> u8 {
        if budget == 0 {
            return 0;
        }
        let used = self.get_month_used();
        ((used as f64 / budget as f64) * 100.0).min(100.0) as u8
    }

    /// 获取日使用百分比
    pub fn get_day_usage_percent(&self, budget: u64) -> u8 {
        if budget == 0 {
            return 0;
        }
        let used = self.get_day_used();
        ((used as f64 / budget as f64) * 100.0).min(100.0) as u8
    }
}

impl Default for BudgetStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// 成本优化器
pub struct CostOptimizer {
    config: CostConfig,
    provider_pricing: HashMap<String, ProviderPricing>,
    budget_status: Arc<BudgetStatus>,
    account_cost_history: Arc<RwLock<HashMap<Uuid, Vec<CostRecord>>>>,
}

/// 成本记录
#[derive(Debug, Clone)]
pub struct CostRecord {
    pub account_id: Uuid,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_cents: u64,
    pub timestamp: DateTime<Utc>,
}

impl CostOptimizer {
    pub fn new(config: CostConfig) -> Self {
        Self {
            config,
            provider_pricing: Self::default_provider_pricing(),
            budget_status: Arc::new(BudgetStatus::new()),
            account_cost_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 默认提供商定价
    fn default_provider_pricing() -> HashMap<String, ProviderPricing> {
        let mut pricing = HashMap::new();

        // Anthropic 定价（示例）
        pricing.insert(
            "anthropic".to_string(),
            ProviderPricing {
                provider: "anthropic".to_string(),
                model_pricing: {
                    let mut m = HashMap::new();
                    m.insert(
                        "claude-3-opus".to_string(),
                        ModelPricing {
                            input_price_per_million: 1500,  // $15
                            output_price_per_million: 7500, // $75
                        },
                    );
                    m.insert(
                        "claude-3-sonnet".to_string(),
                        ModelPricing {
                            input_price_per_million: 300,   // $3
                            output_price_per_million: 1500, // $15
                        },
                    );
                    m.insert(
                        "claude-3-haiku".to_string(),
                        ModelPricing {
                            input_price_per_million: 25,   // $0.25
                            output_price_per_million: 125, // $1.25
                        },
                    );
                    m
                },
                default_pricing: ModelPricing {
                    input_price_per_million: 500,
                    output_price_per_million: 1500,
                },
            },
        );

        // OpenAI 定价（示例）
        pricing.insert(
            "openai".to_string(),
            ProviderPricing {
                provider: "openai".to_string(),
                model_pricing: {
                    let mut m = HashMap::new();
                    m.insert(
                        "gpt-4-turbo".to_string(),
                        ModelPricing {
                            input_price_per_million: 1000,  // $10
                            output_price_per_million: 3000, // $30
                        },
                    );
                    m.insert(
                        "gpt-4".to_string(),
                        ModelPricing {
                            input_price_per_million: 3000,  // $30
                            output_price_per_million: 6000, // $60
                        },
                    );
                    m.insert(
                        "gpt-3.5-turbo".to_string(),
                        ModelPricing {
                            input_price_per_million: 50,   // $0.50
                            output_price_per_million: 150, // $1.50
                        },
                    );
                    m
                },
                default_pricing: ModelPricing {
                    input_price_per_million: 500,
                    output_price_per_million: 1500,
                },
            },
        );

        // Gemini 定价（示例）
        pricing.insert(
            "gemini".to_string(),
            ProviderPricing {
                provider: "gemini".to_string(),
                model_pricing: {
                    let mut m = HashMap::new();
                    m.insert(
                        "gemini-pro".to_string(),
                        ModelPricing {
                            input_price_per_million: 125,  // $1.25
                            output_price_per_million: 375, // $3.75
                        },
                    );
                    m.insert(
                        "gemini-ultra".to_string(),
                        ModelPricing {
                            input_price_per_million: 500,   // $5
                            output_price_per_million: 1500, // $15
                        },
                    );
                    m
                },
                default_pricing: ModelPricing {
                    input_price_per_million: 250,
                    output_price_per_million: 750,
                },
            },
        );

        pricing
    }

    /// 计算请求成本
    pub fn calculate_cost(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> u64 {
        if let Some(pricing) = self.provider_pricing.get(provider) {
            let model_pricing = pricing
                .model_pricing
                .get(model)
                .unwrap_or(&pricing.default_pricing);
            model_pricing.calculate_cost(input_tokens, output_tokens)
        } else {
            // 未知提供商使用默认定价
            ModelPricing {
                input_price_per_million: 500,
                output_price_per_million: 1500,
            }
            .calculate_cost(input_tokens, output_tokens)
        }
    }

    /// 记录消费
    pub async fn record_usage(
        &self,
        account_id: Uuid,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> u64 {
        let cost = self.calculate_cost(provider, model, input_tokens, output_tokens);

        // 更新预算状态
        self.budget_status.record_cost(cost).await;

        // 记录历史
        let record = CostRecord {
            account_id,
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost_cents: cost,
            timestamp: Utc::now(),
        };

        let mut history = self.account_cost_history.write().await;
        history
            .entry(account_id)
            .or_insert_with(Vec::new)
            .push(record);

        cost
    }

    /// 检查是否在预算内
    pub fn is_within_budget(&self) -> bool {
        let day_percent = self
            .budget_status
            .get_day_usage_percent(self.config.daily_budget_cents);
        let month_percent = self
            .budget_status
            .get_month_usage_percent(self.config.monthly_budget_cents);

        day_percent < self.config.stop_threshold_percent
            && month_percent < self.config.stop_threshold_percent
    }

    /// 检查是否需要警告
    pub fn needs_warning(&self) -> bool {
        let day_percent = self
            .budget_status
            .get_day_usage_percent(self.config.daily_budget_cents);
        let month_percent = self
            .budget_status
            .get_month_usage_percent(self.config.monthly_budget_cents);

        day_percent >= self.config.warning_threshold_percent
            || month_percent >= self.config.warning_threshold_percent
    }

    /// 获取账号成本分数（越低越好）
    pub async fn get_account_cost_score(&self, account_id: Uuid, _metrics: &AccountMetrics) -> f64 {
        let history = self.account_cost_history.read().await;

        if let Some(records) = history.get(&account_id) {
            if records.is_empty() {
                return 0.5; // 无历史数据，中等分数
            }

            // 计算最近 24 小时的平均成本
            let now = Utc::now();
            let recent: Vec<_> = records
                .iter()
                .filter(|r| (now - r.timestamp).num_hours() < 24)
                .collect();

            if recent.is_empty() {
                return 0.5;
            }

            let total_cost: u64 = recent.iter().map(|r| r.cost_cents).sum();
            let avg_cost = total_cost / recent.len() as u64;

            // 成本越低，分数越低（越好）
            // 使用对数缩放
            (avg_cost as f64).ln() / 10.0
        } else {
            0.5
        }
    }

    /// 获取预算状态摘要
    pub fn get_budget_summary(&self) -> BudgetSummary {
        BudgetSummary {
            month_used_cents: self.budget_status.get_month_used(),
            month_budget_cents: self.config.monthly_budget_cents,
            month_usage_percent: self
                .budget_status
                .get_month_usage_percent(self.config.monthly_budget_cents),
            day_used_cents: self.budget_status.get_day_used(),
            day_budget_cents: self.config.daily_budget_cents,
            day_usage_percent: self
                .budget_status
                .get_day_usage_percent(self.config.daily_budget_cents),
            is_within_budget: self.is_within_budget(),
            needs_warning: self.needs_warning(),
        }
    }

    /// 获取账号成本历史
    pub async fn get_account_cost_history(&self, account_id: Uuid) -> Vec<CostRecord> {
        let history = self.account_cost_history.read().await;
        history.get(&account_id).cloned().unwrap_or_default()
    }

    /// 清理旧记录（保留最近 30 天）
    pub async fn cleanup_old_records(&self) {
        let mut history = self.account_cost_history.write().await;
        let cutoff = Utc::now() - chrono::Duration::days(30);

        for records in history.values_mut() {
            records.retain(|r| r.timestamp > cutoff);
        }
    }

    /// 获取优化建议
    pub async fn get_optimization_suggestions(&self) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();
        let history = self.account_cost_history.read().await;

        // 分析各账号的成本效率
        for (account_id, records) in history.iter() {
            if records.len() < 10 {
                continue;
            }

            let total_cost: u64 = records.iter().map(|r| r.cost_cents).sum();
            let total_tokens: u64 = records
                .iter()
                .map(|r| r.input_tokens + r.output_tokens)
                .sum();

            if total_tokens == 0 {
                continue;
            }

            let cost_per_million = (total_cost as f64 / total_tokens as f64) * 1_000_000.0;

            // 如果成本过高，建议切换
            if cost_per_million > 2000.0 {
                suggestions.push(OptimizationSuggestion {
                    account_id: *account_id,
                    suggestion_type: SuggestionType::HighCost,
                    current_value: cost_per_million,
                    message: format!(
                        "账号成本较高（${:.2}/M tokens），建议切换到更经济的账号或模型",
                        cost_per_million / 100.0
                    ),
                });
            }
        }

        // 检查预算使用情况
        let summary = self.get_budget_summary();
        if summary.needs_warning {
            suggestions.push(OptimizationSuggestion {
                account_id: Uuid::nil(),
                suggestion_type: SuggestionType::BudgetWarning,
                current_value: summary.month_usage_percent as f64,
                message: format!(
                    "月度预算已使用 {}%，请注意控制使用量",
                    summary.month_usage_percent
                ),
            });
        }

        suggestions
    }
}

impl Default for CostOptimizer {
    fn default() -> Self {
        Self::new(CostConfig::default())
    }
}

/// 预算摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub month_used_cents: u64,
    pub month_budget_cents: u64,
    pub month_usage_percent: u8,
    pub day_used_cents: u64,
    pub day_budget_cents: u64,
    pub day_usage_percent: u8,
    pub is_within_budget: bool,
    pub needs_warning: bool,
}

/// 优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    pub account_id: Uuid,
    pub suggestion_type: SuggestionType,
    pub current_value: f64,
    pub message: String,
}

/// 建议类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    HighCost,
    BudgetWarning,
    LowEfficiency,
    ModelMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing() {
        let pricing = ModelPricing {
            input_price_per_million: 1000,
            output_price_per_million: 2000,
        };

        // 100K 输入 + 50K 输出
        let cost = pricing.calculate_cost(100_000, 50_000);
        // 应该是 (100K * 1000 / 1M) + (50K * 2000 / 1M) = 100 + 100 = 200
        assert_eq!(cost, 200);
    }

    #[tokio::test]
    async fn test_budget_status() {
        let status = BudgetStatus::new();

        status.record_cost(1000).await;
        assert_eq!(status.get_month_used(), 1000);
        assert_eq!(status.get_day_used(), 1000);

        let percent = status.get_month_usage_percent(10000);
        assert_eq!(percent, 10);
    }

    #[tokio::test]
    async fn test_cost_optimizer() {
        let optimizer = CostOptimizer::new(CostConfig::default());

        // 计算成本
        let cost = optimizer.calculate_cost("anthropic", "claude-3-opus", 100_000, 50_000);
        assert!(cost > 0);

        // 记录使用
        let account_id = Uuid::new_v4();
        let recorded_cost = optimizer
            .record_usage(account_id, "anthropic", "claude-3-opus", 100_000, 50_000)
            .await;

        assert_eq!(cost, recorded_cost);
        assert!(optimizer.is_within_budget());
    }

    #[tokio::test]
    async fn test_budget_summary() {
        let optimizer = CostOptimizer::new(CostConfig::default());

        let summary = optimizer.get_budget_summary();
        assert!(summary.is_within_budget);
        assert!(!summary.needs_warning);
    }

    #[tokio::test]
    async fn test_cost_score() {
        let optimizer = CostOptimizer::new(CostConfig::default());
        let metrics = AccountMetrics::new();
        let account_id = Uuid::new_v4();

        // 没有历史记录
        let score = optimizer.get_account_cost_score(account_id, &metrics).await;
        assert_eq!(score, 0.5);

        // 添加一些记录
        optimizer
            .record_usage(account_id, "anthropic", "claude-3-haiku", 1000, 500)
            .await;

        let score = optimizer.get_account_cost_score(account_id, &metrics).await;
        assert!(score >= 0.0);
    }

    #[tokio::test]
    async fn test_optimization_suggestions() {
        let optimizer = CostOptimizer::new(CostConfig::default());

        let suggestions = optimizer.get_optimization_suggestions().await;
        // 没有历史数据，应该没有建议
        assert!(suggestions.is_empty());
    }
}
