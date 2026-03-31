//! 多模型支持模块
//!
//! 提供统一的模型定义、配置管理和路由功能

#![allow(dead_code)]
mod config;

pub use config::{
    get_model_config, get_model_info, list_all_models, list_models_by_provider, ModelConfig,
    ModelInfo, ProviderConfig,
};

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// AI 服务提供商
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelProvider {
    OpenAI,
    Anthropic,
    Google,
    DeepSeek,
    Mistral,
    Cohere,
}

impl ModelProvider {
    /// 获取提供商的基础 URL
    pub fn base_url(&self) -> &'static str {
        match self {
            ModelProvider::OpenAI => "https://api.openai.com",
            ModelProvider::Anthropic => "https://api.anthropic.com",
            ModelProvider::Google => "https://generativelanguage.googleapis.com",
            ModelProvider::DeepSeek => "https://api.deepseek.com",
            ModelProvider::Mistral => "https://api.mistral.ai",
            ModelProvider::Cohere => "https://api.cohere.ai",
        }
    }

    /// 获取提供商的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            ModelProvider::OpenAI => "OpenAI",
            ModelProvider::Anthropic => "Anthropic",
            ModelProvider::Google => "Google",
            ModelProvider::DeepSeek => "DeepSeek",
            ModelProvider::Mistral => "Mistral AI",
            ModelProvider::Cohere => "Cohere",
        }
    }

    /// 获取认证头名称
    pub fn auth_header(&self) -> &'static str {
        match self {
            ModelProvider::Anthropic => "x-api-key",
            _ => "Authorization",
        }
    }

    /// 是否需要在请求中添加版本头
    pub fn requires_version_header(&self) -> bool {
        matches!(self, ModelProvider::Anthropic)
    }

    /// 获取所有提供商列表
    pub fn all() -> Vec<ModelProvider> {
        vec![
            ModelProvider::OpenAI,
            ModelProvider::Anthropic,
            ModelProvider::Google,
            ModelProvider::DeepSeek,
            ModelProvider::Mistral,
            ModelProvider::Cohere,
        ]
    }
}

impl fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl FromStr for ModelProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(ModelProvider::OpenAI),
            "anthropic" => Ok(ModelProvider::Anthropic),
            "google" | "gemini" => Ok(ModelProvider::Google),
            "deepseek" => Ok(ModelProvider::DeepSeek),
            "mistral" => Ok(ModelProvider::Mistral),
            "cohere" => Ok(ModelProvider::Cohere),
            _ => Err(format!("Unknown provider: {s}")),
        }
    }
}

/// AI 模型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    // OpenAI 模型
    #[serde(rename = "gpt-4-turbo")]
    GPT4Turbo,
    #[serde(rename = "gpt-4")]
    GPT4,
    #[serde(rename = "gpt-4o")]
    GPT4o,
    #[serde(rename = "gpt-4o-mini")]
    GPT4oMini,
    #[serde(rename = "gpt-3.5-turbo")]
    GPT35Turbo,

    // Anthropic 模型
    #[serde(rename = "claude-3-opus")]
    Claude3Opus,
    #[serde(rename = "claude-3.5-sonnet")]
    Claude35Sonnet,
    #[serde(rename = "claude-3-haiku")]
    Claude3Haiku,
    #[serde(rename = "claude-3.5-sonnet-v2")]
    Claude35SonnetV2,

    // Google 模型
    #[serde(rename = "gemini-pro")]
    GeminiPro,
    #[serde(rename = "gemini-ultra")]
    GeminiUltra,
    #[serde(rename = "gemini-1.5-pro")]
    Gemini15Pro,

    // DeepSeek 模型
    #[serde(rename = "deepseek-v3")]
    DeepSeekV3,
    #[serde(rename = "deepseek-coder")]
    DeepSeekCoder,

    // Mistral 模型
    #[serde(rename = "mistral-large")]
    MistralLarge,
    #[serde(rename = "mistral-medium")]
    MistralMedium,
    #[serde(rename = "mistral-small")]
    MistralSmall,

    // Cohere 模型
    #[serde(rename = "command-r-plus")]
    CommandRPlus,
    #[serde(rename = "command-r")]
    CommandR,
}

impl Model {
    /// 获取模型所属的提供商
    pub fn provider(&self) -> ModelProvider {
        match self {
            // OpenAI
            Model::GPT4Turbo
            | Model::GPT4
            | Model::GPT4o
            | Model::GPT4oMini
            | Model::GPT35Turbo => ModelProvider::OpenAI,

            // Anthropic
            Model::Claude3Opus
            | Model::Claude35Sonnet
            | Model::Claude3Haiku
            | Model::Claude35SonnetV2 => ModelProvider::Anthropic,

            // Google
            Model::GeminiPro | Model::GeminiUltra | Model::Gemini15Pro => ModelProvider::Google,

            // DeepSeek
            Model::DeepSeekV3 | Model::DeepSeekCoder => ModelProvider::DeepSeek,

            // Mistral
            Model::MistralLarge | Model::MistralMedium | Model::MistralSmall => {
                ModelProvider::Mistral
            }

            // Cohere
            Model::CommandRPlus | Model::CommandR => ModelProvider::Cohere,
        }
    }

    /// 获取模型的 API 标识符（发送给上游 API 的名称）
    pub fn api_name(&self) -> &'static str {
        match self {
            // OpenAI
            Model::GPT4Turbo => "gpt-4-turbo-preview",
            Model::GPT4 => "gpt-4",
            Model::GPT4o => "gpt-4o",
            Model::GPT4oMini => "gpt-4o-mini",
            Model::GPT35Turbo => "gpt-3.5-turbo",

            // Anthropic
            Model::Claude3Opus => "claude-3-opus-20240229",
            Model::Claude35Sonnet => "claude-3-5-sonnet-20241022",
            Model::Claude3Haiku => "claude-3-haiku-20240307",
            Model::Claude35SonnetV2 => "claude-3-5-sonnet-20241022",

            // Google
            Model::GeminiPro => "gemini-pro",
            Model::GeminiUltra => "gemini-ultra",
            Model::Gemini15Pro => "gemini-1.5-pro",

            // DeepSeek
            Model::DeepSeekV3 => "deepseek-chat",
            Model::DeepSeekCoder => "deepseek-coder",

            // Mistral
            Model::MistralLarge => "mistral-large-latest",
            Model::MistralMedium => "mistral-medium-latest",
            Model::MistralSmall => "mistral-small-latest",

            // Cohere
            Model::CommandRPlus => "command-r-plus",
            Model::CommandR => "command-r",
        }
    }

    /// 获取模型的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Model::GPT4Turbo => "GPT-4 Turbo",
            Model::GPT4 => "GPT-4",
            Model::GPT4o => "GPT-4o",
            Model::GPT4oMini => "GPT-4o Mini",
            Model::GPT35Turbo => "GPT-3.5 Turbo",
            Model::Claude3Opus => "Claude 3 Opus",
            Model::Claude35Sonnet => "Claude 3.5 Sonnet",
            Model::Claude3Haiku => "Claude 3 Haiku",
            Model::Claude35SonnetV2 => "Claude 3.5 Sonnet v2",
            Model::GeminiPro => "Gemini Pro",
            Model::GeminiUltra => "Gemini Ultra",
            Model::Gemini15Pro => "Gemini 1.5 Pro",
            Model::DeepSeekV3 => "DeepSeek V3",
            Model::DeepSeekCoder => "DeepSeek Coder",
            Model::MistralLarge => "Mistral Large",
            Model::MistralMedium => "Mistral Medium",
            Model::MistralSmall => "Mistral Small",
            Model::CommandRPlus => "Command R+",
            Model::CommandR => "Command R",
        }
    }

    /// 获取降级模型
    /// 当主模型不可用时，自动降级到备选模型
    pub fn fallback_models(&self) -> Vec<Model> {
        match self {
            // OpenAI 降级链
            Model::GPT4Turbo => vec![Model::GPT4, Model::GPT4o, Model::GPT35Turbo],
            Model::GPT4 => vec![Model::GPT4Turbo, Model::GPT4o, Model::GPT35Turbo],
            Model::GPT4o => vec![Model::GPT4Turbo, Model::GPT35Turbo],
            Model::GPT4oMini => vec![Model::GPT35Turbo, Model::GPT4o],
            Model::GPT35Turbo => vec![Model::GPT4oMini],

            // Anthropic 降级链
            Model::Claude3Opus => vec![Model::Claude35Sonnet, Model::Claude3Haiku],
            Model::Claude35Sonnet => vec![Model::Claude35SonnetV2, Model::Claude3Haiku],
            Model::Claude35SonnetV2 => vec![Model::Claude35Sonnet, Model::Claude3Haiku],
            Model::Claude3Haiku => vec![Model::Claude35Sonnet],

            // Google 降级链
            Model::GeminiUltra => vec![Model::Gemini15Pro, Model::GeminiPro],
            Model::Gemini15Pro => vec![Model::GeminiPro],
            Model::GeminiPro => vec![Model::Gemini15Pro],

            // DeepSeek 降级链
            Model::DeepSeekV3 => vec![Model::DeepSeekCoder],
            Model::DeepSeekCoder => vec![Model::DeepSeekV3],

            // Mistral 降级链
            Model::MistralLarge => vec![Model::MistralMedium, Model::MistralSmall],
            Model::MistralMedium => vec![Model::MistralSmall, Model::MistralLarge],
            Model::MistralSmall => vec![Model::MistralMedium],

            // Cohere 降级链
            Model::CommandRPlus => vec![Model::CommandR],
            Model::CommandR => vec![Model::CommandRPlus],
        }
    }

    /// 获取所有模型列表
    pub fn all() -> Vec<Model> {
        vec![
            // OpenAI
            Model::GPT4Turbo,
            Model::GPT4,
            Model::GPT4o,
            Model::GPT4oMini,
            Model::GPT35Turbo,
            // Anthropic
            Model::Claude3Opus,
            Model::Claude35Sonnet,
            Model::Claude3Haiku,
            Model::Claude35SonnetV2,
            // Google
            Model::GeminiPro,
            Model::GeminiUltra,
            Model::Gemini15Pro,
            // DeepSeek
            Model::DeepSeekV3,
            Model::DeepSeekCoder,
            // Mistral
            Model::MistralLarge,
            Model::MistralMedium,
            Model::MistralSmall,
            // Cohere
            Model::CommandRPlus,
            Model::CommandR,
        ]
    }

    /// 按提供商获取模型列表
    pub fn by_provider(provider: ModelProvider) -> Vec<Model> {
        Self::all()
            .into_iter()
            .filter(|m| m.provider() == provider)
            .collect()
    }

    /// 获取支持的模型数量
    pub fn count() -> usize {
        Self::all().len()
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl FromStr for Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 支持多种格式的模型名称
        let normalized = s.to_lowercase().replace("_", "-").replace(".", "-");

        // 直接匹配
        let model = match normalized.as_str() {
            // OpenAI - 多种格式支持
            "gpt-4-turbo" | "gpt-4-turbo-preview" | "gpt4-turbo" => Model::GPT4Turbo,
            "gpt-4" | "gpt4" => Model::GPT4,
            "gpt-4o" | "gpt4o" => Model::GPT4o,
            "gpt-4o-mini" | "gpt4o-mini" => Model::GPT4oMini,
            "gpt-3-5-turbo" | "gpt-35-turbo" | "gpt35-turbo" | "gpt-3.5-turbo" => Model::GPT35Turbo,

            // Anthropic
            "claude-3-opus" | "claude-3-opus-20240229" | "claude3-opus" => Model::Claude3Opus,
            "claude-3-5-sonnet" | "claude-35-sonnet" | "claude3-5-sonnet" | "claude-3.5-sonnet" => {
                Model::Claude35Sonnet
            }
            "claude-3-haiku" | "claude3-haiku" => Model::Claude3Haiku,
            "claude-3-5-sonnet-v2" | "claude-35-sonnet-v2" | "claude3-5-sonnet-v2" => {
                Model::Claude35SonnetV2
            }

            // Google
            "gemini-pro" | "geminipro" => Model::GeminiPro,
            "gemini-ultra" | "geminiultra" => Model::GeminiUltra,
            "gemini-1-5-pro" | "gemini-15-pro" | "gemini1-5-pro" | "gemini-1.5-pro" => {
                Model::Gemini15Pro
            }

            // DeepSeek
            "deepseek-v3" | "deepseekv3" | "deepseek-chat" => Model::DeepSeekV3,
            "deepseek-coder" | "deepseekcoder" => Model::DeepSeekCoder,

            // Mistral
            "mistral-large" | "mistrallarge" | "mistral-large-latest" => Model::MistralLarge,
            "mistral-medium" | "mistralmedium" | "mistral-medium-latest" => Model::MistralMedium,
            "mistral-small" | "mistralsmall" | "mistral-small-latest" => Model::MistralSmall,

            // Cohere
            "command-r-plus" | "commandrplus" => Model::CommandRPlus,
            "command-r" | "commandr" => Model::CommandR,

            _ => return Err(format!("Unknown model: {s}")),
        };

        Ok(model)
    }
}

/// 模型别名映射
pub fn resolve_model_alias(name: &str) -> Option<Model> {
    let aliases: &[(&str, Model)] = &[
        // OpenAI 别名
        ("gpt4t", Model::GPT4Turbo),
        ("gpt4-turbo", Model::GPT4Turbo),
        ("gpt4turbo", Model::GPT4Turbo),
        ("gpt4", Model::GPT4),
        ("gpt-4o-2024-05-13", Model::GPT4o),
        ("gpt-4o-2024-08-06", Model::GPT4o),
        ("gpt-4o-mini-2024-07-18", Model::GPT4oMini),
        ("gpt35", Model::GPT35Turbo),
        ("gpt-3.5", Model::GPT35Turbo),
        // Anthropic 别名
        ("claude-opus", Model::Claude3Opus),
        ("claude-sonnet", Model::Claude35Sonnet),
        ("claude-haiku", Model::Claude3Haiku),
        ("claude-3.5-sonnet", Model::Claude35Sonnet),
        ("claude-3-5-sonnet-20241022", Model::Claude35Sonnet),
        // Google 别名
        ("gemini", Model::GeminiPro),
        // DeepSeek 别名
        ("deepseek", Model::DeepSeekV3),
        ("ds-v3", Model::DeepSeekV3),
        ("dscoder", Model::DeepSeekCoder),
        // Mistral 别名
        ("mistral", Model::MistralMedium),
        ("mistral-7b", Model::MistralSmall),
        // Cohere 别名
        ("command", Model::CommandR),
        ("command-plus", Model::CommandRPlus),
    ];

    let normalized = name.to_lowercase().replace("_", "-").replace(".", "-");

    for (alias, model) in aliases {
        if alias.to_lowercase() == normalized || alias.to_lowercase() == name.to_lowercase() {
            return Some(*model);
        }
    }

    // 尝试直接解析
    Model::from_str(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_base_url() {
        assert_eq!(ModelProvider::OpenAI.base_url(), "https://api.openai.com");
        assert_eq!(
            ModelProvider::Anthropic.base_url(),
            "https://api.anthropic.com"
        );
    }

    #[test]
    fn test_model_provider() {
        assert_eq!(Model::GPT4Turbo.provider(), ModelProvider::OpenAI);
        assert_eq!(Model::Claude3Opus.provider(), ModelProvider::Anthropic);
        assert_eq!(Model::GeminiPro.provider(), ModelProvider::Google);
    }

    #[test]
    fn test_model_from_str() {
        assert!(Model::from_str("gpt-4").is_ok());
        assert!(Model::from_str("claude-3-opus").is_ok());
        assert!(Model::from_str("unknown-model").is_err());
    }

    #[test]
    fn test_model_fallback() {
        let fallbacks = Model::GPT4Turbo.fallback_models();
        assert!(!fallbacks.is_empty());
        assert!(fallbacks.contains(&Model::GPT4));
    }

    #[test]
    fn test_resolve_alias() {
        assert_eq!(resolve_model_alias("gpt4t"), Some(Model::GPT4Turbo));
        assert_eq!(
            resolve_model_alias("claude-sonnet"),
            Some(Model::Claude35Sonnet)
        );
        assert_eq!(resolve_model_alias("deepseek"), Some(Model::DeepSeekV3));
    }

    #[test]
    fn test_model_count() {
        // 至少 15 个模型
        assert!(Model::count() >= 15);
    }
}
