//! 使用日志 - Usage Log
//!
//! 基于 `usages` 真表提供写入、查询与聚合能力，并将旧链路中的扩展字段落入 metadata。

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as JsonValue};
use std::collections::HashMap;
use uuid::Uuid;

use crate::entity::usages;

/// 使用日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogEntry {
    pub id: Uuid,
    pub request_id: String,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub group_id: Option<i64>,
    pub platform: String,
    pub model: String,
    pub request_type: i16,
    pub stream: bool,
    pub status_code: i16,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cost: i64,
    pub billing_type: i8,
    pub response_time_ms: i64,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 使用日志统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageLogStats {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: i64,
    pub total_cost_yuan: f64,
    pub avg_response_time_ms: f64,
    pub success_rate: f64,
}

/// 使用日志查询参数
#[derive(Debug, Clone)]
pub struct UsageLogQueryParams {
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub group_id: Option<i64>,
    pub platform: Option<String>,
    pub model: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: u64,
    pub offset: u64,
}

/// 使用日志管理器
pub struct UsageLog {
    pub db: DatabaseConnection,
}

impl UsageLog {
    /// 创建新的使用日志管理器
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 插入使用日志
    pub async fn insert(&self, entry: UsageLogEntry) -> Result<Uuid> {
        let user_id = entry
            .user_id
            .ok_or_else(|| anyhow!("usage log requires user_id"))?;
        let api_key_id = entry
            .api_key_id
            .ok_or_else(|| anyhow!("usage log requires api_key_id"))?;

        let active_model = usages::ActiveModel {
            id: Set(entry.id),
            user_id: Set(user_id),
            api_key_id: Set(api_key_id),
            account_id: Set(entry.account_id),
            model: Set(entry.model.clone()),
            input_tokens: Set(entry.prompt_tokens),
            output_tokens: Set(entry.completion_tokens),
            cost: Set(entry.cost),
            request_id: Set((!entry.request_id.is_empty()).then_some(entry.request_id.clone())),
            success: Set(entry.status_code < 400),
            error_message: Set(entry.error_message.clone()),
            metadata: Set(Self::build_metadata(&entry)),
            created_at: Set(entry.created_at),
        };

        let saved = active_model.insert(&self.db).await?;
        Ok(saved.id)
    }

    /// 批量插入使用日志
    pub async fn insert_batch(&self, entries: Vec<UsageLogEntry>) -> Result<Vec<Uuid>> {
        let mut ids = Vec::with_capacity(entries.len());

        for entry in entries {
            ids.push(self.insert(entry).await?);
        }

        Ok(ids)
    }

    /// 查询使用日志
    pub async fn query(&self, params: UsageLogQueryParams) -> Result<Vec<UsageLogEntry>> {
        let mut query = usages::Entity::find().order_by_desc(usages::Column::CreatedAt);

        if let Some(user_id) = params.user_id {
            query = query.filter(usages::Column::UserId.eq(user_id));
        }
        if let Some(api_key_id) = params.api_key_id {
            query = query.filter(usages::Column::ApiKeyId.eq(api_key_id));
        }
        if let Some(account_id) = params.account_id {
            query = query.filter(usages::Column::AccountId.eq(account_id));
        }
        if let Some(model) = &params.model {
            query = query.filter(usages::Column::Model.eq(model.as_str()));
        }
        if let Some(start_time) = params.start_time {
            query = query.filter(usages::Column::CreatedAt.gte(start_time));
        }
        if let Some(end_time) = params.end_time {
            query = query.filter(usages::Column::CreatedAt.lte(end_time));
        }

        let mut entries = query
            .all(&self.db)
            .await?
            .into_iter()
            .map(Self::from_model)
            .collect::<Vec<_>>();

        if let Some(platform) = &params.platform {
            entries.retain(|entry| entry.platform == *platform);
        }
        if let Some(group_id) = params.group_id {
            entries.retain(|entry| entry.group_id == Some(group_id));
        }

        let start = params.offset as usize;
        let end = start
            .saturating_add(params.limit as usize)
            .min(entries.len());
        if start >= entries.len() {
            return Ok(Vec::new());
        }

        Ok(entries[start..end].to_vec())
    }

    /// 获取统计数据
    pub async fn get_stats(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
    ) -> Result<UsageLogStats> {
        let entries = self
            .query(UsageLogQueryParams {
                user_id,
                api_key_id,
                account_id: None,
                group_id: None,
                platform: None,
                model: None,
                start_time: Some(start_time),
                end_time: Some(end_time),
                limit: u64::MAX,
                offset: 0,
            })
            .await?;

        Ok(Self::aggregate(&entries))
    }

    /// 按平台分组统计
    pub async fn get_stats_by_platform(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<HashMap<String, UsageLogStats>> {
        let entries = self
            .query(UsageLogQueryParams {
                user_id: None,
                api_key_id: None,
                account_id: None,
                group_id: None,
                platform: None,
                model: None,
                start_time: Some(start_time),
                end_time: Some(end_time),
                limit: u64::MAX,
                offset: 0,
            })
            .await?;

        Ok(Self::group_entries(entries, |entry| entry.platform.clone()))
    }

    /// 按模型分组统计
    pub async fn get_stats_by_model(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<HashMap<String, UsageLogStats>> {
        let entries = self
            .query(UsageLogQueryParams {
                user_id: None,
                api_key_id: None,
                account_id: None,
                group_id: None,
                platform: None,
                model: None,
                start_time: Some(start_time),
                end_time: Some(end_time),
                limit: u64::MAX,
                offset: 0,
            })
            .await?;

        Ok(Self::group_entries(entries, |entry| entry.model.clone()))
    }

    /// 按用户分组统计
    pub async fn get_stats_by_user(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<HashMap<Uuid, UsageLogStats>> {
        let entries = self
            .query(UsageLogQueryParams {
                user_id: None,
                api_key_id: None,
                account_id: None,
                group_id: None,
                platform: None,
                model: None,
                start_time: Some(start_time),
                end_time: Some(end_time),
                limit: u64::MAX,
                offset: 0,
            })
            .await?;

        Ok(Self::group_entries_optional(entries, |entry| entry.user_id))
    }

    /// 获取用户排行榜
    pub async fn get_user_leaderboard(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        limit: u64,
    ) -> Result<Vec<(Uuid, UsageLogStats)>> {
        let mut rows = self
            .get_stats_by_user(start_time, end_time)
            .await?
            .into_iter()
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| {
            b.1.total_cost
                .cmp(&a.1.total_cost)
                .then_with(|| b.1.total_tokens.cmp(&a.1.total_tokens))
                .then_with(|| b.1.total_requests.cmp(&a.1.total_requests))
        });
        rows.truncate(limit as usize);

        Ok(rows)
    }

    /// 获取模型排行榜
    pub async fn get_model_leaderboard(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        limit: u64,
    ) -> Result<Vec<(String, UsageLogStats)>> {
        let mut rows = self
            .get_stats_by_model(start_time, end_time)
            .await?
            .into_iter()
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| {
            b.1.total_cost
                .cmp(&a.1.total_cost)
                .then_with(|| b.1.total_tokens.cmp(&a.1.total_tokens))
                .then_with(|| b.1.total_requests.cmp(&a.1.total_requests))
        });
        rows.truncate(limit as usize);

        Ok(rows)
    }

    fn build_metadata(entry: &UsageLogEntry) -> Option<JsonValue> {
        let mut metadata = Map::new();
        metadata.insert("platform".to_string(), json!(entry.platform));
        metadata.insert("request_type".to_string(), json!(entry.request_type));
        metadata.insert("stream".to_string(), json!(entry.stream));
        metadata.insert("status_code".to_string(), json!(entry.status_code));
        metadata.insert("billing_type".to_string(), json!(entry.billing_type));
        metadata.insert(
            "response_time_ms".to_string(),
            json!(entry.response_time_ms),
        );
        metadata.insert("total_tokens".to_string(), json!(entry.total_tokens));

        if let Some(group_id) = entry.group_id {
            metadata.insert("group_id".to_string(), json!(group_id));
        }

        if metadata.is_empty() {
            None
        } else {
            Some(JsonValue::Object(metadata))
        }
    }

    fn from_model(model: usages::Model) -> UsageLogEntry {
        let usages::Model {
            id,
            user_id,
            api_key_id,
            account_id,
            model,
            input_tokens,
            output_tokens,
            cost,
            request_id,
            success,
            error_message,
            metadata,
            created_at,
        } = model;

        let metadata = metadata.unwrap_or(JsonValue::Object(Map::new()));
        let total_tokens =
            Self::metadata_i64(&metadata, "total_tokens").unwrap_or(input_tokens + output_tokens);
        let request_id = request_id.unwrap_or_default();
        let platform = Self::metadata_string(&metadata, "platform").unwrap_or_default();
        let request_type = Self::metadata_i64(&metadata, "request_type").unwrap_or_default() as i16;
        let stream = Self::metadata_bool(&metadata, "stream").unwrap_or(false);
        let status_code = Self::metadata_i64(&metadata, "status_code").unwrap_or_else(|| {
            if success {
                200
            } else {
                500
            }
        }) as i16;
        let billing_type = Self::metadata_i64(&metadata, "billing_type").unwrap_or_default() as i8;
        let response_time_ms =
            Self::metadata_i64(&metadata, "response_time_ms").unwrap_or_default();

        UsageLogEntry {
            id,
            request_id,
            user_id: Some(user_id),
            api_key_id: Some(api_key_id),
            account_id,
            group_id: Self::metadata_i64(&metadata, "group_id"),
            platform,
            model,
            request_type,
            stream,
            status_code,
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens,
            cost,
            billing_type,
            response_time_ms,
            error_message,
            created_at,
        }
    }

    fn metadata_string(metadata: &JsonValue, key: &str) -> Option<String> {
        metadata.get(key)?.as_str().map(ToOwned::to_owned)
    }

    fn metadata_bool(metadata: &JsonValue, key: &str) -> Option<bool> {
        metadata.get(key)?.as_bool()
    }

    fn metadata_i64(metadata: &JsonValue, key: &str) -> Option<i64> {
        metadata.get(key)?.as_i64()
    }

    fn aggregate(entries: &[UsageLogEntry]) -> UsageLogStats {
        if entries.is_empty() {
            return UsageLogStats::default();
        }

        let total_requests = entries.len() as i64;
        let total_tokens = entries.iter().map(|entry| entry.total_tokens).sum::<i64>();
        let total_cost = entries.iter().map(|entry| entry.cost).sum::<i64>();
        let total_response_time = entries
            .iter()
            .map(|entry| entry.response_time_ms)
            .sum::<i64>();
        let success_count = entries
            .iter()
            .filter(|entry| entry.status_code < 400)
            .count() as i64;

        UsageLogStats {
            total_requests,
            total_tokens,
            total_cost,
            total_cost_yuan: total_cost as f64 / 100.0,
            avg_response_time_ms: total_response_time as f64 / total_requests as f64,
            success_rate: success_count as f64 / total_requests as f64,
        }
    }

    fn group_entries<K, F>(entries: Vec<UsageLogEntry>, key_fn: F) -> HashMap<K, UsageLogStats>
    where
        K: Eq + std::hash::Hash,
        F: Fn(&UsageLogEntry) -> K,
    {
        let mut grouped: HashMap<K, Vec<UsageLogEntry>> = HashMap::new();

        for entry in entries {
            grouped.entry(key_fn(&entry)).or_default().push(entry);
        }

        grouped
            .into_iter()
            .map(|(key, entries)| (key, Self::aggregate(&entries)))
            .collect()
    }

    fn group_entries_optional<K, F>(
        entries: Vec<UsageLogEntry>,
        key_fn: F,
    ) -> HashMap<K, UsageLogStats>
    where
        K: Eq + std::hash::Hash,
        F: Fn(&UsageLogEntry) -> Option<K>,
    {
        let mut grouped: HashMap<K, Vec<UsageLogEntry>> = HashMap::new();

        for entry in entries {
            if let Some(key) = key_fn(&entry) {
                grouped.entry(key).or_default().push(entry);
            }
        }

        grouped
            .into_iter()
            .map(|(key, entries)| (key, Self::aggregate(&entries)))
            .collect()
    }
}

/// 使用日志构建器
pub struct UsageLogBuilder {
    entry: UsageLogEntry,
}

impl UsageLogBuilder {
    /// 创建新的构建器
    pub fn new(request_id: String, platform: String, model: String) -> Self {
        Self {
            entry: UsageLogEntry {
                id: Uuid::new_v4(),
                request_id,
                user_id: None,
                api_key_id: None,
                account_id: None,
                group_id: None,
                platform,
                model,
                request_type: 0,
                stream: false,
                status_code: 200,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                cost: 0,
                billing_type: 0,
                response_time_ms: 0,
                error_message: None,
                created_at: Utc::now(),
            },
        }
    }

    /// 设置用户 ID
    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.entry.user_id = Some(user_id);
        self
    }

    /// 设置 API Key ID
    pub fn api_key_id(mut self, api_key_id: Uuid) -> Self {
        self.entry.api_key_id = Some(api_key_id);
        self
    }

    /// 设置账号 ID
    pub fn account_id(mut self, account_id: Uuid) -> Self {
        self.entry.account_id = Some(account_id);
        self
    }

    /// 设置分组 ID
    pub fn group_id(mut self, group_id: i64) -> Self {
        self.entry.group_id = Some(group_id);
        self
    }

    /// 设置请求类型
    pub fn request_type(mut self, request_type: i16) -> Self {
        self.entry.request_type = request_type;
        self
    }

    /// 设置是否流式
    pub fn stream(mut self, stream: bool) -> Self {
        self.entry.stream = stream;
        self
    }

    /// 设置状态码
    pub fn status_code(mut self, status_code: i16) -> Self {
        self.entry.status_code = status_code;
        self
    }

    /// 设置 tokens
    pub fn tokens(mut self, prompt: i64, completion: i64) -> Self {
        self.entry.prompt_tokens = prompt;
        self.entry.completion_tokens = completion;
        self.entry.total_tokens = prompt + completion;
        self
    }

    /// 设置成本（单位：分）
    pub fn cost(mut self, cost: i64) -> Self {
        self.entry.cost = cost;
        self
    }

    /// 设置计费类型
    pub fn billing_type(mut self, billing_type: i8) -> Self {
        self.entry.billing_type = billing_type;
        self
    }

    /// 设置响应时间
    pub fn response_time_ms(mut self, response_time_ms: i64) -> Self {
        self.entry.response_time_ms = response_time_ms;
        self
    }

    /// 设置错误信息
    pub fn error_message(mut self, error_message: impl Into<String>) -> Self {
        self.entry.error_message = Some(error_message.into());
        self
    }

    /// 构建条目
    pub fn build(self) -> UsageLogEntry {
        self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_log_builder() {
        let user_id = Uuid::new_v4();
        let api_key_id = Uuid::new_v4();

        let entry = UsageLogBuilder::new(
            "req_123".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
        )
        .user_id(user_id)
        .api_key_id(api_key_id)
        .tokens(100, 200)
        .cost(12)
        .response_time_ms(500)
        .build();

        assert_eq!(entry.request_id, "req_123");
        assert_eq!(entry.platform, "openai");
        assert_eq!(entry.model, "gpt-4");
        assert_eq!(entry.user_id, Some(user_id));
        assert_eq!(entry.api_key_id, Some(api_key_id));
        assert_eq!(entry.total_tokens, 300);
        assert_eq!(entry.cost, 12);
    }
}
