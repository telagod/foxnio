//! 高性能批量导入服务
//!
//! 针对几千到几万账号的批量导入优化：
//! - 真正的批量 SQL INSERT（使用 insert_many）
//! - 分批处理，避免内存溢出
//! - 并行凭证验证
//! - 事务保证数据一致性
//! - 进度回调支持

use anyhow::Result;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::entity::accounts;
use crate::metrics::{batch_throughput, BatchMetrics};
use crate::utils::encryption_global::GlobalEncryption;

/// 批量导入配置
#[derive(Debug, Clone)]
pub struct BatchImportConfig {
    /// 每批次大小（数据库插入）
    pub batch_size: usize,
    /// 并发验证数
    pub validation_concurrency: usize,
    /// 是否跳过重复账号
    pub skip_duplicates: bool,
    /// 是否在错误时继续
    pub continue_on_error: bool,
}

impl Default for BatchImportConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,           // 每批 1000 条
            validation_concurrency: 50, // 并发验证 50 个
            skip_duplicates: true,
            continue_on_error: true,
        }
    }
}

/// 导入项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAccountItem {
    /// 账号名称
    pub name: String,
    /// 提供商 (anthropic, openai, gemini, etc.)
    pub provider: String,
    /// 凭证类型 (api_key, oauth, etc.)
    #[serde(default = "default_credential_type")]
    pub credential_type: String,
    /// 凭证值
    pub credential: String,
    /// 优先级 (默认 50)
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// 并发限制
    #[serde(default = "default_concurrent_limit")]
    pub concurrent_limit: Option<i32>,
    /// 每分钟请求限制
    #[serde(default)]
    pub rate_limit_rpm: Option<i32>,
    /// 分组 ID
    #[serde(default)]
    pub group_id: Option<i64>,
}

fn default_credential_type() -> String {
    "api_key".to_string()
}

fn default_priority() -> i32 {
    50
}

fn default_concurrent_limit() -> Option<i32> {
    Some(5)
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub index: usize,
    pub valid: bool,
    pub error: Option<String>,
}

/// 导入结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// 总数
    pub total: usize,
    /// 成功导入数
    pub imported: usize,
    /// 跳过数（重复等）
    pub skipped: usize,
    /// 失败数
    pub failed: usize,
    /// 成功导入的账号 ID
    pub account_ids: Vec<String>,
    /// 错误列表
    pub errors: Vec<ImportError>,
    /// 处理耗时（毫秒）
    pub duration_ms: u64,
    /// 吞吐（items/s）
    pub throughput_items_per_sec: f64,
    /// Provider 维度导入汇总
    pub providers: Vec<ImportResultProvider>,
}

/// 导入预检结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    /// 原始输入总数
    pub total: usize,
    /// 通过校验数
    pub valid: usize,
    /// 校验失败数
    pub invalid: usize,
    /// 命中已存在名称的重复数
    pub duplicate: usize,
    /// 预计实际导入数
    pub will_import: usize,
    /// 是否跳过重复
    pub skip_duplicates: bool,
    /// 是否为 fast mode
    pub fast_mode: bool,
    /// 实际生效批次大小
    pub batch_size: usize,
    /// 实际生效验证并发
    pub validation_concurrency: usize,
    /// 处理耗时（毫秒）
    pub duration_ms: u64,
    /// 吞吐（items/s）
    pub throughput_items_per_sec: f64,
    /// Provider 维度预估
    pub providers: Vec<ImportPreviewProvider>,
    /// 错误样本
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewProvider {
    pub provider: String,
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub duplicate: usize,
    pub will_import: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResultProvider {
    pub provider: String,
    pub total: usize,
    pub imported: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// 导入错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportError {
    pub index: usize,
    pub name: String,
    pub error: String,
}

/// 进度更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub phase: ImportPhase,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub percentage: f64,
    pub message: String,
}

/// 导入阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportPhase {
    Validating,
    CheckingDuplicates,
    Importing,
    Completed,
}

/// 高性能批量导入服务
pub struct BatchImportService {
    db: DatabaseConnection,
    config: BatchImportConfig,
}

impl BatchImportService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            config: BatchImportConfig::default(),
        }
    }

    pub fn with_config(db: DatabaseConnection, config: BatchImportConfig) -> Self {
        Self { db, config }
    }

    /// 批量导入预检（不落库）
    pub async fn preview_import(
        &self,
        items: &[ImportAccountItem],
        fast_mode: bool,
    ) -> Result<ImportPreview> {
        let start = std::time::Instant::now();
        let total = items.len();

        let validation_meta = if fast_mode {
            items
                .iter()
                .enumerate()
                .map(|(index, _)| ValidationResult {
                    index,
                    valid: true,
                    error: None,
                })
                .collect::<Vec<_>>()
        } else {
            self.collect_validation_meta(items).await
        };

        let mut existing_name_set = std::collections::HashSet::new();
        if self.config.skip_duplicates {
            let names: Vec<&str> = items
                .iter()
                .zip(validation_meta.iter())
                .filter_map(|(item, meta)| meta.valid.then_some(item.name.as_str()))
                .collect();

            if !names.is_empty() {
                let existing: Vec<String> = accounts::Entity::find()
                    .select_only()
                    .column(accounts::Column::Name)
                    .filter(accounts::Column::Name.is_in(names))
                    .into_tuple()
                    .all(&self.db)
                    .await?;
                existing_name_set = existing
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>();
            }
        }

        let mut provider_stats = std::collections::BTreeMap::<String, ImportPreviewProvider>::new();
        let mut errors = Vec::new();
        let mut valid = 0usize;
        let mut invalid = 0usize;
        let mut duplicate = 0usize;
        let mut will_import = 0usize;

        for (idx, item) in items.iter().enumerate() {
            let stat = provider_stats
                .entry(item.provider.clone())
                .or_insert_with(|| ImportPreviewProvider {
                    provider: item.provider.clone(),
                    total: 0,
                    valid: 0,
                    invalid: 0,
                    duplicate: 0,
                    will_import: 0,
                });
            stat.total += 1;

            let meta = &validation_meta[idx];
            if meta.valid {
                valid += 1;
                stat.valid += 1;

                let is_duplicate =
                    self.config.skip_duplicates && existing_name_set.contains(&item.name);
                if is_duplicate {
                    duplicate += 1;
                    stat.duplicate += 1;
                } else {
                    will_import += 1;
                    stat.will_import += 1;
                }
            } else {
                invalid += 1;
                stat.invalid += 1;
                errors.push(ImportError {
                    index: idx,
                    name: item.name.clone(),
                    error: meta
                        .error
                        .clone()
                        .unwrap_or_else(|| "Invalid item".to_string()),
                });
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        BatchMetrics::record(
            "fast_import_preview",
            if fast_mode { "fast" } else { "validated" },
            total,
            invalid,
            duration_ms,
        );

        Ok(ImportPreview {
            total,
            valid,
            invalid,
            duplicate,
            will_import,
            skip_duplicates: self.config.skip_duplicates,
            fast_mode,
            batch_size: self.config.batch_size,
            validation_concurrency: self.config.validation_concurrency,
            duration_ms,
            throughput_items_per_sec: batch_throughput(total, duration_ms),
            providers: provider_stats.into_values().collect(),
            errors: errors.into_iter().take(20).collect(),
        })
    }

    /// 批量导入账号（带进度回调）
    pub async fn import_accounts_with_progress(
        self,
        items: Vec<ImportAccountItem>,
        progress_tx: Option<mpsc::Sender<ImportProgress>>,
    ) -> Result<ImportResult> {
        let start = std::time::Instant::now();
        let total = items.len();

        // 发送进度辅助函数
        let send_progress =
            |phase: ImportPhase, processed: usize, succeeded: usize, failed: usize, msg: &str| {
                if let Some(tx) = &progress_tx {
                    let progress = ImportProgress {
                        phase,
                        total,
                        processed,
                        succeeded,
                        failed,
                        percentage: if total > 0 {
                            (processed as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        },
                        message: msg.to_string(),
                    };
                    // 忽略发送错误（接收端可能已关闭）
                    let _ = tx.try_send(progress);
                }
            };

        // 阶段 1: 并行验证
        send_progress(ImportPhase::Validating, 0, 0, 0, "验证账号格式...");

        let validation_meta = self.collect_validation_meta(&items).await;

        let mut valid_items = Vec::new();
        for (idx, item) in items.iter().enumerate() {
            if let Some(meta) = validation_meta.get(idx) {
                if meta.valid {
                    valid_items.push((idx, item.clone()));
                }
            }
        }

        let validation_errors: Vec<ImportError> = validation_meta
            .iter()
            .enumerate()
            .filter_map(|(idx, result)| {
                if result.valid {
                    None
                } else {
                    Some(ImportError {
                        index: idx,
                        name: items[idx].name.clone(),
                        error: result.error.clone().unwrap_or_default(),
                    })
                }
            })
            .collect();

        send_progress(
            ImportPhase::Validating,
            total,
            valid_items.len(),
            validation_errors.len(),
            &format!("验证完成，{} 个有效", valid_items.len()),
        );

        // 阶段 2: 检查重复
        send_progress(ImportPhase::CheckingDuplicates, 0, 0, 0, "检查重复账号...");

        let unique_items = if self.config.skip_duplicates {
            self.filter_duplicates_batch(&valid_items).await?
        } else {
            valid_items.clone()
        };

        let skipped = valid_items.len() - unique_items.len();
        let unique_indices = unique_items
            .iter()
            .map(|(idx, _)| *idx)
            .collect::<std::collections::HashSet<_>>();

        send_progress(
            ImportPhase::CheckingDuplicates,
            total,
            unique_items.len(),
            skipped,
            &format!("去重完成，跳过 {} 个重复", skipped),
        );

        // 阶段 3: 批量导入
        send_progress(ImportPhase::Importing, 0, 0, 0, "开始导入...");

        let (imported_ids, import_errors) = self
            .import_batch_with_progress(unique_items, |processed| {
                send_progress(
                    ImportPhase::Importing,
                    processed,
                    0,
                    0,
                    &format!("导入中... {}/{}", processed, total),
                );
            })
            .await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let import_error_indices = import_errors
            .iter()
            .map(|error| error.index)
            .collect::<std::collections::HashSet<_>>();
        let mut provider_stats = std::collections::BTreeMap::<String, ImportResultProvider>::new();

        for (idx, item) in items.iter().enumerate() {
            let stat = provider_stats
                .entry(item.provider.clone())
                .or_insert_with(|| ImportResultProvider {
                    provider: item.provider.clone(),
                    total: 0,
                    imported: 0,
                    skipped: 0,
                    failed: 0,
                });
            stat.total += 1;

            let meta = &validation_meta[idx];
            if !meta.valid {
                stat.failed += 1;
                continue;
            }

            if !unique_indices.contains(&idx) {
                stat.skipped += 1;
                continue;
            }

            if import_error_indices.contains(&idx) {
                stat.failed += 1;
            } else {
                stat.imported += 1;
            }
        }

        let result = ImportResult {
            total,
            imported: imported_ids.len(),
            skipped,
            failed: validation_errors.len() + import_errors.len(),
            account_ids: imported_ids,
            errors: [validation_errors, import_errors].concat(),
            duration_ms,
            throughput_items_per_sec: batch_throughput(total, duration_ms),
            providers: provider_stats.into_values().collect(),
        };

        BatchMetrics::record(
            "fast_import",
            "validated",
            total,
            result.failed,
            result.duration_ms,
        );

        send_progress(
            ImportPhase::Completed,
            total,
            result.imported,
            result.failed,
            &format!(
                "导入完成: 成功 {}, 跳过 {}, 失败 {}, 耗时 {}ms",
                result.imported, result.skipped, result.failed, result.duration_ms
            ),
        );

        Ok(result)
    }

    /// 批量导入账号（简化版，无进度回调）
    pub async fn import_accounts(self, items: Vec<ImportAccountItem>) -> Result<ImportResult> {
        self.import_accounts_with_progress(items, None).await
    }

    /// 并行验证账号项
    async fn validate_items_parallel(&self, items: &[ImportAccountItem]) -> Vec<ValidationResult> {
        let concurrency = self.config.validation_concurrency;
        stream::iter(items.iter().cloned().enumerate())
            .map(|(index, item)| async move {
                let (valid, error) = self.validate_item(&item).await;
                ValidationResult {
                    index,
                    valid,
                    error,
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await
    }

    async fn collect_validation_meta(&self, items: &[ImportAccountItem]) -> Vec<ValidationResult> {
        let mut validation_results = self.validate_items_parallel(items).await;
        validation_results.sort_by_key(|result| result.index);

        let mut validation_meta = vec![
            ValidationResult {
                index: 0,
                valid: false,
                error: Some("Validation result missing".to_string()),
            };
            items.len()
        ];

        for result in validation_results {
            let idx = result.index;
            if idx < items.len() {
                validation_meta[idx] = result;
            }
        }

        validation_meta
    }

    /// 验证单个账号项
    async fn validate_item(&self, item: &ImportAccountItem) -> (bool, Option<String>) {
        // 名称验证
        if item.name.is_empty() {
            return (false, Some("名称不能为空".to_string()));
        }
        if item.name.len() > 255 {
            return (false, Some("名称过长（最大 255 字符）".to_string()));
        }

        // 提供商验证
        let valid_providers = [
            "anthropic",
            "openai",
            "gemini",
            "droid",
            "antigravity",
            "azure",
            "bedrock",
        ];
        if !valid_providers.contains(&item.provider.as_str()) {
            return (false, Some(format!("不支持的提供商: {}", item.provider)));
        }

        // 凭证类型验证
        let valid_types = ["api_key", "oauth", "setup_token", "upstream", "bedrock"];
        if !valid_types.contains(&item.credential_type.as_str()) {
            return (
                false,
                Some(format!("不支持的凭证类型: {}", item.credential_type)),
            );
        }

        // 凭证验证
        if item.credential.is_empty() {
            return (false, Some("凭证不能为空".to_string()));
        }

        // API Key 格式验证
        if item.credential_type == "api_key" {
            match item.provider.as_str() {
                "anthropic" => {
                    if !item.credential.starts_with("sk-ant-") {
                        return (
                            false,
                            Some("Anthropic API Key 应以 sk-ant- 开头".to_string()),
                        );
                    }
                }
                "openai" => {
                    if !item.credential.starts_with("sk-") {
                        return (false, Some("OpenAI API Key 应以 sk- 开头".to_string()));
                    }
                }
                "gemini" => {
                    if item.credential.len() < 20 {
                        return (false, Some("Gemini API Key 格式不正确".to_string()));
                    }
                }
                _ => {}
            }
        }

        // 优先级范围验证
        if item.priority < 0 || item.priority > 100 {
            return (false, Some("优先级应在 0-100 之间".to_string()));
        }

        (true, None)
    }

    /// 批量过滤重复账号
    async fn filter_duplicates_batch(
        &self,
        items: &[(usize, ImportAccountItem)],
    ) -> Result<Vec<(usize, ImportAccountItem)>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        // 收集所有名称
        let names: Vec<&str> = items.iter().map(|(_, item)| item.name.as_str()).collect();

        // 批量查询已存在的账号
        let existing = accounts::Entity::find()
            .filter(accounts::Column::Name.is_in(names))
            .all(&self.db)
            .await?;

        let existing_names: std::collections::HashSet<&str> =
            existing.iter().map(|a| a.name.as_str()).collect();

        // 过滤掉重复的
        let unique: Vec<(usize, ImportAccountItem)> = items
            .iter()
            .filter(|(_, item)| !existing_names.contains(item.name.as_str()))
            .cloned()
            .collect();

        Ok(unique)
    }

    /// 批量导入（分批事务）
    async fn import_batch_with_progress<F>(
        &self,
        items: Vec<(usize, ImportAccountItem)>,
        mut progress_fn: F,
    ) -> Result<(Vec<String>, Vec<ImportError>)>
    where
        F: FnMut(usize),
    {
        if items.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        let mut all_ids = Vec::with_capacity(items.len());
        let mut all_errors = Vec::new();
        let batch_size = self.config.batch_size;

        // 分批处理
        for chunk in items.chunks(batch_size) {
            match self.import_single_batch(chunk).await {
                Ok(ids) => {
                    all_ids.extend(ids);
                }
                Err(e) => {
                    // 整批失败，记录错误
                    for (idx, item) in chunk {
                        all_errors.push(ImportError {
                            index: *idx,
                            name: item.name.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
            progress_fn(all_ids.len() + all_errors.len());
        }

        Ok((all_ids, all_errors))
    }

    /// 单批次导入（事务）
    async fn import_single_batch(
        &self,
        items: &[(usize, ImportAccountItem)],
    ) -> Result<Vec<String>> {
        let txn = self.db.begin().await?;
        let now = Utc::now();

        // 构建批量插入模型（带凭证加密）
        let mut models = Vec::with_capacity(items.len());
        let mut inserted_ids = Vec::with_capacity(items.len());
        for (_, item) in items {
            // 加密凭证
            let encrypted_credential = GlobalEncryption::encrypt(&item.credential)
                .map_err(|e| anyhow::anyhow!("Failed to encrypt credential: {}", e))?;

            let id = Uuid::new_v4();
            inserted_ids.push(id.to_string());
            models.push(accounts::ActiveModel {
                id: Set(id),
                name: Set(item.name.clone()),
                provider: Set(item.provider.clone()),
                credential_type: Set(item.credential_type.clone()),
                credential: Set(encrypted_credential),
                metadata: Set(None),
                status: Set("active".to_string()),
                last_error: Set(None),
                priority: Set(item.priority),
                concurrent_limit: Set(item.concurrent_limit),
                rate_limit_rpm: Set(item.rate_limit_rpm),
                group_id: Set(item.group_id),
                created_at: Set(now),
                updated_at: Set(now),
            });
        }

        // 批量插入
        let _inserted = accounts::Entity::insert_many(models).exec(&txn).await?;

        txn.commit().await?;

        Ok(inserted_ids)
    }

    /// 快速批量导入（无验证，适用于可信数据源）
    pub async fn fast_import(self, items: Vec<ImportAccountItem>) -> Result<ImportResult> {
        let start = std::time::Instant::now();
        let total = items.len();

        let txn = self.db.begin().await?;
        let now = Utc::now();

        // 构建批量插入模型（带凭证加密）
        let mut models = Vec::with_capacity(items.len());
        let mut account_ids = Vec::with_capacity(items.len());
        let mut provider_stats = std::collections::BTreeMap::<String, ImportResultProvider>::new();
        for item in &items {
            // 加密凭证
            let encrypted_credential = GlobalEncryption::encrypt(&item.credential)
                .map_err(|e| anyhow::anyhow!("Failed to encrypt credential: {}", e))?;

            let stat = provider_stats
                .entry(item.provider.clone())
                .or_insert_with(|| ImportResultProvider {
                    provider: item.provider.clone(),
                    total: 0,
                    imported: 0,
                    skipped: 0,
                    failed: 0,
                });
            stat.total += 1;

            let id = Uuid::new_v4();
            account_ids.push(id.to_string());
            models.push(accounts::ActiveModel {
                id: Set(id),
                name: Set(item.name.clone()),
                provider: Set(item.provider.clone()),
                credential_type: Set(item.credential_type.clone()),
                credential: Set(encrypted_credential),
                metadata: Set(None),
                status: Set("active".to_string()),
                last_error: Set(None),
                priority: Set(item.priority),
                concurrent_limit: Set(item.concurrent_limit),
                rate_limit_rpm: Set(item.rate_limit_rpm),
                group_id: Set(item.group_id),
                created_at: Set(now),
                updated_at: Set(now),
            });
        }

        // 分批插入避免超大批次
        let mut inserted = 0;
        for chunk in models.chunks(self.config.batch_size) {
            let chunk_models = chunk.to_vec();
            let chunk_len = chunk_models.len();
            accounts::Entity::insert_many(chunk_models)
                .exec(&txn)
                .await?;
            inserted += chunk_len;
        }

        txn.commit().await?;

        for stat in provider_stats.values_mut() {
            stat.imported = stat.total;
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = ImportResult {
            total,
            imported: inserted,
            skipped: 0,
            failed: 0,
            account_ids,
            errors: Vec::new(),
            duration_ms,
            throughput_items_per_sec: batch_throughput(total, duration_ms),
            providers: provider_stats.into_values().collect(),
        };

        BatchMetrics::record("fast_import", "fast", total, 0, result.duration_ms);

        Ok(result)
    }
}

/// 批量验证服务（独立使用）
pub struct AccountValidator {
    providers: Vec<&'static str>,
}

impl Default for AccountValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountValidator {
    pub fn new() -> Self {
        Self {
            providers: vec![
                "anthropic",
                "openai",
                "gemini",
                "antigravity",
                "azure",
                "bedrock",
            ],
        }
    }

    /// 并行验证账号有效性（调用实际 API）
    pub async fn validate_accounts_parallel(
        &self,
        accounts: &[(Uuid, String, String, String)], // (id, provider, credential_type, credential)
        concurrency: usize,
    ) -> Vec<(Uuid, bool, Option<String>)> {
        let results: Vec<(Uuid, bool, Option<String>)> = stream::iter(accounts.iter())
            .map(|(id, _provider, _cred_type, cred)| async move {
                // NOTE: 实际调用 API 验证
                // 这里简化为格式验证
                let valid = !cred.is_empty();
                (*id, valid, None)
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_account_item_defaults() {
        let json = r#"{"name":"test","provider":"openai","credential":"sk-test"}"#;
        let item: ImportAccountItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.name, "test");
        assert_eq!(item.credential_type, "api_key");
        assert_eq!(item.priority, 50);
        assert_eq!(item.concurrent_limit, Some(5));
    }

    #[test]
    fn test_batch_import_config_default() {
        let config = BatchImportConfig::default();
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.validation_concurrency, 50);
        assert!(config.skip_duplicates);
        assert!(config.continue_on_error);
    }

    #[tokio::test]
    async fn test_validate_items_parallel() {
        let service = BatchImportService::new(Default::default());
        let items = vec![
            ImportAccountItem {
                name: "ok-openai".to_string(),
                provider: "openai".to_string(),
                credential_type: "api_key".to_string(),
                credential: "sk-test".to_string(),
                priority: 50,
                concurrent_limit: Some(5),
                rate_limit_rpm: None,
                group_id: None,
            },
            ImportAccountItem {
                name: "".to_string(),
                provider: "openai".to_string(),
                credential_type: "api_key".to_string(),
                credential: "sk-test".to_string(),
                priority: 50,
                concurrent_limit: Some(5),
                rate_limit_rpm: None,
                group_id: None,
            },
            ImportAccountItem {
                name: "bad-key".to_string(),
                provider: "openai".to_string(),
                credential_type: "api_key".to_string(),
                credential: "bad".to_string(),
                priority: 50,
                concurrent_limit: Some(5),
                rate_limit_rpm: None,
                group_id: None,
            },
        ];

        let mut results = service.validate_items_parallel(&items).await;
        results.sort_by_key(|result| result.index);
        assert_eq!(results.len(), 3);
        assert!(results[0].valid);
        assert!(!results[1].valid);
        assert!(!results[2].valid);
    }
}
