//! Cost Optimizer Service
//!
//! 成本优化建议服务，分析用户使用模式并提供优化建议
//!
//! # 功能
//! - 使用分析：按模型统计请求数、tokens、成本等
//! - 模式识别：识别高频聊天、长上下文、批量请求等使用模式
//! - 异常检测：检测成本突增、失败激增、配额超支等异常
//! - 优化建议：生成模型选择、缓存、请求优化等建议
//! - 成本报告：生成详细的成本分析报告

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::entity::model_configs;
use crate::entity::quota_usage_history;

/// 成本优化服务
pub struct CostOptimizerService {
    db: DatabaseConnection,
}

/// 使用分析结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageAnalysis {
    pub user_id: i64,
    pub period: TimePeriod,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub model_breakdown: Vec<ModelUsage>,
    pub patterns: Vec<UsagePattern>,
    pub anomalies: Vec<Anomaly>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimePeriod {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelUsage {
    pub model_name: String,
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time_ms: f64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsagePattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub frequency: f64, // 0.0 - 1.0
    pub impact: Impact,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PatternType {
    HighVolumeChat,    // 高频聊天
    LongContextUsage,  // 长上下文使用
    CodeGeneration,    // 代码生成
    BatchRequests,     // 批量请求
    PeakHourUsage,     // 高峰时段使用
    RepetitiveQueries, // 重复查询
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Impact {
    pub cost_impact: f64,       // 成本影响 (USD)
    pub efficiency_impact: f64, // 效率影响 (-1.0 to 1.0)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub detected_at: DateTime<Utc>,
    pub severity: Severity,
    pub description: String,
    pub affected_models: Vec<String>,
    pub estimated_extra_cost: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AnomalyType {
    UnexpectedHighCost,    // 意外高成本
    UnusualTrafficPattern, // 异常流量模式
    FailedRequestSpike,    // 失败请求激增
    SlowResponsePattern,   // 慢响应模式
    QuotaOverrun,          // 配额超支
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// 优化建议
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptimizationRecommendation {
    pub recommendation_id: String,
    pub category: RecommendationCategory,
    pub title: String,
    pub description: String,
    pub potential_savings: f64, // 预计节省 (USD/月)
    pub effort: EffortLevel,
    pub priority: Priority,
    pub action_items: Vec<ActionItem>,
    pub affected_models: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RecommendationCategory {
    ModelSelection,      // 模型选择优化
    Caching,             // 缓存策略
    RequestOptimization, // 请求优化
    CostAllocation,      // 成本分摊
    QuotaManagement,     // 配额管理
    ProviderSwitch,      // 服务商切换
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum EffortLevel {
    Low,    // 简单修改，立即生效
    Medium, // 需要一定工作量
    High,   // 需要重构或重新设计
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActionItem {
    pub action: String,
    pub estimated_impact: f64,
    pub implementation_time: std::time::Duration,
}

/// 成本报告
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CostReport {
    pub user_id: i64,
    pub period: TimePeriod,
    pub summary: CostSummary,
    pub breakdown: Vec<CostBreakdownItem>,
    pub trends: Vec<CostTrend>,
    pub recommendations: Vec<OptimizationRecommendation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CostSummary {
    pub total_cost: f64,
    pub total_tokens: i64,
    pub total_requests: i64,
    pub avg_cost_per_request: f64,
    pub avg_cost_per_token: f64,
    pub cost_change_from_previous: f64, // 百分比变化
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CostBreakdownItem {
    pub category: String,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CostTrend {
    pub date: DateTime<Utc>,
    pub cost: f64,
    pub tokens: i64,
    pub requests: i64,
}

/// 使用记录（内部）
#[derive(Debug, Clone)]
struct UsageRecord {
    timestamp: DateTime<Utc>,
    model: String,
    provider: String,
    tokens_in: i64,
    tokens_out: i64,
    cost: f64,
    response_time_ms: f64,
    success: bool,
    prompt_hash: Option<String>, // 用于检测重复查询
}

/// 模型配置缓存
#[derive(Debug, Clone)]
struct ModelConfigCache {
    name: String,
    provider: String,
    input_price: f64,
    output_price: f64,
    context_window: i32,
    capabilities: Vec<String>,
}

impl CostOptimizerService {
    /// 创建新的成本优化服务
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 分析用户使用情况
    ///
    /// 从数据库查询使用历史，按模型分组统计，识别模式和异常
    pub async fn analyze_usage(&self, user_id: i64, period: TimePeriod) -> Result<UsageAnalysis> {
        info!(
            "分析用户 {} 使用情况，时间范围: {:?} - {:?}",
            user_id, period.start, period.end
        );

        // 1. 从数据库获取使用数据
        let usage_data = self.fetch_usage_data(user_id, &period).await?;

        // 2. 按模型分组统计
        let model_breakdown = self.group_by_model(&usage_data)?;

        // 3. 识别使用模式
        let patterns = self.identify_patterns(&usage_data)?;

        // 4. 检测异常
        let anomalies = self.detect_anomalies(&usage_data, &model_breakdown)?;

        // 5. 计算总成本和总 tokens
        let total_cost = model_breakdown.iter().map(|m| m.total_cost).sum();
        let total_tokens = model_breakdown.iter().map(|m| m.total_tokens).sum();

        Ok(UsageAnalysis {
            user_id,
            period,
            total_cost,
            total_tokens,
            model_breakdown,
            patterns,
            anomalies,
        })
    }

    /// 从数据库查询使用记录
    ///
    /// 从 quota_usage_history 表查询指定时间范围内的使用记录
    async fn fetch_usage_data(
        &self,
        user_id: i64,
        period: &TimePeriod,
    ) -> Result<Vec<UsageRecord>> {
        use quota_usage_history::Column as Q;

        // 查询使用历史记录
        let records = quota_usage_history::Entity::find()
            .filter(Q::UserId.eq(user_id))
            .filter(Q::CreatedAt.gte(period.start))
            .filter(Q::CreatedAt.lte(period.end))
            .order_by_asc(Q::CreatedAt)
            .all(&self.db)
            .await?;

        info!("查询到 {} 条使用记录", records.len());

        // 转换为内部使用记录格式
        let usage_data: Vec<UsageRecord> = records
            .iter()
            .map(|r| {
                let tokens_in = r.tokens_in.unwrap_or(0) as i64;
                let tokens_out = r.tokens_out.unwrap_or(0) as i64;
                // 将 Decimal 转换为 f64
                let cost = r.amount.to_string().parse::<f64>().unwrap_or(0.0);

                // 从 metadata 提取响应时间和成功状态
                let (response_time_ms, success, prompt_hash) = r
                    .metadata
                    .as_ref()
                    .and_then(|m| {
                        let obj = m.as_object()?;
                        let response_time = obj.get("response_time_ms")?.as_f64().unwrap_or(0.0);
                        let success = obj.get("success")?.as_bool().unwrap_or(true);
                        let prompt_hash = obj
                            .get("prompt_hash")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        Some((response_time, success, prompt_hash))
                    })
                    .unwrap_or((0.0, true, None));

                UsageRecord {
                    timestamp: r.created_at.with_timezone(&Utc),
                    model: r.model.clone(),
                    provider: r
                        .request_type
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    tokens_in,
                    tokens_out,
                    cost,
                    response_time_ms,
                    success,
                    prompt_hash,
                }
            })
            .collect();

        Ok(usage_data)
    }

    /// 按模型分组统计使用量
    ///
    /// 计算每个模型的请求数、tokens、成本、平均响应时间和成功率
    fn group_by_model(&self, data: &[UsageRecord]) -> Result<Vec<ModelUsage>> {
        // 按模型分组
        let mut model_stats: HashMap<String, ModelStatsAccumulator> = HashMap::new();

        for record in data {
            let stats = model_stats
                .entry(record.model.clone())
                .or_insert_with(ModelStatsAccumulator::new);

            stats.request_count += 1;
            stats.total_tokens += record.tokens_in + record.tokens_out;
            stats.total_cost += record.cost;
            stats.total_response_time += record.response_time_ms;
            if record.success {
                stats.success_count += 1;
            }
            stats.provider = record.provider.clone();
        }

        // 转换为 ModelUsage 列表
        let mut result: Vec<ModelUsage> = model_stats
            .into_iter()
            .map(|(model_name, stats)| {
                let avg_response_time = if stats.request_count > 0 {
                    stats.total_response_time / stats.request_count as f64
                } else {
                    0.0
                };

                let success_rate = if stats.request_count > 0 {
                    stats.success_count as f64 / stats.request_count as f64
                } else {
                    1.0
                };

                ModelUsage {
                    model_name,
                    provider: stats.provider,
                    request_count: stats.request_count,
                    total_tokens: stats.total_tokens,
                    total_cost: stats.total_cost,
                    avg_response_time_ms: avg_response_time,
                    success_rate,
                }
            })
            .collect();

        // 按成本降序排序
        result.sort_by(|a, b| b.total_cost.partial_cmp(&a.total_cost).unwrap());

        Ok(result)
    }

    /// 识别使用模式
    ///
    /// 分析使用数据，识别以下模式：
    /// - HighVolumeChat: 短时间内大量小请求
    /// - LongContextUsage: 高 tokens/请求比
    /// - CodeGeneration: 特定模型高频使用
    /// - BatchRequests: 固定间隔大批量
    /// - PeakHourUsage: 特定时段集中
    /// - RepetitiveQueries: 相似提示词重复
    fn identify_patterns(&self, data: &[UsageRecord]) -> Result<Vec<UsagePattern>> {
        let mut patterns = Vec::new();

        if data.is_empty() {
            return Ok(patterns);
        }

        let total_requests = data.len() as i64;
        let total_tokens: i64 = data.iter().map(|r| r.tokens_in + r.tokens_out).sum();
        let total_cost: f64 = data.iter().map(|r| r.cost).sum();

        // 1. 检测高频聊天模式（短时间内大量小请求）
        if let Some(pattern) = self.detect_high_volume_chat(data, total_requests) {
            patterns.push(pattern);
        }

        // 2. 检测长上下文使用模式（高 tokens/请求比）
        if let Some(pattern) = self.detect_long_context_usage(data, total_tokens, total_requests) {
            patterns.push(pattern);
        }

        // 3. 检测代码生成模式（特定模型高频使用）
        if let Some(pattern) = self.detect_code_generation(data) {
            patterns.push(pattern);
        }

        // 4. 检测批量请求模式（固定间隔大批量）
        if let Some(pattern) = self.detect_batch_requests(data) {
            patterns.push(pattern);
        }

        // 5. 检测高峰时段使用模式
        if let Some(pattern) = self.detect_peak_hour_usage(data) {
            patterns.push(pattern);
        }

        // 6. 检测重复查询模式
        if let Some(pattern) = self.detect_repetitive_queries(data, total_cost) {
            patterns.push(pattern);
        }

        Ok(patterns)
    }

    /// 检测高频聊天模式
    ///
    /// 条件：平均每个请求 tokens < 500，且请求数 > 100
    fn detect_high_volume_chat(
        &self,
        data: &[UsageRecord],
        total_requests: i64,
    ) -> Option<UsagePattern> {
        if total_requests < 100 {
            return None;
        }

        let total_tokens: i64 = data.iter().map(|r| r.tokens_in + r.tokens_out).sum();
        let avg_tokens_per_request = total_tokens as f64 / total_requests as f64;

        if avg_tokens_per_request < 500.0 {
            let total_cost: f64 = data.iter().map(|r| r.cost).sum();
            let frequency = (total_requests as f64 / 100.0).min(1.0);

            Some(UsagePattern {
                pattern_type: PatternType::HighVolumeChat,
                description: format!(
                    "检测到高频聊天模式：平均每请求 {:.0} tokens，共 {} 次请求。考虑使用更经济的模型。",
                    avg_tokens_per_request, total_requests
                ),
                frequency,
                impact: Impact {
                    cost_impact: total_cost * 0.3, // 假设可节省 30%
                    efficiency_impact: 0.5,
                },
            })
        } else {
            None
        }
    }

    /// 检测长上下文使用模式
    ///
    /// 条件：平均每个请求 tokens > 8000
    fn detect_long_context_usage(
        &self,
        data: &[UsageRecord],
        total_tokens: i64,
        total_requests: i64,
    ) -> Option<UsagePattern> {
        if total_requests == 0 {
            return None;
        }

        let avg_tokens_per_request = total_tokens as f64 / total_requests as f64;

        if avg_tokens_per_request > 8000.0 {
            let total_cost: f64 = data.iter().map(|r| r.cost).sum();
            let frequency = (avg_tokens_per_request / 16000.0).min(1.0);

            Some(UsagePattern {
                pattern_type: PatternType::LongContextUsage,
                description: format!(
                    "检测到长上下文使用模式：平均每请求 {:.0} tokens。建议优化上下文或使用支持长上下文的模型。",
                    avg_tokens_per_request
                ),
                frequency,
                impact: Impact {
                    cost_impact: total_cost * 0.2,
                    efficiency_impact: -0.3, // 可能影响效率
                },
            })
        } else {
            None
        }
    }

    /// 检测代码生成模式
    ///
    /// 条件：使用代码模型（如 claude、gpt-4、deepseek-coder）的请求占比 > 50%
    fn detect_code_generation(&self, data: &[UsageRecord]) -> Option<UsagePattern> {
        let code_models = [
            "claude",
            "gpt-4",
            "deepseek-coder",
            "codellama",
            "codeqwen",
            "starcoder",
        ];

        let mut code_request_count = 0i64;
        let mut code_cost = 0.0f64;
        let total_requests = data.len() as i64;

        for record in data {
            let is_code_model = code_models
                .iter()
                .any(|m| record.model.to_lowercase().contains(m));

            if is_code_model {
                code_request_count += 1;
                code_cost += record.cost;
            }
        }

        let code_ratio = code_request_count as f64 / total_requests as f64;

        if code_ratio > 0.5 {
            Some(UsagePattern {
                pattern_type: PatternType::CodeGeneration,
                description: format!(
                    "检测到代码生成模式：{:.0}% 的请求使用代码模型。可考虑使用专门的代码助手模型。",
                    code_ratio * 100.0
                ),
                frequency: code_ratio,
                impact: Impact {
                    cost_impact: code_cost * 0.15,
                    efficiency_impact: 0.4,
                },
            })
        } else {
            None
        }
    }

    /// 检测批量请求模式
    ///
    /// 条件：存在明显的时间间隔规律（每隔固定时间有大量请求）
    fn detect_batch_requests(&self, data: &[UsageRecord]) -> Option<UsagePattern> {
        if data.len() < 20 {
            return None;
        }

        // 计算请求时间间隔
        let mut intervals: Vec<i64> = Vec::new();
        for i in 1..data.len() {
            let interval = (data[i].timestamp - data[i - 1].timestamp).num_seconds();
            intervals.push(interval);
        }

        // 检查是否有规律性间隔（标准差小）
        if intervals.is_empty() {
            return None;
        }

        let mean = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;
        let variance: f64 = intervals
            .iter()
            .map(|i| (*i as f64 - mean).powi(2))
            .sum::<f64>()
            / intervals.len() as f64;
        let std_dev = variance.sqrt();

        // 如果标准差小于平均值的 20%，认为有规律
        if std_dev < mean * 0.2 && mean > 60.0 && mean < 3600.0 {
            let total_cost: f64 = data.iter().map(|r| r.cost).sum();

            Some(UsagePattern {
                pattern_type: PatternType::BatchRequests,
                description: format!(
                    "检测到批量请求模式：平均间隔 {:.0} 秒。建议使用批量 API 获得更优惠价格。",
                    mean
                ),
                frequency: 0.8,
                impact: Impact {
                    cost_impact: total_cost * 0.25,
                    efficiency_impact: 0.6,
                },
            })
        } else {
            None
        }
    }

    /// 检测高峰时段使用模式
    ///
    /// 条件：某个小时段的请求量占总量的 40% 以上
    fn detect_peak_hour_usage(&self, data: &[UsageRecord]) -> Option<UsagePattern> {
        if data.len() < 50 {
            return None;
        }

        // 统计每小时的请求数
        let mut hour_counts: HashMap<u32, i64> = HashMap::new();
        for record in data {
            let hour = record.timestamp.naive_utc().hour();
            *hour_counts.entry(hour).or_insert(0) += 1;
        }

        // 找出高峰时段
        let total_requests = data.len() as i64;
        let max_hour = hour_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(h, _)| *h)?;
        let max_count = hour_counts.get(&max_hour).copied().unwrap_or(0);

        let peak_ratio = max_count as f64 / total_requests as f64;

        if peak_ratio > 0.4 {
            let peak_cost: f64 = data
                .iter()
                .filter(|r| r.timestamp.naive_utc().hour() == max_hour)
                .map(|r| r.cost)
                .sum();

            Some(UsagePattern {
                pattern_type: PatternType::PeakHourUsage,
                description: format!(
                    "检测到高峰时段使用模式：{}:00 时段占 {:.0}% 的请求。建议分散使用或预约批量处理。",
                    max_hour, peak_ratio * 100.0
                ),
                frequency: peak_ratio,
                impact: Impact {
                    cost_impact: peak_cost * 0.1,
                    efficiency_impact: -0.2,
                },
            })
        } else {
            None
        }
    }

    /// 检测重复查询模式
    ///
    /// 条件：相似提示词（相同的 prompt_hash）重复出现
    fn detect_repetitive_queries(
        &self,
        data: &[UsageRecord],
        total_cost: f64,
    ) -> Option<UsagePattern> {
        // 统计每个 prompt_hash 的出现次数
        let mut hash_counts: HashMap<String, i64> = HashMap::new();
        let mut repetitive_cost = 0.0f64;

        for record in data {
            if let Some(hash) = &record.prompt_hash {
                let count = hash_counts.entry(hash.clone()).or_insert(0);
                *count += 1;
                if *count > 1 {
                    repetitive_cost += record.cost;
                }
            }
        }

        // 计算重复查询的比例
        let total_with_hash: i64 = hash_counts.values().sum();
        let repetitive_count: i64 = hash_counts
            .values()
            .filter(|&&c| c > 1)
            .map(|&c| c - 1)
            .sum();

        if total_with_hash > 0 {
            let repetitive_ratio = repetitive_count as f64 / total_with_hash as f64;

            if repetitive_ratio > 0.1 {
                return Some(UsagePattern {
                    pattern_type: PatternType::RepetitiveQueries,
                    description: format!(
                        "检测到重复查询模式：{:.0}% 的查询重复。强烈建议启用响应缓存。",
                        repetitive_ratio * 100.0
                    ),
                    frequency: repetitive_ratio,
                    impact: Impact {
                        cost_impact: repetitive_cost,
                        efficiency_impact: 0.7,
                    },
                });
            }
        }

        None
    }

    /// 检测异常情况
    ///
    /// 分析使用数据，检测以下异常：
    /// - UnexpectedHighCost: 成本突增 > 2x 平均值
    /// - UnusualTrafficPattern: 流量模式异常
    /// - FailedRequestSpike: 失败率 > 10%
    /// - SlowResponsePattern: 平均响应 > 5s
    /// - QuotaOverrun: 超出配额
    fn detect_anomalies(
        &self,
        data: &[UsageRecord],
        model_breakdown: &[ModelUsage],
    ) -> Result<Vec<Anomaly>> {
        let mut anomalies = Vec::new();
        let now = Utc::now();

        if data.is_empty() {
            return Ok(anomalies);
        }

        // 1. 检测失败请求激增
        for model_usage in model_breakdown {
            if model_usage.success_rate < 0.9 {
                let failed_count =
                    (model_usage.request_count as f64 * (1.0 - model_usage.success_rate)) as i64;
                let estimated_extra_cost =
                    model_usage.total_cost * (1.0 - model_usage.success_rate);

                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::FailedRequestSpike,
                    detected_at: now,
                    severity: if model_usage.success_rate < 0.7 {
                        Severity::Critical
                    } else if model_usage.success_rate < 0.8 {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    description: format!(
                        "模型 {} 失败率 {:.1}% ({} 次失败)，建议检查账户状态或请求参数",
                        model_usage.model_name,
                        (1.0 - model_usage.success_rate) * 100.0,
                        failed_count
                    ),
                    affected_models: vec![model_usage.model_name.clone()],
                    estimated_extra_cost,
                });
            }
        }

        // 2. 检测慢响应模式
        for model_usage in model_breakdown {
            if model_usage.avg_response_time_ms > 5000.0 {
                anomalies.push(Anomaly {
                    anomaly_type: AnomalyType::SlowResponsePattern,
                    detected_at: now,
                    severity: if model_usage.avg_response_time_ms > 10000.0 {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    description: format!(
                        "模型 {} 平均响应时间 {:.1}s，可能影响用户体验",
                        model_usage.model_name,
                        model_usage.avg_response_time_ms / 1000.0
                    ),
                    affected_models: vec![model_usage.model_name.clone()],
                    estimated_extra_cost: 0.0, // 慢响应不直接产生额外成本
                });
            }
        }

        // 3. 检测成本突增
        if let Some(anomaly) = self.detect_cost_spike(data) {
            anomalies.push(anomaly);
        }

        // 4. 检测流量模式异常（例如：凌晨时段异常活跃）
        if let Some(anomaly) = self.detect_traffic_anomaly(data) {
            anomalies.push(anomaly);
        }

        // 5. 检测配额超支（这里简化处理，实际需要查询用户配额）
        if let Some(anomaly) = self.detect_quota_overrun(data, model_breakdown) {
            anomalies.push(anomaly);
        }

        Ok(anomalies)
    }

    /// 检测成本突增
    ///
    /// 条件：某天成本超过平均成本的 2 倍
    fn detect_cost_spike(&self, data: &[UsageRecord]) -> Option<Anomaly> {
        // 按天分组计算成本
        let mut daily_costs: HashMap<chrono::NaiveDate, f64> = HashMap::new();
        for record in data {
            let date = record.timestamp.date_naive();
            *daily_costs.entry(date).or_insert(0.0) += record.cost;
        }

        if daily_costs.len() < 3 {
            return None;
        }

        let costs: Vec<f64> = daily_costs.values().copied().collect();
        let avg_cost = costs.iter().sum::<f64>() / costs.len() as f64;

        // 找出超过平均值 2 倍的日期
        for (date, &cost) in &daily_costs {
            if cost > avg_cost * 2.0 {
                return Some(Anomaly {
                    anomaly_type: AnomalyType::UnexpectedHighCost,
                    detected_at: Utc::now(),
                    severity: if cost > avg_cost * 5.0 {
                        Severity::Critical
                    } else if cost > avg_cost * 3.0 {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    description: format!(
                        "{} 成本突增 ${:.2}，是日均值 ${:.2} 的 {:.1} 倍",
                        date,
                        cost,
                        avg_cost,
                        cost / avg_cost
                    ),
                    affected_models: vec![],
                    estimated_extra_cost: cost - avg_cost,
                });
            }
        }

        None
    }

    /// 检测流量模式异常
    ///
    /// 条件：凌晨时段（0-6点）请求量超过全天的 30%
    fn detect_traffic_anomaly(&self, data: &[UsageRecord]) -> Option<Anomaly> {
        let total_requests = data.len() as i64;
        let night_requests = data
            .iter()
            .filter(|r| r.timestamp.naive_utc().hour() < 6)
            .count() as i64;

        let night_ratio = night_requests as f64 / total_requests as f64;

        if night_ratio > 0.3 && total_requests > 50 {
            Some(Anomaly {
                anomaly_type: AnomalyType::UnusualTrafficPattern,
                detected_at: Utc::now(),
                severity: Severity::Low,
                description: format!(
                    "凌晨时段（0-6点）请求占比 {:.0}%，可能存在自动化脚本或异常使用",
                    night_ratio * 100.0
                ),
                affected_models: vec![],
                estimated_extra_cost: 0.0,
            })
        } else {
            None
        }
    }

    /// 检测配额超支
    ///
    /// 条件：简化处理 - 如果总成本超过预设阈值（$100）
    fn detect_quota_overrun(
        &self,
        data: &[UsageRecord],
        model_breakdown: &[ModelUsage],
    ) -> Option<Anomaly> {
        let total_cost: f64 = data.iter().map(|r| r.cost).sum();

        // 这里使用固定阈值，实际应该查询用户的配额设置
        let quota_threshold = 100.0;

        if total_cost > quota_threshold {
            let affected_models: Vec<String> = model_breakdown
                .iter()
                .map(|m| m.model_name.clone())
                .collect();

            Some(Anomaly {
                anomaly_type: AnomalyType::QuotaOverrun,
                detected_at: Utc::now(),
                severity: if total_cost > quota_threshold * 2.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!(
                    "总成本 ${:.2} 超出预设配额 ${:.2}",
                    total_cost, quota_threshold
                ),
                affected_models,
                estimated_extra_cost: total_cost - quota_threshold,
            })
        } else {
            None
        }
    }

    /// 生成优化建议
    ///
    /// 基于使用分析结果，生成针对性的优化建议
    pub async fn generate_recommendations(
        &self,
        user_id: i64,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 获取最近 30 天的使用数据
        let period = TimePeriod {
            start: Utc::now() - Duration::days(30),
            end: Utc::now(),
        };

        let analysis = self.analyze_usage(user_id, period).await?;

        // 1. 模型选择优化建议
        recommendations.extend(self.recommend_model_selection(&analysis).await?);

        // 2. 缓存策略建议
        recommendations.extend(self.recommend_caching(&analysis).await?);

        // 3. 请求优化建议
        recommendations.extend(self.recommend_request_optimization(&analysis).await?);

        // 4. 配额管理建议
        recommendations.extend(self.recommend_quota_management(&analysis).await?);

        // 5. 成本分摊建议（如果是团队账户）
        recommendations.extend(self.recommend_cost_allocation(&analysis).await?);

        // 按潜在节省金额降序排序
        recommendations.sort_by(|a, b| {
            b.potential_savings
                .partial_cmp(&a.potential_savings)
                .unwrap()
        });

        Ok(recommendations)
    }

    /// 模型选择建议
    ///
    /// 为高价模型推荐更经济的替代方案
    async fn recommend_model_selection(
        &self,
        analysis: &UsageAnalysis,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 加载模型配置
        let model_configs = self.load_model_configs().await?;

        for model_usage in &analysis.model_breakdown {
            // 查找更便宜的替代模型
            if let Some(alternative) = self
                .find_cheaper_alternative(&model_usage.model_name, &model_configs)
                .await?
            {
                let potential_savings = model_usage.total_cost * 0.5; // 假设切换后节省 50%

                recommendations.push(OptimizationRecommendation {
                    recommendation_id: format!("model-switch-{}", model_usage.model_name),
                    category: RecommendationCategory::ModelSelection,
                    title: format!("考虑切换到更经济的模型: {alternative}"),
                    description: format!(
                        "您正在使用 {} 模型，上月花费 ${:.2}。对于简单任务，可以切换到 {}，预计每月可节省约 ${:.2}",
                        model_usage.model_name,
                        model_usage.total_cost,
                        alternative,
                        potential_savings
                    ),
                    potential_savings,
                    effort: EffortLevel::Low,
                    priority: if model_usage.total_cost > 50.0 {
                        Priority::High
                    } else {
                        Priority::Medium
                    },
                    action_items: vec![
                        ActionItem {
                            action: format!("评估 {alternative} 的适用场景"),
                            estimated_impact: potential_savings * 0.3,
                            implementation_time: std::time::Duration::from_hours(1),
                        },
                        ActionItem {
                            action: "更新模型路由配置".to_string(),
                            estimated_impact: potential_savings * 0.7,
                            implementation_time: std::time::Duration::from_hours(2),
                        },
                    ],
                    affected_models: vec![model_usage.model_name.clone()],
                    created_at: Utc::now(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 查找更便宜的替代模型
    ///
    /// 从 model_configs 表查询相似能力但更便宜的模型
    pub async fn find_cheaper_alternative(
        &self,
        model_name: &str,
        model_configs: &[ModelConfigCache],
    ) -> Result<Option<String>> {
        // 找到当前模型的配置
        let current_model = model_configs.iter().find(|m| {
            m.name.to_lowercase() == model_name.to_lowercase()
                || model_name.to_lowercase().contains(&m.name.to_lowercase())
        });

        if current_model.is_none() {
            // 如果找不到精确匹配，尝试模糊匹配
            let similar_models: Vec<&ModelConfigCache> = model_configs
                .iter()
                .filter(|m| {
                    model_name.to_lowercase().contains(&m.name.to_lowercase())
                        || m.name.to_lowercase().contains(&model_name.to_lowercase())
                })
                .collect();

            if similar_models.is_empty() {
                return Ok(None);
            }
        }

        let current = match current_model {
            Some(m) => m,
            None => return Ok(None),
        };

        // 计算当前模型的价格（假设输入输出价格比为 3:1）
        let current_price = current.input_price * 0.75 + current.output_price * 0.25;

        // 查找相似能力但更便宜的模型
        let alternatives: Vec<&ModelConfigCache> = model_configs
            .iter()
            .filter(|m| {
                // 排除自身
                if m.name == current.name {
                    return false;
                }

                // 计算价格
                let m_price = m.input_price * 0.75 + m.output_price * 0.25;

                // 价格必须更低
                if m_price >= current_price {
                    return false;
                }

                // 能力交集不为空（至少有一种相同的能力）
                let common_caps: Vec<_> = m
                    .capabilities
                    .iter()
                    .filter(|c| current.capabilities.contains(c))
                    .collect();

                !common_caps.is_empty()
            })
            .collect();

        // 返回最便宜的替代
        let cheapest = alternatives.iter().min_by(|a, b| {
            let price_a = a.input_price * 0.75 + a.output_price * 0.25;
            let price_b = b.input_price * 0.75 + b.output_price * 0.25;
            price_a.partial_cmp(&price_b).unwrap()
        });

        Ok(cheapest.map(|m| m.name.clone()))
    }

    /// 从数据库加载模型配置
    async fn load_model_configs(&self) -> Result<Vec<ModelConfigCache>> {
        use model_configs::Column as M;

        let models = model_configs::Entity::find()
            .filter(M::Enabled.eq(true))
            .all(&self.db)
            .await?;

        let configs: Vec<ModelConfigCache> = models
            .iter()
            .map(|m| {
                let capabilities = m.get_capabilities();
                ModelConfigCache {
                    name: m.name.clone(),
                    provider: m.provider.clone(),
                    input_price: m.input_price,
                    output_price: m.output_price,
                    context_window: m.context_window,
                    capabilities,
                }
            })
            .collect();

        Ok(configs)
    }

    /// 缓存策略建议
    async fn recommend_caching(
        &self,
        analysis: &UsageAnalysis,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 检测重复查询模式
        for pattern in &analysis.patterns {
            if pattern.pattern_type == PatternType::RepetitiveQueries {
                let potential_savings = analysis.total_cost * pattern.frequency * 0.3; // 假设缓存命中率 30%

                recommendations.push(OptimizationRecommendation {
                    recommendation_id: "caching-repetitive-queries".into(),
                    category: RecommendationCategory::Caching,
                    title: "启用响应缓存".into(),
                    description: format!(
                        "检测到 {:.0}% 的请求是重复查询。启用缓存可节省约 ${:.2}/月",
                        pattern.frequency * 100.0,
                        potential_savings
                    ),
                    potential_savings,
                    effort: EffortLevel::Medium,
                    priority: Priority::High,
                    action_items: vec![
                        ActionItem {
                            action: "实现 Redis 缓存层".into(),
                            estimated_impact: potential_savings,
                            implementation_time: std::time::Duration::from_hours(8),
                        },
                        ActionItem {
                            action: "配置缓存过期策略".into(),
                            estimated_impact: potential_savings * 0.2,
                            implementation_time: std::time::Duration::from_hours(2),
                        },
                    ],
                    affected_models: analysis
                        .model_breakdown
                        .iter()
                        .map(|m| m.model_name.clone())
                        .collect(),
                    created_at: Utc::now(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 请求优化建议
    async fn recommend_request_optimization(
        &self,
        analysis: &UsageAnalysis,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 检查失败请求
        for model_usage in &analysis.model_breakdown {
            if model_usage.success_rate < 0.95 {
                let wasted_cost = model_usage.total_cost * (1.0 - model_usage.success_rate);

                recommendations.push(OptimizationRecommendation {
                    recommendation_id: format!("optimize-failures-{}", model_usage.model_name),
                    category: RecommendationCategory::RequestOptimization,
                    title: format!("提高 {} 的成功率", model_usage.model_name),
                    description: format!(
                        "该模型成功率为 {:.1}%，失败请求浪费了 ${:.2}。建议检查请求参数或账户状态",
                        model_usage.success_rate * 100.0,
                        wasted_cost
                    ),
                    potential_savings: wasted_cost,
                    effort: EffortLevel::Low,
                    priority: if model_usage.success_rate < 0.8 {
                        Priority::Urgent
                    } else {
                        Priority::High
                    },
                    action_items: vec![
                        ActionItem {
                            action: "分析失败原因".into(),
                            estimated_impact: wasted_cost * 0.8,
                            implementation_time: std::time::Duration::from_hours(2),
                        },
                        ActionItem {
                            action: "实现重试机制".into(),
                            estimated_impact: wasted_cost * 0.2,
                            implementation_time: std::time::Duration::from_hours(4),
                        },
                    ],
                    affected_models: vec![model_usage.model_name.clone()],
                    created_at: Utc::now(),
                });
            }
        }

        // 批量请求优化建议
        for pattern in &analysis.patterns {
            if pattern.pattern_type == PatternType::BatchRequests {
                recommendations.push(OptimizationRecommendation {
                    recommendation_id: "batch-optimization".into(),
                    category: RecommendationCategory::RequestOptimization,
                    title: "优化批量请求处理".into(),
                    description: pattern.description.clone(),
                    potential_savings: pattern.impact.cost_impact,
                    effort: EffortLevel::Medium,
                    priority: Priority::Medium,
                    action_items: vec![ActionItem {
                        action: "实现请求批处理".into(),
                        estimated_impact: pattern.impact.cost_impact,
                        implementation_time: std::time::Duration::from_hours(4),
                    }],
                    affected_models: vec![],
                    created_at: Utc::now(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 配额管理建议
    async fn recommend_quota_management(
        &self,
        analysis: &UsageAnalysis,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 检查异常使用
        for anomaly in &analysis.anomalies {
            if anomaly.anomaly_type == AnomalyType::QuotaOverrun {
                recommendations.push(OptimizationRecommendation {
                    recommendation_id: "quota-overrun".into(),
                    category: RecommendationCategory::QuotaManagement,
                    title: "配额超支警告".into(),
                    description: anomaly.description.clone(),
                    potential_savings: anomaly.estimated_extra_cost,
                    effort: EffortLevel::Low,
                    priority: Priority::Urgent,
                    action_items: vec![
                        ActionItem {
                            action: "立即检查使用情况".into(),
                            estimated_impact: anomaly.estimated_extra_cost,
                            implementation_time: std::time::Duration::from_hours(1),
                        },
                        ActionItem {
                            action: "设置配额告警阈值".into(),
                            estimated_impact: anomaly.estimated_extra_cost * 0.5,
                            implementation_time: std::time::Duration::from_hours(2),
                        },
                    ],
                    affected_models: anomaly.affected_models.clone(),
                    created_at: Utc::now(),
                });
            }
        }

        // 高成本警告
        if analysis.total_cost > 50.0 {
            recommendations.push(OptimizationRecommendation {
                recommendation_id: "high-cost-alert".into(),
                category: RecommendationCategory::QuotaManagement,
                title: "高成本使用提醒".into(),
                description: format!(
                    "过去30天总成本 ${:.2}，建议设置预算限制和使用告警",
                    analysis.total_cost
                ),
                potential_savings: analysis.total_cost * 0.1,
                effort: EffortLevel::Low,
                priority: Priority::Medium,
                action_items: vec![ActionItem {
                    action: "设置月度预算限制".into(),
                    estimated_impact: analysis.total_cost * 0.1,
                    implementation_time: std::time::Duration::from_hours(1),
                }],
                affected_models: vec![],
                created_at: Utc::now(),
            });
        }

        Ok(recommendations)
    }

    /// 成本分摊建议
    async fn recommend_cost_allocation(
        &self,
        analysis: &UsageAnalysis,
    ) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();

        // 如果有多个模型使用，建议按团队/项目分摊成本
        if analysis.model_breakdown.len() > 2 && analysis.total_cost > 30.0 {
            recommendations.push(OptimizationRecommendation {
                recommendation_id: "cost-allocation".into(),
                category: RecommendationCategory::CostAllocation,
                title: "建立成本分摊机制".into(),
                description: format!(
                    "您使用了 {} 个不同的模型，总成本 ${:.2}。建议按项目或团队追踪成本。",
                    analysis.model_breakdown.len(),
                    analysis.total_cost
                ),
                potential_savings: analysis.total_cost * 0.05,
                effort: EffortLevel::Medium,
                priority: Priority::Low,
                action_items: vec![
                    ActionItem {
                        action: "设置项目标签".into(),
                        estimated_impact: analysis.total_cost * 0.03,
                        implementation_time: std::time::Duration::from_hours(4),
                    },
                    ActionItem {
                        action: "配置成本报表".into(),
                        estimated_impact: analysis.total_cost * 0.02,
                        implementation_time: std::time::Duration::from_hours(2),
                    },
                ],
                affected_models: vec![],
                created_at: Utc::now(),
            });
        }

        Ok(recommendations)
    }

    /// 生成成本报告
    ///
    /// 生成包含汇总统计、成本分解、趋势分析和优化建议的完整报告
    pub async fn generate_cost_report(
        &self,
        user_id: i64,
        period: TimePeriod,
    ) -> Result<CostReport> {
        info!(
            "生成用户 {} 成本报告，时间范围: {:?} - {:?}",
            user_id, period.start, period.end
        );

        let analysis = self.analyze_usage(user_id, period.clone()).await?;
        let recommendations = self.generate_recommendations(user_id).await?;

        // 计算趋势（按天分组）
        let trends = self.calculate_trends(user_id, &period).await?;

        // 计算上一周期用于对比
        let previous_period = TimePeriod {
            start: period.start - (period.end - period.start),
            end: period.start,
        };
        let previous_analysis = self.analyze_usage(user_id, previous_period).await.ok();
        let cost_change = match previous_analysis {
            Some(prev) => {
                if prev.total_cost > 0.0 {
                    ((analysis.total_cost - prev.total_cost) / prev.total_cost) * 100.0
                } else {
                    0.0
                }
            }
            None => 0.0,
        };

        // 计算总请求数
        let total_requests: i64 = analysis
            .model_breakdown
            .iter()
            .map(|m| m.request_count)
            .sum();

        // 生成摘要
        let summary = CostSummary {
            total_cost: analysis.total_cost,
            total_tokens: analysis.total_tokens,
            total_requests,
            avg_cost_per_request: if total_requests > 0 {
                analysis.total_cost / total_requests as f64
            } else {
                0.0
            },
            avg_cost_per_token: if analysis.total_tokens > 0 {
                analysis.total_cost / analysis.total_tokens as f64
            } else {
                0.0
            },
            cost_change_from_previous: cost_change,
        };

        // 生成成本分解
        let breakdown: Vec<CostBreakdownItem> = if analysis.total_cost > 0.0 {
            analysis
                .model_breakdown
                .iter()
                .map(|m| CostBreakdownItem {
                    category: m.model_name.clone(),
                    amount: m.total_cost,
                    percentage: (m.total_cost / analysis.total_cost) * 100.0,
                })
                .collect()
        } else {
            vec![]
        };

        Ok(CostReport {
            user_id,
            period,
            summary,
            breakdown,
            trends,
            recommendations,
        })
    }

    /// 计算使用趋势
    ///
    /// 按天分组统计成本、tokens 和请求数
    async fn calculate_trends(&self, user_id: i64, period: &TimePeriod) -> Result<Vec<CostTrend>> {
        // 从数据库查询使用记录
        let data = self.fetch_usage_data(user_id, period).await?;

        // 按天分组
        let mut daily_stats: HashMap<chrono::NaiveDate, (f64, i64, i64)> = HashMap::new();

        for record in data {
            let date = record.timestamp.date_naive();
            let stats = daily_stats.entry(date).or_insert((0.0, 0, 0));
            stats.0 += record.cost;
            stats.1 += record.tokens_in + record.tokens_out;
            stats.2 += 1;
        }

        // 转换为趋势数据
        let mut trends: Vec<CostTrend> = daily_stats
            .into_iter()
            .map(|(date, (cost, tokens, requests))| CostTrend {
                date: date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                cost,
                tokens,
                requests,
            })
            .collect();

        // 按日期排序
        trends.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(trends)
    }
}

/// 模型统计累加器（内部使用）
struct ModelStatsAccumulator {
    request_count: i64,
    total_tokens: i64,
    total_cost: f64,
    total_response_time: f64,
    success_count: i64,
    provider: String,
}

impl ModelStatsAccumulator {
    fn new() -> Self {
        Self {
            request_count: 0,
            total_tokens: 0,
            total_cost: 0.0,
            total_response_time: 0.0,
            success_count: 0,
            provider: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的 UsageRecord
    fn create_test_record(
        model: &str,
        tokens_in: i64,
        tokens_out: i64,
        cost: f64,
        success: bool,
        timestamp_hours_ago: i64,
    ) -> UsageRecord {
        UsageRecord {
            timestamp: Utc::now() - Duration::hours(timestamp_hours_ago),
            model: model.to_string(),
            provider: "openai".to_string(),
            tokens_in,
            tokens_out,
            cost,
            response_time_ms: 1000.0,
            success,
            prompt_hash: None,
        }
    }

    #[test]
    fn test_pattern_type_equality() {
        assert_eq!(PatternType::HighVolumeChat, PatternType::HighVolumeChat);
        assert_ne!(PatternType::HighVolumeChat, PatternType::LongContextUsage);
    }

    #[test]
    fn test_anomaly_type_equality() {
        assert_eq!(
            AnomalyType::UnexpectedHighCost,
            AnomalyType::UnexpectedHighCost
        );
        assert_ne!(
            AnomalyType::UnexpectedHighCost,
            AnomalyType::FailedRequestSpike
        );
    }

    #[test]
    fn test_severity_ordering() {
        let severities = vec![
            Severity::Critical,
            Severity::High,
            Severity::Medium,
            Severity::Low,
        ];
        assert_eq!(severities[0], Severity::Critical);
        assert_eq!(severities[3], Severity::Low);
    }

    #[test]
    fn test_effort_level() {
        assert_eq!(EffortLevel::Low, EffortLevel::Low);
        assert_ne!(EffortLevel::Low, EffortLevel::Medium);
    }

    #[test]
    fn test_priority_ordering() {
        let priorities = vec![
            Priority::Urgent,
            Priority::High,
            Priority::Medium,
            Priority::Low,
        ];
        assert_eq!(priorities[0], Priority::Urgent);
        assert_eq!(priorities[3], Priority::Low);
    }

    #[test]
    fn test_usage_analysis_serialization() {
        let analysis = UsageAnalysis {
            user_id: 1,
            period: TimePeriod {
                start: Utc::now() - Duration::days(30),
                end: Utc::now(),
            },
            total_cost: 100.0,
            total_tokens: 1000000,
            model_breakdown: vec![ModelUsage {
                model_name: "gpt-4".to_string(),
                provider: "openai".to_string(),
                request_count: 100,
                total_tokens: 50000,
                total_cost: 50.0,
                avg_response_time_ms: 2000.0,
                success_rate: 0.95,
            }],
            patterns: vec![],
            anomalies: vec![],
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: UsageAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, 1);
        assert_eq!(deserialized.total_cost, 100.0);
        assert_eq!(deserialized.model_breakdown.len(), 1);
    }

    #[test]
    fn test_cost_report_serialization() {
        let report = CostReport {
            user_id: 1,
            period: TimePeriod {
                start: Utc::now() - Duration::days(30),
                end: Utc::now(),
            },
            summary: CostSummary {
                total_cost: 100.0,
                total_tokens: 1000000,
                total_requests: 100,
                avg_cost_per_request: 1.0,
                avg_cost_per_token: 0.0001,
                cost_change_from_previous: 10.0,
            },
            breakdown: vec![CostBreakdownItem {
                category: "gpt-4".to_string(),
                amount: 50.0,
                percentage: 50.0,
            }],
            trends: vec![],
            recommendations: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: CostReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, 1);
        assert_eq!(deserialized.summary.total_cost, 100.0);
        assert_eq!(deserialized.breakdown.len(), 1);
    }

    #[test]
    fn test_optimization_recommendation() {
        let recommendation = OptimizationRecommendation {
            recommendation_id: "test-1".to_string(),
            category: RecommendationCategory::ModelSelection,
            title: "Test Recommendation".to_string(),
            description: "Test description".to_string(),
            potential_savings: 10.0,
            effort: EffortLevel::Low,
            priority: Priority::High,
            action_items: vec![ActionItem {
                action: "Test action".to_string(),
                estimated_impact: 10.0,
                implementation_time: std::time::Duration::from_hours(1),
            }],
            affected_models: vec!["gpt-4".to_string()],
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&recommendation).unwrap();
        let deserialized: OptimizationRecommendation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.recommendation_id, "test-1");
        assert_eq!(deserialized.potential_savings, 10.0);
        assert_eq!(deserialized.action_items.len(), 1);
    }

    #[test]
    fn test_time_period() {
        let period = TimePeriod {
            start: Utc::now() - Duration::days(30),
            end: Utc::now(),
        };

        let duration = period.end - period.start;
        assert_eq!(duration.num_days(), 30);
    }

    #[test]
    fn test_impact_calculation() {
        let impact = Impact {
            cost_impact: 100.0,
            efficiency_impact: 0.5,
        };

        assert_eq!(impact.cost_impact, 100.0);
        assert_eq!(impact.efficiency_impact, 0.5);
    }

    #[test]
    fn test_model_usage() {
        let usage = ModelUsage {
            model_name: "gpt-4".to_string(),
            provider: "openai".to_string(),
            request_count: 100,
            total_tokens: 50000,
            total_cost: 50.0,
            avg_response_time_ms: 2000.0,
            success_rate: 0.95,
        };

        assert_eq!(usage.model_name, "gpt-4");
        assert_eq!(usage.success_rate, 0.95);
    }

    #[test]
    fn test_anomaly_creation() {
        let anomaly = Anomaly {
            anomaly_type: AnomalyType::FailedRequestSpike,
            detected_at: Utc::now(),
            severity: Severity::High,
            description: "Test anomaly".to_string(),
            affected_models: vec!["gpt-4".to_string()],
            estimated_extra_cost: 10.0,
        };

        assert_eq!(anomaly.anomaly_type, AnomalyType::FailedRequestSpike);
        assert_eq!(anomaly.severity, Severity::High);
    }

    #[test]
    fn test_usage_pattern() {
        let pattern = UsagePattern {
            pattern_type: PatternType::HighVolumeChat,
            description: "Test pattern".to_string(),
            frequency: 0.8,
            impact: Impact {
                cost_impact: 50.0,
                efficiency_impact: 0.3,
            },
        };

        assert_eq!(pattern.pattern_type, PatternType::HighVolumeChat);
        assert_eq!(pattern.frequency, 0.8);
    }

    #[test]
    fn test_cost_summary() {
        let summary = CostSummary {
            total_cost: 100.0,
            total_tokens: 1000000,
            total_requests: 100,
            avg_cost_per_request: 1.0,
            avg_cost_per_token: 0.0001,
            cost_change_from_previous: 10.0,
        };

        assert_eq!(summary.total_cost, 100.0);
        assert_eq!(summary.avg_cost_per_request, 1.0);
        assert_eq!(summary.cost_change_from_previous, 10.0);
    }

    #[test]
    fn test_cost_breakdown_item() {
        let item = CostBreakdownItem {
            category: "gpt-4".to_string(),
            amount: 50.0,
            percentage: 50.0,
        };

        assert_eq!(item.category, "gpt-4");
        assert_eq!(item.percentage, 50.0);
    }

    #[test]
    fn test_cost_trend() {
        let trend = CostTrend {
            date: Utc::now(),
            cost: 10.0,
            tokens: 10000,
            requests: 100,
        };

        assert_eq!(trend.cost, 10.0);
        assert_eq!(trend.tokens, 10000);
    }

    #[test]
    fn test_action_item() {
        let action = ActionItem {
            action: "Test action".to_string(),
            estimated_impact: 10.0,
            implementation_time: std::time::Duration::from_hours(2),
        };

        assert_eq!(action.action, "Test action");
        assert_eq!(action.estimated_impact, 10.0);
    }
}
