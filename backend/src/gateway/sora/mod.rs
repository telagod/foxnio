//! Sora 图片/视频生成模块 - Sora Image/Video Generation Module
//!
//! 提供 Sora API 集成，包括：
//! - 模型配置管理
//! - 图片/视频生成路由
//! - 按次计费功能
//!
//! # 模型类型
//!
//! - **GPT Image**: 图片生成 (gpt-image, gpt-image-landscape, gpt-image-portrait)
//! - **Sora 2**: 标准视频生成 (sora2-landscape-10s, sora2-portrait-10s, ...)
//! - **Sora 2 Pro**: 高级视频生成 (sora2pro-landscape-10s, ...)
//! - **Sora 2 Pro HD**: 高清视频生成 (sora2pro-hd-landscape-10s, ...)
//! - **Prompt Enhance**: 提示词增强 (prompt-enhance-short-10s, ...)
//!
//! # 按次计费
//!
//! - 图片：基础费用 + 尺寸加价
//! - 视频：基础费用 + 时长费用 + HD/Pro 加价
//! - 提示词增强：低价固定费用

pub mod models;
pub mod service;

// 重导出主要类型
pub use models::{
    build_sora_model_families, get_model_config, SoraModelType,
};
pub use service::{
    create_sora_model_list, SoraGenerateRequest, SoraService,
};

/// Sora 模块版本
pub const SORA_MODULE_VERSION: &str = "1.0.0";

/// 检查模型是否为 Sora 模型
pub fn is_sora_model(model_id: &str) -> bool {
    let model_lower = model_id.to_lowercase();
    model_lower.starts_with("gpt-image")
        || model_lower.starts_with("sora")
        || model_lower.starts_with("prompt-enhance")
}

/// 获取 Sora 模型的 API 端点
pub fn get_sora_endpoint(model_id: &str) -> Option<&'static str> {
    let config = get_model_config(model_id)?;
    match config.model_type {
        SoraModelType::Image => Some("/v1/images/generations"),
        SoraModelType::Video => Some("/v1/videos/generations"),
        SoraModelType::PromptEnhance => Some("/v1/prompts/enhance"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sora_model() {
        assert!(is_sora_model("gpt-image"));
        assert!(is_sora_model("sora2-landscape-10s"));
        assert!(is_sora_model("Sora2Pro-HD-Landscape-10s")); // case insensitive
        assert!(is_sora_model("prompt-enhance-short-10s"));
        assert!(!is_sora_model("gpt-4"));
        assert!(!is_sora_model("claude-3-opus"));
    }

    #[test]
    fn test_get_sora_endpoint() {
        assert_eq!(
            get_sora_endpoint("gpt-image"),
            Some("/v1/images/generations")
        );
        assert_eq!(
            get_sora_endpoint("sora2-landscape-10s"),
            Some("/v1/videos/generations")
        );
        assert_eq!(
            get_sora_endpoint("prompt-enhance-short-10s"),
            Some("/v1/prompts/enhance")
        );
        assert_eq!(get_sora_endpoint("gpt-4"), None);
    }

    #[test]
    fn test_module_version() {
        assert!(!SORA_MODULE_VERSION.is_empty());
    }
}
