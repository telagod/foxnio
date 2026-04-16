//! Sora 服务层 - Sora Service Layer
//!
//! 提供图片/视频生成路由和按次计费功能

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::models::{get_model_config, SoraModelConfig, SoraModelType};
use crate::entity::usages;
use crate::service::billing::BillingService;

/// Sora 生成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraGenerateRequest {
    /// 提示词
    pub prompt: String,
    /// 模型名称
    pub model: String,
    /// 负面提示词
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// 参考图片 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_image: Option<String>,
    /// 种子值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

/// Sora 生成响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraGenerateResponse {
    /// 任务 ID
    pub id: String,
    /// 状态
    pub status: String,
    /// 模型
    pub model: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 结果 URL（图片或视频）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_url: Option<String>,
    /// 缩略图 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SoraErrorInfo>,
}

/// 错误信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraErrorInfo {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// 按次计费配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraPricingConfig {
    /// 基础价格（分）
    pub base_price: i64,
    /// 视频每秒价格（分）
    pub price_per_second: i64,
    /// HD 额外费用倍率
    pub hd_multiplier: f64,
    /// Pro 额外费用倍率
    pub pro_multiplier: f64,
}

impl Default for SoraPricingConfig {
    fn default() -> Self {
        Self {
            base_price: 100,      // 1 元基础费用
            price_per_second: 10, // 0.1 元/秒
            hd_multiplier: 2.0,
            pro_multiplier: 1.5,
        }
    }
}

/// Sora 服务
pub struct SoraService {
    pricing_config: SoraPricingConfig,
}

impl SoraService {
    /// 创建新的服务实例
    pub fn new(pricing_config: SoraPricingConfig) -> Self {
        Self { pricing_config }
    }

    /// 使用默认配置创建服务
    pub fn with_default_pricing() -> Self {
        Self::new(SoraPricingConfig::default())
    }

    /// 验证请求
    pub fn validate_request(&self, request: &SoraGenerateRequest) -> Result<SoraModelConfig> {
        let model_config = get_model_config(&request.model)
            .with_context(|| format!("Unknown model: {}", request.model))?;

        // 验证提示词长度
        if request.prompt.is_empty() {
            bail!("Prompt cannot be empty");
        }

        if request.prompt.len() > 4000 {
            bail!("Prompt too long, maximum 4000 characters");
        }

        Ok(model_config)
    }

    /// 计算费用（单位：分）
    pub fn calculate_cost(&self, model_config: &SoraModelConfig) -> i64 {
        match model_config.model_type {
            SoraModelType::Image => {
                // 图片按固定价格计费
                let mut cost = self.pricing_config.base_price;

                // 大尺寸加价
                if let (Some(w), Some(h)) = (model_config.width, model_config.height) {
                    if w * h > 360 * 360 {
                        cost = (cost as f64 * 1.5) as i64;
                    }
                }

                cost
            }
            SoraModelType::Video => {
                // 视频按时长计费
                let duration_secs = model_config.duration_seconds().unwrap_or(10);
                let mut cost = self.pricing_config.base_price
                    + (duration_secs as i64 * self.pricing_config.price_per_second);

                // HD 加价
                if model_config.size.as_deref() == Some("large") {
                    cost = (cost as f64 * self.pricing_config.hd_multiplier) as i64;
                }

                // Pro 加价
                if model_config.require_pro {
                    cost = (cost as f64 * self.pricing_config.pro_multiplier) as i64;
                }

                cost
            }
            SoraModelType::PromptEnhance => {
                // 提示词增强低价
                self.pricing_config.base_price / 5
            }
        }
    }

    /// 构建上游请求体
    pub fn build_upstream_request(
        &self,
        request: &SoraGenerateRequest,
        model_config: &SoraModelConfig,
    ) -> serde_json::Value {
        match model_config.model_type {
            SoraModelType::Image => {
                let mut body = serde_json::Map::new();
                body.insert("prompt".to_string(), request.prompt.clone().into());

                if let Some(width) = model_config.width {
                    body.insert("width".to_string(), width.into());
                }
                if let Some(height) = model_config.height {
                    body.insert("height".to_string(), height.into());
                }

                if let Some(ref img) = request.reference_image {
                    body.insert("reference_image".to_string(), img.clone().into());
                }

                if let Some(seed) = request.seed {
                    body.insert("seed".to_string(), seed.into());
                }

                serde_json::Value::Object(body)
            }
            SoraModelType::Video => {
                let mut body = serde_json::Map::new();
                body.insert("prompt".to_string(), request.prompt.clone().into());

                if let Some(ref model) = model_config.model {
                    body.insert("model".to_string(), model.clone().into());
                }

                if let Some(ref orientation) = model_config.orientation {
                    body.insert("orientation".to_string(), orientation.clone().into());
                }

                if let Some(frames) = model_config.frames {
                    body.insert("frames".to_string(), frames.into());
                }

                if let Some(ref size) = model_config.size {
                    body.insert("size".to_string(), size.clone().into());
                }

                if let Some(ref negative) = request.negative_prompt {
                    body.insert("negative_prompt".to_string(), negative.clone().into());
                }

                if let Some(ref img) = request.reference_image {
                    body.insert("reference_image".to_string(), img.clone().into());
                }

                if let Some(seed) = request.seed {
                    body.insert("seed".to_string(), seed.into());
                }

                serde_json::Value::Object(body)
            }
            SoraModelType::PromptEnhance => {
                let mut body = serde_json::Map::new();
                body.insert("prompt".to_string(), request.prompt.clone().into());

                if let Some(ref level) = model_config.expansion_level {
                    body.insert("expansion_level".to_string(), level.clone().into());
                }

                if let Some(duration) = model_config.duration_s {
                    body.insert("duration_s".to_string(), duration.into());
                }

                serde_json::Value::Object(body)
            }
        }
    }

    /// 获取上游 API 路径
    pub fn get_upstream_path(&self, model_config: &SoraModelConfig) -> &'static str {
        match model_config.model_type {
            SoraModelType::Image => "/v1/images/generations",
            SoraModelType::Video => "/v1/videos/generations",
            SoraModelType::PromptEnhance => "/v1/prompts/enhance",
        }
    }
}

/// Sora 路由服务
pub struct SoraRouterService {
    sora_service: Arc<SoraService>,
    billing_service: Arc<BillingService>,
    db: DatabaseConnection,
}

impl SoraRouterService {
    /// 创建新的路由服务
    pub fn new(
        sora_service: Arc<SoraService>,
        billing_service: Arc<BillingService>,
        db: DatabaseConnection,
    ) -> Self {
        Self {
            sora_service,
            billing_service,
            db,
        }
    }

    /// 处理生成请求（预扣费）
    pub async fn prepare_generation(
        &self,
        _user_id: Uuid,
        _api_key_id: Uuid,
        request: &SoraGenerateRequest,
    ) -> Result<(SoraModelConfig, i64)> {
        // 验证请求并获取模型配置
        let model_config = self.sora_service.validate_request(request)?;

        // 计算费用
        let cost = self.sora_service.calculate_cost(&model_config);

        // NOTE: 检查用户余额
        // 这里需要实现余额检查逻辑

        Ok((model_config, cost))
    }

    /// 记录使用量（完成扣费）
    pub async fn record_usage(
        &self,
        user_id: Uuid,
        api_key_id: Uuid,
        model: &str,
        cost: i64,
        success: bool,
        error_message: Option<String>,
    ) -> Result<()> {
        // 创建使用记录
        let usage = usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            api_key_id: Set(api_key_id),
            account_id: Set(None),
            model: Set(model.to_string()),
            input_tokens: Set(0),
            output_tokens: Set(0),
            cost: Set(cost),
            request_id: Set(None),
            success: Set(success),
            error_message: Set(error_message),
            metadata: Set(Some(serde_json::json!({
                "gateway": "sora",
            }))),
            created_at: Set(Utc::now()),
        };

        usage.insert(&self.db).await?;

        Ok(())
    }
}

/// 创建 Sora OpenAI 兼容模型列表
pub fn create_sora_model_list(hide_prompt_enhance: bool) -> serde_json::Value {
    use super::models::default_sora_models;

    let models = default_sora_models(hide_prompt_enhance);

    serde_json::json!({
        "object": "list",
        "data": models.iter().map(|m| serde_json::json!({
            "id": m.id,
            "object": m.object,
            "owned_by": m.owned_by,
            "type": m.model_type,
            "display_name": m.display_name,
        })).collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_request() {
        let service = SoraService::with_default_pricing();

        let request = SoraGenerateRequest {
            prompt: "A beautiful sunset".to_string(),
            model: "gpt-image".to_string(),
            negative_prompt: None,
            reference_image: None,
            seed: None,
        };

        let config = service.validate_request(&request).unwrap();
        assert_eq!(config.model_type, SoraModelType::Image);
    }

    #[test]
    fn test_calculate_cost_image() {
        let service = SoraService::with_default_pricing();
        let config = get_model_config("gpt-image").unwrap();
        let cost = service.calculate_cost(&config);
        assert!(cost > 0);
    }

    #[test]
    fn test_calculate_cost_video() {
        let service = SoraService::with_default_pricing();

        let config = get_model_config("sora2-landscape-10s").unwrap();
        let cost_standard = service.calculate_cost(&config);

        let config_pro = get_model_config("sora2pro-landscape-10s").unwrap();
        let cost_pro = service.calculate_cost(&config_pro);

        // Pro 应该更贵
        assert!(cost_pro > cost_standard);
    }

    #[test]
    fn test_calculate_cost_hd() {
        let service = SoraService::with_default_pricing();

        let config = get_model_config("sora2pro-landscape-10s").unwrap();
        let cost_sd = service.calculate_cost(&config);

        let config_hd = get_model_config("sora2pro-hd-landscape-10s").unwrap();
        let cost_hd = service.calculate_cost(&config_hd);

        // HD 应该更贵
        assert!(cost_hd > cost_sd);
    }

    #[test]
    fn test_build_upstream_request() {
        let service = SoraService::with_default_pricing();

        let request = SoraGenerateRequest {
            prompt: "A cat playing piano".to_string(),
            model: "sora2-landscape-10s".to_string(),
            negative_prompt: Some("blurry".to_string()),
            reference_image: None,
            seed: Some(12345),
        };

        let config = service.validate_request(&request).unwrap();
        let body = service.build_upstream_request(&request, &config);

        assert_eq!(body["prompt"], "A cat playing piano");
        assert_eq!(body["negative_prompt"], "blurry");
        assert_eq!(body["seed"], 12345);
    }

    #[test]
    fn test_create_model_list() {
        let list = create_sora_model_list(false);
        assert_eq!(list["object"], "list");

        let data = list["data"].as_array().unwrap();
        assert!(!data.is_empty());
    }
}
