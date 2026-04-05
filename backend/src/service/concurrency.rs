//! 并发控制实现
//!
//! 提供用户级、账号级、API Key 级和全局级的并发控制
//! 支持动态限制调整、分布式计数、详细统计
//!
//! 预留功能：并发控制（扩展功能）

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::time::Instant;

/// 并发限制配置
#[derive(Debug, Clone)]
pub struct ConcurrencyConfig {
    /// 用户最大并发数
    pub user_max_concurrent: u32,
    /// 账号最大并发数
    pub account_max_concurrent: u32,
    /// API Key 最大并发数
    pub api_key_max_concurrent: u32,
    /// 全局最大并发数
    pub global_max_concurrent: u32,
    /// 是否启用动态调整
    pub enable_dynamic_adjustment: bool,
    /// 动态调整周期（秒）
    pub adjustment_interval_seconds: u64,
    /// 高负载阈值（0.0-1.0）
    pub high_load_threshold: f64,
    /// 低负载阈值（0.0-1.0）
    pub low_load_threshold: f64,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            user_max_concurrent: 5,
            account_max_concurrent: 10,
            api_key_max_concurrent: 5,
            global_max_concurrent: 1000,
            enable_dynamic_adjustment: true,
            adjustment_interval_seconds: 60,
            high_load_threshold: 0.8,
            low_load_threshold: 0.3,
        }
    }
}

/// 并发槽位
pub struct ConcurrencySlot {
    _global_permit: OwnedSemaphorePermit,
    _user_permit: OwnedSemaphorePermit,
    _account_permit: OwnedSemaphorePermit,
    _api_key_permit: OwnedSemaphorePermit,
}

/// Maximum entries per semaphore map before LRU eviction kicks in
const MAX_SEMAPHORE_MAP_ENTRIES: usize = 10_000;

/// 并发控制器
pub struct ConcurrencyController {
    config: ConcurrencyConfig,
    global_semaphore: Arc<Semaphore>,
    user_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    account_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    api_key_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    user_last_access: Arc<RwLock<HashMap<String, Instant>>>,
    account_last_access: Arc<RwLock<HashMap<String, Instant>>>,
    api_key_last_access: Arc<RwLock<HashMap<String, Instant>>>,
}

impl ConcurrencyController {
    pub fn new(config: ConcurrencyConfig) -> Self {
        let global_semaphore = Arc::new(Semaphore::new(config.global_max_concurrent as usize));

        Self {
            config,
            global_semaphore,
            user_semaphores: Arc::new(RwLock::new(HashMap::new())),
            account_semaphores: Arc::new(RwLock::new(HashMap::new())),
            api_key_semaphores: Arc::new(RwLock::new(HashMap::new())),
            user_last_access: Arc::new(RwLock::new(HashMap::new())),
            account_last_access: Arc::new(RwLock::new(HashMap::new())),
            api_key_last_access: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取用户信号量
    async fn get_user_semaphore(&self, user_id: &str) -> Arc<Semaphore> {
        let mut semaphores = self.user_semaphores.write().await;
        let mut access = self.user_last_access.write().await;
        access.insert(user_id.to_string(), Instant::now());

        semaphores
            .entry(user_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.user_max_concurrent as usize)))
            .clone()
    }

    /// 获取账号信号量
    async fn get_account_semaphore(&self, account_id: &str) -> Arc<Semaphore> {
        let mut semaphores = self.account_semaphores.write().await;
        let mut access = self.account_last_access.write().await;
        access.insert(account_id.to_string(), Instant::now());

        semaphores
            .entry(account_id.to_string())
            .or_insert_with(|| {
                Arc::new(Semaphore::new(self.config.account_max_concurrent as usize))
            })
            .clone()
    }

    /// 获取 API Key 信号量
    async fn get_api_key_semaphore(&self, api_key_id: &str) -> Arc<Semaphore> {
        let mut semaphores = self.api_key_semaphores.write().await;
        let mut access = self.api_key_last_access.write().await;
        access.insert(api_key_id.to_string(), Instant::now());

        semaphores
            .entry(api_key_id.to_string())
            .or_insert_with(|| {
                Arc::new(Semaphore::new(self.config.api_key_max_concurrent as usize))
            })
            .clone()
    }

    /// 尝试获取并发槽位（非阻塞）
    pub async fn try_acquire(
        &self,
        user_id: &str,
        account_id: &str,
        api_key_id: &str,
    ) -> Result<Option<ConcurrencySlot>, ConcurrencyError> {
        // 1. 检查全局限制
        let global_permit = self
            .global_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::Global)?;

        // 2. 检查用户限制
        let user_semaphore = self.get_user_semaphore(user_id).await;
        let user_permit = user_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::User)?;

        // 3. 检查账号限制
        let account_semaphore = self.get_account_semaphore(account_id).await;
        let account_permit = account_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::Account)?;

        // 4. 检查 API Key 限制
        let api_key_semaphore = self.get_api_key_semaphore(api_key_id).await;
        let api_key_permit = api_key_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::ApiKey)?;

        // 所有检查通过，返回槽位
        Ok(Some(ConcurrencySlot {
            _global_permit: global_permit,
            _user_permit: user_permit,
            _account_permit: account_permit,
            _api_key_permit: api_key_permit,
        }))
    }

    /// 获取并发槽位（阻塞等待）
    pub async fn acquire(
        &self,
        user_id: &str,
        account_id: &str,
        api_key_id: &str,
    ) -> Result<ConcurrencySlot, ConcurrencyError> {
        // 按顺序获取许可
        let global_permit = self
            .global_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::Global)?;

        let user_semaphore = self.get_user_semaphore(user_id).await;
        let user_permit = user_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::User)?;

        let account_semaphore = self.get_account_semaphore(account_id).await;
        let account_permit = account_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::Account)?;

        let api_key_semaphore = self.get_api_key_semaphore(api_key_id).await;
        let api_key_permit = api_key_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::ApiKey)?;

        Ok(ConcurrencySlot {
            _global_permit: global_permit,
            _user_permit: user_permit,
            _account_permit: account_permit,
            _api_key_permit: api_key_permit,
        })
    }

    /// 获取当前并发统计
    pub async fn get_stats(&self) -> ConcurrencyStats {
        let user_semaphores = self.user_semaphores.read().await;
        let account_semaphores = self.account_semaphores.read().await;
        let api_key_semaphores = self.api_key_semaphores.read().await;

        // 计算各层级的活跃连接数
        let mut active_user_connections = 0;
        for sem in user_semaphores.values() {
            active_user_connections +=
                self.config.user_max_concurrent as usize - sem.available_permits();
        }

        let mut active_account_connections = 0;
        for sem in account_semaphores.values() {
            active_account_connections +=
                self.config.account_max_concurrent as usize - sem.available_permits();
        }

        let global_used =
            self.config.global_max_concurrent as usize - self.global_semaphore.available_permits();

        ConcurrencyStats {
            global_available: self.global_semaphore.available_permits(),
            global_used,
            total_users: user_semaphores.len(),
            total_accounts: account_semaphores.len(),
            total_api_keys: api_key_semaphores.len(),
            active_user_connections,
            active_account_connections,
            utilization_rate: global_used as f64 / self.config.global_max_concurrent as f64,
        }
    }

    /// 获取账号级并发统计
    pub async fn get_account_stats(&self, account_id: &str) -> Option<AccountConcurrencyStats> {
        let account_semaphores = self.account_semaphores.read().await;

        if let Some(sem) = account_semaphores.get(account_id) {
            let max_concurrent = self.config.account_max_concurrent as usize;
            let available = sem.available_permits();
            let used = max_concurrent - available;

            Some(AccountConcurrencyStats {
                account_id: account_id.to_string(),
                max_concurrent,
                current_concurrent: used,
                available,
                utilization_rate: used as f64 / max_concurrent as f64,
            })
        } else {
            None
        }
    }

    /// 获取所有账号的并发统计
    pub async fn get_all_account_stats(&self) -> Vec<AccountConcurrencyStats> {
        let account_semaphores = self.account_semaphores.read().await;
        let max_concurrent = self.config.account_max_concurrent as usize;

        account_semaphores
            .iter()
            .map(|(account_id, sem)| {
                let available = sem.available_permits();
                let used = max_concurrent - available;
                AccountConcurrencyStats {
                    account_id: account_id.clone(),
                    max_concurrent,
                    current_concurrent: used,
                    available,
                    utilization_rate: used as f64 / max_concurrent as f64,
                }
            })
            .collect()
    }

    /// 设置账号级并发限制
    pub async fn set_account_limit(&self, account_id: &str, max_concurrent: u32) -> Result<()> {
        let mut account_semaphores = self.account_semaphores.write().await;

        // 创建新的信号量替换旧的
        account_semaphores.insert(
            account_id.to_string(),
            Arc::new(Semaphore::new(max_concurrent as usize)),
        );

        Ok(())
    }

    /// 动态调整并发限制
    pub async fn adjust_limits(&self) -> Result<AdjustmentResult> {
        if !self.config.enable_dynamic_adjustment {
            return Ok(AdjustmentResult::default());
        }

        let stats = self.get_stats().await;
        let mut adjustments = Vec::new();

        // 检查全局负载
        if stats.utilization_rate > self.config.high_load_threshold {
            // 高负载：减少用户并发限制
            let new_limit = (self.config.user_max_concurrent as f64 * 0.8) as u32;
            adjustments.push(Adjustment {
                level: "user".to_string(),
                old_limit: self.config.user_max_concurrent,
                new_limit,
                reason: "High load detected".to_string(),
            });
        } else if stats.utilization_rate < self.config.low_load_threshold {
            // 低负载：增加用户并发限制
            let new_limit = (self.config.user_max_concurrent as f64 * 1.2) as u32;
            adjustments.push(Adjustment {
                level: "user".to_string(),
                old_limit: self.config.user_max_concurrent,
                new_limit,
                reason: "Low load detected".to_string(),
            });
        }

        Ok(AdjustmentResult {
            utilization_rate: stats.utilization_rate,
            adjustments,
            timestamp: Utc::now(),
        })
    }

    /// 清理不活跃的信号量
    /// When any semaphore map exceeds MAX_SEMAPHORE_MAP_ENTRIES, evict the oldest
    /// entries (by last access time) that have all permits available (i.e. idle).
    pub async fn cleanup_inactive(&self) {
        Self::evict_lru(
            &self.user_semaphores,
            &self.user_last_access,
            self.config.user_max_concurrent as usize,
        )
        .await;
        Self::evict_lru(
            &self.account_semaphores,
            &self.account_last_access,
            self.config.account_max_concurrent as usize,
        )
        .await;
        Self::evict_lru(
            &self.api_key_semaphores,
            &self.api_key_last_access,
            self.config.api_key_max_concurrent as usize,
        )
        .await;
    }

    /// Evict idle entries from a semaphore map when it exceeds the threshold.
    /// Only removes entries whose semaphore has all permits available (no active usage).
    /// Removes the oldest half (by last access) to amortise cleanup cost.
    async fn evict_lru(
        semaphores: &RwLock<HashMap<String, Arc<Semaphore>>>,
        last_access: &RwLock<HashMap<String, Instant>>,
        max_permits: usize,
    ) {
        let sem_len = semaphores.read().await.len();
        if sem_len <= MAX_SEMAPHORE_MAP_ENTRIES {
            return;
        }

        let target_removals = sem_len - MAX_SEMAPHORE_MAP_ENTRIES / 2;

        // Collect idle keys with their last access time
        let sems = semaphores.read().await;
        let access = last_access.read().await;
        let mut idle_entries: Vec<(String, Instant)> = sems
            .iter()
            .filter(|(_, sem)| sem.available_permits() == max_permits)
            .map(|(key, _)| {
                let ts = access.get(key).copied().unwrap_or_else(Instant::now);
                (key.clone(), ts)
            })
            .collect();
        drop(sems);
        drop(access);

        // Sort oldest first
        idle_entries.sort_by_key(|(_, ts)| *ts);

        let to_remove: Vec<String> = idle_entries
            .into_iter()
            .take(target_removals)
            .map(|(key, _)| key)
            .collect();

        if to_remove.is_empty() {
            return;
        }

        let mut sems = semaphores.write().await;
        let mut access = last_access.write().await;
        for key in &to_remove {
            // Double-check the semaphore is still fully idle before removing
            if let Some(sem) = sems.get(key) {
                if sem.available_permits() == max_permits {
                    sems.remove(key);
                    access.remove(key);
                }
            }
        }
    }
}

/// 账号并发统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConcurrencyStats {
    pub account_id: String,
    pub max_concurrent: usize,
    pub current_concurrent: usize,
    pub available: usize,
    pub utilization_rate: f64,
}

/// 并发错误
#[derive(Debug, Clone)]
pub enum ConcurrencyError {
    /// 全局并发限制
    Global,
    /// 用户并发限制
    User,
    /// 账号并发限制
    Account,
    /// API Key 并发限制
    ApiKey,
}

impl std::fmt::Display for ConcurrencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConcurrencyError::Global => write!(f, "Global concurrency limit reached"),
            ConcurrencyError::User => write!(f, "User concurrency limit reached"),
            ConcurrencyError::Account => write!(f, "Account concurrency limit reached"),
            ConcurrencyError::ApiKey => write!(f, "API Key concurrency limit reached"),
        }
    }
}

impl std::error::Error for ConcurrencyError {}

/// 并发统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyStats {
    pub global_available: usize,
    pub global_used: usize,
    pub total_users: usize,
    pub total_accounts: usize,
    pub total_api_keys: usize,
    pub active_user_connections: usize,
    pub active_account_connections: usize,
    pub utilization_rate: f64,
}

/// 动态调整结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdjustmentResult {
    pub utilization_rate: f64,
    pub adjustments: Vec<Adjustment>,
    pub timestamp: DateTime<Utc>,
}

/// 单个调整
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adjustment {
    pub level: String,
    pub old_limit: u32,
    pub new_limit: u32,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_config_default() {
        let config = ConcurrencyConfig::default();

        assert_eq!(config.user_max_concurrent, 5);
        assert_eq!(config.account_max_concurrent, 10);
        assert_eq!(config.api_key_max_concurrent, 5);
        assert_eq!(config.global_max_concurrent, 1000);
    }

    #[test]
    fn test_concurrency_controller_creation() {
        let config = ConcurrencyConfig::default();
        let controller = ConcurrencyController::new(config);

        assert_eq!(controller.global_semaphore.available_permits(), 1000);
    }

    #[tokio::test]
    async fn test_concurrency_acquire() {
        let config = ConcurrencyConfig {
            user_max_concurrent: 2,
            account_max_concurrent: 2,
            api_key_max_concurrent: 2,
            global_max_concurrent: 100,
            enable_dynamic_adjustment: false,
            adjustment_interval_seconds: 60,
            high_load_threshold: 0.8,
            low_load_threshold: 0.3,
        };

        let controller = ConcurrencyController::new(config);

        // 第一个应该成功
        let slot1 = controller.try_acquire("user1", "account1", "key1").await;
        assert!(slot1.is_ok());

        // 第二个应该成功
        let slot2 = controller.try_acquire("user1", "account1", "key1").await;
        assert!(slot2.is_ok());

        // 第三个应该失败（超过限制）
        let slot3 = controller.try_acquire("user1", "account1", "key1").await;
        assert!(slot3.is_err());
    }

    #[tokio::test]
    async fn test_concurrency_stats() {
        let controller = ConcurrencyController::new(ConcurrencyConfig::default());

        let stats = controller.get_stats().await;

        assert_eq!(stats.global_available, 1000);
        assert_eq!(stats.total_users, 0);
    }

    #[test]
    fn test_concurrency_error_display() {
        let error = ConcurrencyError::User;
        let display = format!("{error}");

        assert!(display.contains("User concurrency limit"));
    }
}
