//! Model Configuration Entity
//!
//! 动态模型配置存储，支持运行时热加载

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 模型配置实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "model_configs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// 模型名称（唯一标识）
    #[sea_orm(unique)]
    pub name: String,
    /// 模型别名列表（JSON 数组）
    pub aliases: Option<JsonValue>,
    /// 提供商：openai, anthropic, google, deepseek, mistral, cohere
    pub provider: String,
    /// API 名称（发送给上游的真实模型名）
    pub api_name: String,
    /// 显示名称
    pub display_name: String,
    /// 输入价格（每 1M tokens，USD）
    pub input_price: f64,
    /// 输出价格（每 1M tokens，USD）
    pub output_price: f64,
    /// 缓存读取价格（每 1M tokens，USD）
    pub cache_read_price: Option<f64>,
    /// 缓存创建价格（每 1M tokens，USD）
    pub cache_creation_price: Option<f64>,
    /// 最大输出 tokens
    pub max_tokens: i32,
    /// 上下文窗口大小
    pub context_window: i32,
    /// 最大并发数
    pub max_concurrent: i32,
    /// 降级模型列表（JSON 数组）
    pub fallback_models: Option<JsonValue>,
    /// 模型能力（JSON 对象）
    pub capabilities: Option<JsonValue>,
    /// 是否支持流式输出
    #[sea_orm(default_value = true)]
    pub supports_streaming: bool,
    /// 是否支持函数调用
    #[sea_orm(default_value = false)]
    pub supports_function_calling: bool,
    /// 是否支持视觉输入
    #[sea_orm(default_value = false)]
    pub supports_vision: bool,
    /// 是否启用
    #[sea_orm(default_value = true)]
    pub enabled: bool,
    /// 优先级（数字越大优先级越高）
    #[sea_orm(default_value = 0)]
    pub priority: i32,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 解析别名列表
    pub fn get_aliases(&self) -> Vec<String> {
        self.aliases
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 解析降级模型列表
    pub fn get_fallback_models(&self) -> Vec<String> {
        self.fallback_models
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 解析能力列表
    pub fn get_capabilities(&self) -> Vec<String> {
        let mut capabilities = Vec::new();

        if let Some(cap) = self.capabilities.as_ref().and_then(|v| v.as_object()) {
            if cap.get("chat").and_then(|v| v.as_bool()).unwrap_or(false) {
                capabilities.push("chat".to_string());
            }
            if cap.get("code").and_then(|v| v.as_bool()).unwrap_or(false) {
                capabilities.push("code".to_string());
            }
            if cap.get("math").and_then(|v| v.as_bool()).unwrap_or(false) {
                capabilities.push("math".to_string());
            }
            if cap
                .get("long_context")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                capabilities.push("long_context".to_string());
            }
            if cap
                .get("multilingual")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                capabilities.push("multilingual".to_string());
            }
            if cap.get("tools").and_then(|v| v.as_bool()).unwrap_or(false) {
                capabilities.push("tools".to_string());
            }
        }

        if self.supports_vision {
            capabilities.push("vision".to_string());
        }
        if self.supports_function_calling {
            capabilities.push("function_calling".to_string());
        }

        capabilities
    }

    /// 转换为 API 响应格式
    pub fn to_model_info(&self) -> ModelInfoResponse {
        ModelInfoResponse {
            id: self.name.clone(),
            name: self.display_name.clone(),
            provider: self.provider.clone(),
            context_window: self.context_window as u32,
            max_tokens: self.max_tokens as u32,
            input_price: self.input_price,
            output_price: self.output_price,
            capabilities: self.get_capabilities(),
            enabled: self.enabled,
        }
    }
}

/// 模型信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoResponse {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_window: u32,
    pub max_tokens: u32,
    pub input_price: f64,
    pub output_price: f64,
    pub capabilities: Vec<String>,
    pub enabled: bool,
}

/// 创建模型请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateModelRequest {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub provider: String,
    #[serde(default)]
    pub api_name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub input_price: f64,
    #[serde(default)]
    pub output_price: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_context_window")]
    pub context_window: i32,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: i32,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default = "default_true")]
    pub supports_streaming: bool,
    #[serde(default)]
    pub supports_function_calling: bool,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_max_tokens() -> i32 {
    4096
}
fn default_context_window() -> i32 {
    8192
}
fn default_max_concurrent() -> i32 {
    5
}
fn default_true() -> bool {
    true
}

/// 模型能力配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    #[serde(default)]
    pub chat: bool,
    #[serde(default)]
    pub code: bool,
    #[serde(default)]
    pub math: bool,
    #[serde(default)]
    pub long_context: bool,
    #[serde(default)]
    pub multilingual: bool,
    #[serde(default)]
    pub tools: bool,
}

/// 更新模型请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub provider: Option<String>,
    pub api_name: Option<String>,
    pub display_name: Option<String>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub max_tokens: Option<i32>,
    pub context_window: Option<i32>,
    pub max_concurrent: Option<i32>,
    pub fallback_models: Option<Vec<String>>,
    pub capabilities: Option<ModelCapabilities>,
    pub supports_streaming: Option<bool>,
    pub supports_function_calling: Option<bool>,
    pub supports_vision: Option<bool>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
}
