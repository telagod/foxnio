//! 模型配置定义
//!
//! 包含各模型的详细配置信息：max_tokens, pricing, context_window 等

#![allow(dead_code)]
use super::{Model, ModelProvider};
use crate::gateway::providers::default_provider_registry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型标识
    pub model: Model,
    /// API 名称
    pub api_name: String,
    /// 显示名称
    pub display_name: String,
    /// 最大输出 tokens
    pub max_tokens: u32,
    /// 上下文窗口大小
    pub context_window: u32,
    /// 输入价格 (每 1M tokens, USD)
    pub input_price_per_m: f64,
    /// 输出价格 (每 1M tokens, USD)
    pub output_price_per_m: f64,
    /// 模型能力
    pub capabilities: ModelCapabilities,
    /// 是否支持流式输出
    pub supports_streaming: bool,
    /// 是否支持函数调用
    pub supports_function_calling: bool,
    /// 是否支持视觉输入
    pub supports_vision: bool,
}

/// 模型能力
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    /// 支持对话
    pub chat: bool,
    /// 支持代码生成
    pub code: bool,
    /// 支持数学推理
    pub math: bool,
    /// 支持长文本
    pub long_context: bool,
    /// 支持多语言
    pub multilingual: bool,
    /// 支持工具调用
    pub tools: bool,
}

/// 提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// 提供商
    pub provider: ModelProvider,
    /// 基础 URL
    pub base_url: String,
    /// 认证头名称
    pub auth_header: String,
    /// 是否需要版本头
    pub requires_version_header: bool,
    /// API 版本
    pub api_version: Option<String>,
    /// 额外请求头
    pub extra_headers: HashMap<String, String>,
}

/// 模型详细信息（用于 API 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// 模型标识
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 提供商
    pub provider: String,
    /// 上下文窗口
    pub context_window: u32,
    /// 最大输出
    pub max_tokens: u32,
    /// 输入价格
    pub input_price: f64,
    /// 输出价格
    pub output_price: f64,
    /// 能力列表
    pub capabilities: Vec<String>,
}

impl From<&ModelConfig> for ModelInfo {
    fn from(config: &ModelConfig) -> Self {
        let mut capabilities = Vec::new();
        if config.capabilities.chat {
            capabilities.push("chat".to_string());
        }
        if config.capabilities.code {
            capabilities.push("code".to_string());
        }
        if config.capabilities.math {
            capabilities.push("math".to_string());
        }
        if config.capabilities.long_context {
            capabilities.push("long_context".to_string());
        }
        if config.supports_vision {
            capabilities.push("vision".to_string());
        }
        if config.supports_function_calling {
            capabilities.push("function_calling".to_string());
        }

        ModelInfo {
            id: config.api_name.clone(),
            name: config.display_name.clone(),
            provider: config.model.provider().to_string(),
            context_window: config.context_window,
            max_tokens: config.max_tokens,
            input_price: config.input_price_per_m,
            output_price: config.output_price_per_m,
            capabilities,
        }
    }
}

// 所有模型的配置
pub static MODEL_CONFIGS: Lazy<Vec<(Model, ModelConfig)>> = Lazy::new(|| {
    vec![
        // ==================== OpenAI 模型 ====================
        (
            Model::GPT4Turbo,
            ModelConfig {
                model: Model::GPT4Turbo,
                api_name: "gpt-4-turbo-preview".to_string(),
                display_name: "GPT-4 Turbo".to_string(),
                max_tokens: 4096,
                context_window: 128_000,
                input_price_per_m: 10.0,
                output_price_per_m: 30.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::GPT4,
            ModelConfig {
                model: Model::GPT4,
                api_name: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                max_tokens: 4096,
                context_window: 8192,
                input_price_per_m: 30.0,
                output_price_per_m: 60.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        (
            Model::GPT4o,
            ModelConfig {
                model: Model::GPT4o,
                api_name: "gpt-4o".to_string(),
                display_name: "GPT-4o".to_string(),
                max_tokens: 4096,
                context_window: 128_000,
                input_price_per_m: 5.0,
                output_price_per_m: 15.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::GPT4oMini,
            ModelConfig {
                model: Model::GPT4oMini,
                api_name: "gpt-4o-mini".to_string(),
                display_name: "GPT-4o Mini".to_string(),
                max_tokens: 4096,
                context_window: 128_000,
                input_price_per_m: 0.15,
                output_price_per_m: 0.6,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::GPT35Turbo,
            ModelConfig {
                model: Model::GPT35Turbo,
                api_name: "gpt-3.5-turbo".to_string(),
                display_name: "GPT-3.5 Turbo".to_string(),
                max_tokens: 4096,
                context_window: 16385,
                input_price_per_m: 0.5,
                output_price_per_m: 1.5,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        // ==================== Anthropic 模型 ====================
        (
            Model::Claude3Opus,
            ModelConfig {
                model: Model::Claude3Opus,
                api_name: "claude-3-opus-20240229".to_string(),
                display_name: "Claude 3 Opus".to_string(),
                max_tokens: 4096,
                context_window: 200_000,
                input_price_per_m: 15.0,
                output_price_per_m: 75.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::Claude35Sonnet,
            ModelConfig {
                model: Model::Claude35Sonnet,
                api_name: "claude-3-5-sonnet-20241022".to_string(),
                display_name: "Claude 3.5 Sonnet".to_string(),
                max_tokens: 8192,
                context_window: 200_000,
                input_price_per_m: 3.0,
                output_price_per_m: 15.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::Claude35SonnetV2,
            ModelConfig {
                model: Model::Claude35SonnetV2,
                api_name: "claude-3-5-sonnet-20241022".to_string(),
                display_name: "Claude 3.5 Sonnet v2".to_string(),
                max_tokens: 8192,
                context_window: 200_000,
                input_price_per_m: 3.0,
                output_price_per_m: 15.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::Claude3Haiku,
            ModelConfig {
                model: Model::Claude3Haiku,
                api_name: "claude-3-haiku-20240307".to_string(),
                display_name: "Claude 3 Haiku".to_string(),
                max_tokens: 4096,
                context_window: 200_000,
                input_price_per_m: 0.25,
                output_price_per_m: 1.25,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        // ==================== Google 模型 ====================
        (
            Model::GeminiPro,
            ModelConfig {
                model: Model::GeminiPro,
                api_name: "gemini-pro".to_string(),
                display_name: "Gemini Pro".to_string(),
                max_tokens: 2048,
                context_window: 32760,
                input_price_per_m: 0.5,
                output_price_per_m: 1.5,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: false,
                    multilingual: true,
                    tools: false,
                },
                supports_streaming: true,
                supports_function_calling: false,
                supports_vision: false,
            },
        ),
        (
            Model::GeminiUltra,
            ModelConfig {
                model: Model::GeminiUltra,
                api_name: "gemini-ultra".to_string(),
                display_name: "Gemini Ultra".to_string(),
                max_tokens: 2048,
                context_window: 32760,
                input_price_per_m: 2.5,
                output_price_per_m: 7.5,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        (
            Model::Gemini15Pro,
            ModelConfig {
                model: Model::Gemini15Pro,
                api_name: "gemini-1.5-pro".to_string(),
                display_name: "Gemini 1.5 Pro".to_string(),
                max_tokens: 8192,
                context_window: 1_000_000,
                input_price_per_m: 3.5,
                output_price_per_m: 10.5,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: true,
            },
        ),
        // ==================== DeepSeek 模型 ====================
        (
            Model::DeepSeekV3,
            ModelConfig {
                model: Model::DeepSeekV3,
                api_name: "deepseek-chat".to_string(),
                display_name: "DeepSeek V3".to_string(),
                max_tokens: 4096,
                context_window: 64000,
                input_price_per_m: 0.14,
                output_price_per_m: 0.28,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        (
            Model::DeepSeekCoder,
            ModelConfig {
                model: Model::DeepSeekCoder,
                api_name: "deepseek-coder".to_string(),
                display_name: "DeepSeek Coder".to_string(),
                max_tokens: 4096,
                context_window: 16000,
                input_price_per_m: 0.14,
                output_price_per_m: 0.28,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: false,
                },
                supports_streaming: true,
                supports_function_calling: false,
                supports_vision: false,
            },
        ),
        // ==================== Mistral 模型 ====================
        (
            Model::MistralLarge,
            ModelConfig {
                model: Model::MistralLarge,
                api_name: "mistral-large-latest".to_string(),
                display_name: "Mistral Large".to_string(),
                max_tokens: 4096,
                context_window: 32000,
                input_price_per_m: 4.0,
                output_price_per_m: 12.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: true,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        (
            Model::MistralMedium,
            ModelConfig {
                model: Model::MistralMedium,
                api_name: "mistral-medium-latest".to_string(),
                display_name: "Mistral Medium".to_string(),
                max_tokens: 4096,
                context_window: 32000,
                input_price_per_m: 2.7,
                output_price_per_m: 8.1,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        (
            Model::MistralSmall,
            ModelConfig {
                model: Model::MistralSmall,
                api_name: "mistral-small-latest".to_string(),
                display_name: "Mistral Small".to_string(),
                max_tokens: 4096,
                context_window: 32000,
                input_price_per_m: 0.2,
                output_price_per_m: 0.6,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: false,
                    multilingual: true,
                    tools: false,
                },
                supports_streaming: true,
                supports_function_calling: false,
                supports_vision: false,
            },
        ),
        // ==================== Cohere 模型 ====================
        (
            Model::CommandRPlus,
            ModelConfig {
                model: Model::CommandRPlus,
                api_name: "command-r-plus".to_string(),
                display_name: "Command R+".to_string(),
                max_tokens: 4096,
                context_window: 128_000,
                input_price_per_m: 3.0,
                output_price_per_m: 15.0,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
        (
            Model::CommandR,
            ModelConfig {
                model: Model::CommandR,
                api_name: "command-r".to_string(),
                display_name: "Command R".to_string(),
                max_tokens: 4096,
                context_window: 128_000,
                input_price_per_m: 0.5,
                output_price_per_m: 1.5,
                capabilities: ModelCapabilities {
                    chat: true,
                    code: true,
                    math: false,
                    long_context: true,
                    multilingual: true,
                    tools: true,
                },
                supports_streaming: true,
                supports_function_calling: true,
                supports_vision: false,
            },
        ),
    ]
});

impl ProviderConfig {
    /// 获取所有提供商配置
    pub fn all() -> HashMap<ModelProvider, ProviderConfig> {
        default_provider_registry()
            .descriptors()
            .into_iter()
            .filter_map(|descriptor| {
                model_provider_from_key(&descriptor.key).map(|provider| {
                    let mut extra_headers = HashMap::new();
                    if let Some(api_version) = &descriptor.api_version {
                        if descriptor.requires_version_header {
                            extra_headers
                                .insert("anthropic-version".to_string(), api_version.clone());
                        }
                    }

                    (
                        provider,
                        ProviderConfig {
                            provider,
                            base_url: descriptor.base_url,
                            auth_header: descriptor.auth_header,
                            requires_version_header: descriptor.requires_version_header,
                            api_version: descriptor.api_version,
                            extra_headers,
                        },
                    )
                })
            })
            .collect()
    }

    /// 获取指定提供商的配置
    pub fn get(provider: ModelProvider) -> Self {
        Self::all()
            .get(&provider)
            .cloned()
            .expect("Provider config should exist")
    }
}

fn model_provider_from_key(key: &str) -> Option<ModelProvider> {
    match key {
        "openai" => Some(ModelProvider::OpenAI),
        "anthropic" => Some(ModelProvider::Anthropic),
        "google" | "gemini" => Some(ModelProvider::Google),
        "deepseek" => Some(ModelProvider::DeepSeek),
        "mistral" => Some(ModelProvider::Mistral),
        "cohere" => Some(ModelProvider::Cohere),
        _ => None,
    }
}

/// 获取模型配置
pub fn get_model_config(model: Model) -> Option<ModelConfig> {
    MODEL_CONFIGS
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, config)| config.clone())
}

/// 获取模型信息
pub fn get_model_info(model: Model) -> Option<ModelInfo> {
    get_model_config(model).as_ref().map(ModelInfo::from)
}

/// 获取所有可用模型
pub fn list_all_models() -> Vec<ModelInfo> {
    MODEL_CONFIGS
        .iter()
        .map(|(_, config)| ModelInfo::from(config))
        .collect()
}

/// 按提供商获取模型
pub fn list_models_by_provider(provider: ModelProvider) -> Vec<ModelInfo> {
    MODEL_CONFIGS
        .iter()
        .filter(|(model, _)| model.provider() == provider)
        .map(|(_, config)| ModelInfo::from(config))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_config() {
        let config = get_model_config(Model::GPT4Turbo);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.context_window, 128000);
    }

    #[test]
    fn test_provider_config() {
        let configs = ProviderConfig::all();
        assert_eq!(configs.len(), 6);

        let openai = ProviderConfig::get(ModelProvider::OpenAI);
        assert_eq!(openai.base_url, "https://api.openai.com");
    }

    #[test]
    fn test_list_all_models() {
        let models = list_all_models();
        assert!(models.len() >= 15);
    }

    #[test]
    fn test_model_info() {
        let info = get_model_info(Model::Claude35Sonnet);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.provider, "Anthropic");
        assert!(info.capabilities.contains(&"chat".to_string()));
    }

    #[test]
    fn test_pricing() {
        let config = get_model_config(Model::GPT4oMini).unwrap();
        assert!(config.input_price_per_m < config.output_price_per_m);
    }

    #[test]
    fn test_context_window() {
        let config = get_model_config(Model::Gemini15Pro).unwrap();
        assert_eq!(config.context_window, 1000000);
    }
}
