//! Sora 模型配置 - Sora Model Configurations
//!
//! 参考 Sub2API 的 sora_models.go 实现
//! 支持 GPT Image 和 Sora 2/Pro 视频生成模型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sora 模型类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoraModelType {
    /// 图片生成
    Image,
    /// 视频生成
    Video,
    /// 提示词增强
    PromptEnhance,
}

/// Sora 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraModelConfig {
    /// 模型类型
    pub model_type: SoraModelType,
    /// 图片宽度
    pub width: Option<u32>,
    /// 图片高度
    pub height: Option<u32>,
    /// 视频方向 (landscape/portrait)
    pub orientation: Option<String>,
    /// 视频帧数
    pub frames: Option<u32>,
    /// Sora 模型标识 (sy_8, sy_ore)
    pub model: Option<String>,
    /// 视频尺寸 (small/large)
    pub size: Option<String>,
    /// 是否需要 Pro 账号
    pub require_pro: bool,
    /// 提示词增强级别 (short/medium/long)
    pub expansion_level: Option<String>,
    /// 提示词增强时长（秒）
    pub duration_s: Option<u32>,
}

impl SoraModelConfig {
    /// 获取视频时长（秒）
    pub fn duration_seconds(&self) -> Option<u32> {
        self.frames.map(|f| f / 30) // 30fps
    }

    /// 获取价格倍率
    pub fn price_multiplier(&self) -> f64 {
        match self.model_type {
            SoraModelType::Image => 1.0,
            SoraModelType::Video => {
                let base = if self.size.as_deref() == Some("large") {
                    2.0 // HD 双倍价格
                } else {
                    1.0
                };
                let pro_multiplier = if self.require_pro { 1.5 } else { 1.0 };
                base * pro_multiplier
            }
            SoraModelType::PromptEnhance => 0.1,
        }
    }
}

/// 获取所有 Sora 模型配置
pub fn get_sora_model_configs() -> HashMap<&'static str, SoraModelConfig> {
    let mut configs = HashMap::new();

    // GPT Image 模型
    configs.insert(
        "gpt-image",
        SoraModelConfig {
            model_type: SoraModelType::Image,
            width: Some(360),
            height: Some(360),
            orientation: None,
            frames: None,
            model: None,
            size: None,
            require_pro: false,
            expansion_level: None,
            duration_s: None,
        },
    );

    configs.insert(
        "gpt-image-landscape",
        SoraModelConfig {
            model_type: SoraModelType::Image,
            width: Some(540),
            height: Some(360),
            orientation: Some("landscape".to_string()),
            frames: None,
            model: None,
            size: None,
            require_pro: false,
            expansion_level: None,
            duration_s: None,
        },
    );

    configs.insert(
        "gpt-image-portrait",
        SoraModelConfig {
            model_type: SoraModelType::Image,
            width: Some(360),
            height: Some(540),
            orientation: Some("portrait".to_string()),
            frames: None,
            model: None,
            size: None,
            require_pro: false,
            expansion_level: None,
            duration_s: None,
        },
    );

    // Sora 2 标准模型 (sy_8)
    for duration in [10, 15, 25] {
        for orientation in ["landscape", "portrait"] {
            let model_id = format!("sora2-{}-{}s", orientation, duration);
            let frames = duration * 30;
            let require_pro = duration == 25;

            configs.insert(
                Box::leak(model_id.into_boxed_str()),
                SoraModelConfig {
                    model_type: SoraModelType::Video,
                    width: None,
                    height: None,
                    orientation: Some(orientation.to_string()),
                    frames: Some(frames),
                    model: Some("sy_8".to_string()),
                    size: Some("small".to_string()),
                    require_pro,
                    expansion_level: None,
                    duration_s: Some(duration),
                },
            );
        }
    }

    // Sora 2 Pro 模型 (sy_ore)
    for duration in [10, 15, 25] {
        for orientation in ["landscape", "portrait"] {
            let model_id = format!("sora2pro-{}-{}s", orientation, duration);
            let frames = duration * 30;

            configs.insert(
                Box::leak(model_id.into_boxed_str()),
                SoraModelConfig {
                    model_type: SoraModelType::Video,
                    width: None,
                    height: None,
                    orientation: Some(orientation.to_string()),
                    frames: Some(frames),
                    model: Some("sy_ore".to_string()),
                    size: Some("small".to_string()),
                    require_pro: true,
                    expansion_level: None,
                    duration_s: Some(duration),
                },
            );
        }
    }

    // Sora 2 Pro HD 模型 (sy_ore + large)
    for duration in [10, 15] {
        for orientation in ["landscape", "portrait"] {
            let model_id = format!("sora2pro-hd-{}-{}s", orientation, duration);
            let frames = duration * 30;

            configs.insert(
                Box::leak(model_id.into_boxed_str()),
                SoraModelConfig {
                    model_type: SoraModelType::Video,
                    width: None,
                    height: None,
                    orientation: Some(orientation.to_string()),
                    frames: Some(frames),
                    model: Some("sy_ore".to_string()),
                    size: Some("large".to_string()),
                    require_pro: true,
                    expansion_level: None,
                    duration_s: Some(duration),
                },
            );
        }
    }

    // Prompt Enhance 模型
    for expansion_level in ["short", "medium", "long"] {
        for duration in [10, 15, 20] {
            let model_id = format!("prompt-enhance-{}-{}s", expansion_level, duration);

            configs.insert(
                Box::leak(model_id.into_boxed_str()),
                SoraModelConfig {
                    model_type: SoraModelType::PromptEnhance,
                    width: None,
                    height: None,
                    orientation: None,
                    frames: None,
                    model: None,
                    size: None,
                    require_pro: false,
                    expansion_level: Some(expansion_level.to_string()),
                    duration_s: Some(duration),
                },
            );
        }
    }

    configs
}

/// 获取模型配置
pub fn get_model_config(model_id: &str) -> Option<SoraModelConfig> {
    let configs = get_sora_model_configs();
    let key = model_id.to_lowercase();
    configs.get(key.as_str()).cloned()
}

/// 获取所有模型 ID 列表
pub fn get_sora_model_ids() -> Vec<&'static str> {
    vec![
        // Image models
        "gpt-image",
        "gpt-image-landscape",
        "gpt-image-portrait",
        // Sora 2 standard
        "sora2-landscape-10s",
        "sora2-portrait-10s",
        "sora2-landscape-15s",
        "sora2-portrait-15s",
        "sora2-landscape-25s",
        "sora2-portrait-25s",
        // Sora 2 Pro
        "sora2pro-landscape-10s",
        "sora2pro-portrait-10s",
        "sora2pro-landscape-15s",
        "sora2pro-portrait-15s",
        "sora2pro-landscape-25s",
        "sora2pro-portrait-25s",
        // Sora 2 Pro HD
        "sora2pro-hd-landscape-10s",
        "sora2pro-hd-portrait-10s",
        "sora2pro-hd-landscape-15s",
        "sora2pro-hd-portrait-15s",
        // Prompt Enhance
        "prompt-enhance-short-10s",
        "prompt-enhance-short-15s",
        "prompt-enhance-short-20s",
        "prompt-enhance-medium-10s",
        "prompt-enhance-medium-15s",
        "prompt-enhance-medium-20s",
        "prompt-enhance-long-10s",
        "prompt-enhance-long-15s",
        "prompt-enhance-long-20s",
    ]
}

/// 模型家族（前端展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraModelFamily {
    /// 家族 ID
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 模型类型
    pub model_type: SoraModelType,
    /// 支持的方向
    pub orientations: Vec<String>,
    /// 支持的时长（视频）
    pub durations: Vec<u32>,
}

/// 家族名称映射
pub fn get_family_names() -> HashMap<&'static str, &'static str> {
    let mut names = HashMap::new();
    names.insert("sora2", "Sora 2");
    names.insert("sora2pro", "Sora 2 Pro");
    names.insert("sora2pro-hd", "Sora 2 Pro HD");
    names.insert("gpt-image", "GPT Image");
    names
}

/// 构建模型家族列表
pub fn build_sora_model_families() -> Vec<SoraModelFamily> {
    use regex::Regex;
    use std::collections::HashSet;

    let video_suffix_re = Regex::new(r"-(landscape|portrait)-(\d+)s$").unwrap();
    let image_suffix_re = Regex::new(r"-(landscape|portrait)$").unwrap();
    let family_names = get_family_names();

    // 收集家族数据
    let mut families: HashMap<String, (SoraModelType, HashSet<String>, HashSet<u32>)> =
        HashMap::new();

    for (id, config) in get_sora_model_configs() {
        if config.model_type == SoraModelType::PromptEnhance {
            continue;
        }

        let (fam_id, orientation, duration) = match config.model_type {
            SoraModelType::Video => {
                if let Some(caps) = video_suffix_re.captures(id) {
                    let fam_id = &id[..id.len() - caps[0].len()];
                    let orientation = caps[1].to_string();
                    let duration: u32 = caps[2].parse().unwrap_or(0);
                    (fam_id.to_string(), Some(orientation), Some(duration))
                } else {
                    continue;
                }
            }
            SoraModelType::Image => {
                if let Some(caps) = image_suffix_re.captures(id) {
                    let fam_id = &id[..id.len() - caps[0].len()];
                    let orientation = caps[1].to_string();
                    (fam_id.to_string(), Some(orientation), None)
                } else {
                    (id.to_string(), Some("square".to_string()), None)
                }
            }
            _ => continue,
        };

        let entry = families
            .entry(fam_id)
            .or_insert_with(|| (config.model_type, HashSet::new(), HashSet::new()));

        if let Some(o) = orientation {
            entry.1.insert(o);
        }
        if let Some(d) = duration {
            entry.2.insert(d);
        }
    }

    // 构建结果
    let mut result: Vec<SoraModelFamily> = families
        .into_iter()
        .map(|(id, (model_type, orientations, durations))| {
            let mut orientations: Vec<String> = orientations.into_iter().collect();
            orientations.sort();

            let mut durations: Vec<u32> = durations.into_iter().collect();
            durations.sort();

            let name = family_names
                .get(id.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| id.clone());

            SoraModelFamily {
                id,
                name,
                model_type,
                orientations,
                durations,
            }
        })
        .collect();

    // 排序：视频在前、图像在后
    result.sort_by(|a, b| {
        if a.model_type != b.model_type {
            if a.model_type == SoraModelType::Video {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        } else {
            a.id.cmp(&b.id)
        }
    });

    result
}

/// 默认模型列表（用于 /v1/models 响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoraModel {
    pub id: String,
    pub object: String,
    pub owned_by: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub display_name: String,
}

impl SoraModel {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            object: "model".to_string(),
            owned_by: "openai".to_string(),
            model_type: "model".to_string(),
            display_name: id.to_string(),
        }
    }
}

/// 获取默认模型列表
pub fn default_sora_models(hide_prompt_enhance: bool) -> Vec<SoraModel> {
    get_sora_model_ids()
        .into_iter()
        .filter(|id| !hide_prompt_enhance || !id.starts_with("prompt-enhance"))
        .map(SoraModel::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_config() {
        let config = get_model_config("gpt-image").unwrap();
        assert_eq!(config.model_type, SoraModelType::Image);
        assert_eq!(config.width, Some(360));

        let config = get_model_config("sora2-landscape-10s").unwrap();
        assert_eq!(config.model_type, SoraModelType::Video);
        assert_eq!(config.frames, Some(300));
        assert_eq!(config.model, Some("sy_8".to_string()));
    }

    #[test]
    fn test_price_multiplier() {
        let config = get_model_config("gpt-image").unwrap();
        assert_eq!(config.price_multiplier(), 1.0);

        let config = get_model_config("sora2pro-hd-landscape-10s").unwrap();
        assert_eq!(config.price_multiplier(), 3.0); // HD (2.0) * Pro (1.5)
    }

    #[test]
    fn test_build_model_families() {
        let families = build_sora_model_families();
        assert!(!families.is_empty());

        // 视频模型应该排在前面
        let first_family = &families[0];
        assert_eq!(first_family.model_type, SoraModelType::Video);
    }

    #[test]
    fn test_default_sora_models() {
        let models = default_sora_models(false);
        assert!(models.len() > 20);

        let models_filtered = default_sora_models(true);
        assert!(models_filtered.len() < models.len());
    }
}
