//! Group Entity - 账号分组管理

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// 分组状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "suspended")]
    Suspended,
}

impl std::fmt::Display for GroupStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupStatus::Active => write!(f, "active"),
            GroupStatus::Inactive => write!(f, "inactive"),
            GroupStatus::Suspended => write!(f, "suspended"),
        }
    }
}

impl From<&str> for GroupStatus {
    fn from(s: &str) -> Self {
        match s {
            "active" => GroupStatus::Active,
            "inactive" => GroupStatus::Inactive,
            "suspended" => GroupStatus::Suspended,
            _ => GroupStatus::Active,
        }
    }
}

/// 平台类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "antigravity")]
    Antigravity,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Anthropic => write!(f, "anthropic"),
            Platform::OpenAI => write!(f, "openai"),
            Platform::Gemini => write!(f, "gemini"),
            Platform::Antigravity => write!(f, "antigravity"),
        }
    }
}

impl From<&str> for Platform {
    fn from(s: &str) -> Self {
        match s {
            "anthropic" => Platform::Anthropic,
            "openai" => Platform::OpenAI,
            "gemini" => Platform::Gemini,
            "antigravity" => Platform::Antigravity,
            _ => Platform::OpenAI,
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub platform: String,
    pub status: String,

    // 配额管理
    pub daily_limit_usd: Option<f64>,
    pub weekly_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,

    // 速率限制
    pub rate_multiplier: f64,

    // 模型路由配置
    pub model_routing: Option<JsonValue>, // HashMap<String, Vec<i64>> -> JSON
    pub model_routing_enabled: bool,

    // 支持的模型系列
    pub supported_model_scopes: Option<JsonValue>, // Vec<String> -> JSON

    // 降级配置
    pub fallback_group_id: Option<i64>,

    // Claude Code 限制
    pub claude_code_only: bool,
    pub fallback_group_id_on_invalid_request: Option<i64>,

    // 排序和显示
    pub sort_order: i32,
    pub is_exclusive: bool,

    // 时间戳
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::accounts::Entity")]
    Accounts,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Accounts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 检查分组是否激活
    pub fn is_active(&self) -> bool {
        self.status == "active" && self.deleted_at.is_none()
    }

    /// 获取平台类型
    pub fn platform_type(&self) -> Platform {
        Platform::from(self.platform.as_str())
    }

    /// 获取状态类型
    pub fn status_type(&self) -> GroupStatus {
        GroupStatus::from(self.status.as_str())
    }

    /// 获取模型路由配置
    pub fn get_model_routing(&self) -> std::collections::HashMap<String, Vec<i64>> {
        self.model_routing
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// 获取支持的模型系列
    pub fn get_supported_model_scopes(&self) -> Vec<String> {
        self.supported_model_scopes
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_else(|| {
                vec![
                    "claude".to_string(),
                    "gemini_text".to_string(),
                    "gemini_image".to_string(),
                ]
            })
    }

    /// 根据请求模型获取路由账号 ID 列表
    /// 返回匹配的优先账号 ID 列表，如果没有匹配规则则返回 None
    pub fn get_routing_account_ids(&self, requested_model: &str) -> Option<Vec<i64>> {
        if !self.model_routing_enabled || self.model_routing.is_none() || requested_model.is_empty()
        {
            return None;
        }

        let routing = self.get_model_routing();

        // 1. 精确匹配优先
        if let Some(account_ids) = routing.get(requested_model) {
            if !account_ids.is_empty() {
                return Some(account_ids.clone());
            }
        }

        // 2. 通配符匹配（前缀匹配）
        for (pattern, account_ids) in &routing {
            if Self::match_model_pattern(pattern, requested_model) && !account_ids.is_empty() {
                return Some(account_ids.clone());
            }
        }

        None
    }

    /// 检查模型是否匹配模式
    /// 支持 * 通配符，如 "claude-opus-*" 匹配 "claude-opus-4-20250514"
    fn match_model_pattern(pattern: &str, model: &str) -> bool {
        if pattern == model {
            return true;
        }

        // 处理 * 通配符（仅支持末尾通配符）
        if let Some(prefix) = pattern.strip_suffix('*') {
            return model.starts_with(prefix);
        }

        false
    }

    /// 检查是否有日限额
    pub fn has_daily_limit(&self) -> bool {
        self.daily_limit_usd.is_some()
    }

    /// 检查是否有月限额
    pub fn has_monthly_limit(&self) -> bool {
        self.monthly_limit_usd.is_some()
    }
}
