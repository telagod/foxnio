//! 统一定价服务
//!
//! 从 model_configs 表加载定价，30s TTL 内存缓存。
//! Fallback: DB 无记录时用硬编码默认值。

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::entity::model_configs;

/// 成本明细
#[derive(Debug, Clone, Default)]
pub struct CostBreakdown {
    pub input_cost: i64,
    pub output_cost: i64,
    pub cache_read_cost: i64,
    pub cache_creation_cost: i64,
    pub total_cost: i64,
}

/// 模型定价（内存缓存条目）
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// 输入价格（每 1M tokens, USD）
    pub input_price: f64,
    /// 输出价格（每 1M tokens, USD）
    pub output_price: f64,
    /// 缓存读取价格（每 1M tokens, USD）
    pub cache_read_price: f64,
    /// 缓存创建价格（每 1M tokens, USD）
    pub cache_creation_price: f64,
}

/// 计费输入参数
#[derive(Debug, Clone, Default)]
pub struct CostInput {
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub rate_multiplier: f64,
}

const CACHE_TTL_SECS: u64 = 30;

/// 统一定价服务
pub struct PricingService {
    db: DatabaseConnection,
    /// model_name → ModelPricing
    cache: Arc<RwLock<HashMap<String, ModelPricing>>>,
    cache_loaded_at: Arc<RwLock<Option<Instant>>>,
}

impl PricingService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_loaded_at: Arc::new(RwLock::new(None)),
        }
    }

    /// 刷新缓存（如果过期）
    async fn ensure_cache(&self) {
        let needs_refresh = {
            let loaded_at = self.cache_loaded_at.read().await;
            loaded_at
                .map(|t| t.elapsed().as_secs() > CACHE_TTL_SECS)
                .unwrap_or(true)
        };

        if needs_refresh {
            if let Ok(configs) = model_configs::Entity::find()
                .filter(model_configs::Column::Enabled.eq(true))
                .all(&self.db)
                .await
            {
                let mut map = HashMap::with_capacity(configs.len() * 2);
                for config in configs {
                    let pricing = ModelPricing {
                        input_price: config.input_price,
                        output_price: config.output_price,
                        cache_read_price: config.cache_read_price.unwrap_or(0.0),
                        cache_creation_price: config.cache_creation_price.unwrap_or(0.0),
                    };
                    // 按 name 和 api_name 双键索引
                    map.insert(config.name.clone(), pricing.clone());
                    if config.api_name != config.name {
                        map.insert(config.api_name, pricing.clone());
                    }
                    // 别名也索引
                    if let Some(aliases) = config.aliases.as_ref().and_then(|v| v.as_array()) {
                        for alias in aliases {
                            if let Some(a) = alias.as_str() {
                                map.insert(a.to_string(), pricing.clone());
                            }
                        }
                    }
                }
                *self.cache.write().await = map;
                *self.cache_loaded_at.write().await = Some(Instant::now());
            }
        }
    }

    /// 获取模型定价（DB → fallback）
    pub async fn get_pricing(&self, model: &str) -> ModelPricing {
        self.ensure_cache().await;

        let cache = self.cache.read().await;
        if let Some(pricing) = cache.get(model) {
            return pricing.clone();
        }

        // 尝试模糊匹配（去掉日期后缀）
        let base_model = strip_date_suffix(model);
        if base_model != model {
            if let Some(pricing) = cache.get(base_model) {
                return pricing.clone();
            }
        }

        drop(cache);

        // Fallback: 硬编码默认值（per 1M tokens, USD）
        fallback_pricing(model)
    }

    /// 计算成本（单位：分）
    pub async fn calculate_cost(&self, input: &CostInput) -> CostBreakdown {
        let pricing = self.get_pricing(&input.model).await;
        let multiplier = if input.rate_multiplier > 0.0 {
            input.rate_multiplier
        } else {
            1.0
        };

        // USD per 1M tokens → 分 per token: price / 1_000_000 * 100
        // 简化: price / 10_000
        let input_cost =
            (input.input_tokens as f64 * pricing.input_price / 10_000.0 * multiplier) as i64;
        let output_cost =
            (input.output_tokens as f64 * pricing.output_price / 10_000.0 * multiplier) as i64;
        let cache_read_cost =
            (input.cache_read_tokens as f64 * pricing.cache_read_price / 10_000.0 * multiplier)
                as i64;
        let cache_creation_cost = (input.cache_creation_tokens as f64
            * pricing.cache_creation_price
            / 10_000.0
            * multiplier) as i64;

        let total = input_cost + output_cost + cache_read_cost + cache_creation_cost;

        CostBreakdown {
            input_cost,
            output_cost,
            cache_read_cost,
            cache_creation_cost,
            total_cost: total,
        }
    }

    /// 简化接口：兼容旧 calculate_cost 签名
    pub async fn calculate_cost_simple(
        &self,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        rate_multiplier: f64,
    ) -> i64 {
        let breakdown = self
            .calculate_cost(&CostInput {
                model: model.to_string(),
                input_tokens,
                output_tokens,
                rate_multiplier,
                ..Default::default()
            })
            .await;
        breakdown.total_cost
    }
}

/// 去掉模型名的日期后缀（如 claude-3-5-sonnet-20241022 → claude-3-5-sonnet）
fn strip_date_suffix(model: &str) -> &str {
    // 匹配 -YYYYMMDD 格式
    if model.len() > 9 {
        let suffix = &model[model.len() - 9..];
        if suffix.starts_with('-') && suffix[1..].chars().all(|c| c.is_ascii_digit()) {
            return &model[..model.len() - 9];
        }
    }
    model
}

/// 硬编码 fallback 定价（per 1M tokens, USD）
fn fallback_pricing(model: &str) -> ModelPricing {
    let m = model.to_lowercase();
    let (input, output) = if m.contains("opus") {
        (15.0, 75.0)
    } else if m.contains("sonnet") {
        (3.0, 15.0)
    } else if m.contains("haiku") {
        (0.25, 1.25)
    } else if m.contains("gpt-4o-mini") {
        (0.15, 0.60)
    } else if m.contains("gpt-4o") {
        (2.50, 10.0)
    } else if m.contains("gpt-4-turbo") || m.contains("gpt-4-0125") {
        (10.0, 30.0)
    } else if m.contains("gpt-4") {
        (30.0, 60.0)
    } else if m.contains("gpt-3.5") {
        (0.50, 1.50)
    } else if m.contains("gemini-1.5-pro") {
        (3.50, 10.50)
    } else if m.contains("gemini-1.5-flash") || m.contains("gemini-2") {
        (0.35, 1.05)
    } else if m.contains("deepseek-reasoner") {
        (0.55, 2.20)
    } else if m.contains("deepseek") {
        (0.10, 0.30)
    } else {
        (1.0, 3.0) // 默认
    };

    ModelPricing {
        input_price: input,
        output_price: output,
        cache_read_price: input * 0.1, // 默认缓存读取为输入价的 10%
        cache_creation_price: input * 1.25, // 默认缓存创建为输入价的 125%
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_date_suffix() {
        assert_eq!(
            strip_date_suffix("claude-3-5-sonnet-20241022"),
            "claude-3-5-sonnet"
        );
        assert_eq!(strip_date_suffix("gpt-4o"), "gpt-4o");
        // gpt-4-0613 只有 4 位数字，不匹配 8 位日期后缀
        assert_eq!(strip_date_suffix("gpt-4-0613"), "gpt-4-0613");
        assert_eq!(
            strip_date_suffix("gpt-4-turbo-20240409"),
            "gpt-4-turbo"
        );
    }

    #[test]
    fn test_fallback_pricing() {
        let p = fallback_pricing("claude-3-opus-20240229");
        assert_eq!(p.input_price, 15.0);
        assert_eq!(p.output_price, 75.0);

        let p = fallback_pricing("gpt-4o-mini");
        assert_eq!(p.input_price, 0.15);

        let p = fallback_pricing("unknown-model");
        assert_eq!(p.input_price, 1.0);
    }

    #[tokio::test]
    async fn test_cost_calculation_no_db() {
        // 直接测试 fallback 路径
        let pricing = fallback_pricing("claude-3-5-sonnet-20241022");
        let input = CostInput {
            model: "claude-3-5-sonnet-20241022".into(),
            input_tokens: 1000,
            output_tokens: 500,
            rate_multiplier: 1.0,
            ..Default::default()
        };

        // input: 1000 * 3.0 / 10000 = 0.3 → 0
        // output: 500 * 15.0 / 10000 = 0.75 → 0
        // 小额请求 cost 为 0 是正常的（分为单位）
        let multiplier = input.rate_multiplier;
        let input_cost =
            (input.input_tokens as f64 * pricing.input_price / 10_000.0 * multiplier) as i64;
        let output_cost =
            (input.output_tokens as f64 * pricing.output_price / 10_000.0 * multiplier) as i64;
        assert_eq!(input_cost, 0);
        assert_eq!(output_cost, 0);

        // 大额请求
        let input2 = CostInput {
            model: "claude-3-5-sonnet-20241022".into(),
            input_tokens: 100_000,
            output_tokens: 50_000,
            rate_multiplier: 1.0,
            ..Default::default()
        };
        let ic = (100_000f64 * 3.0 / 10_000.0) as i64;
        let oc = (50_000f64 * 15.0 / 10_000.0) as i64;
        assert_eq!(ic, 30);  // 30 分
        assert_eq!(oc, 75);  // 75 分
    }
}
