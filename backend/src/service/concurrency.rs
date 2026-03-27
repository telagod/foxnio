//! 并发控制实现

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore, OwnedSemaphorePermit};

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
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            user_max_concurrent: 5,
            account_max_concurrent: 10,
            api_key_max_concurrent: 5,
            global_max_concurrent: 1000,
        }
    }
}

/// 并发槽位
pub struct ConcurrencySlot {
    _permit: OwnedSemaphorePermit,
}

/// 并发控制器
pub struct ConcurrencyController {
    config: ConcurrencyConfig,
    global_semaphore: Arc<Semaphore>,
    user_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    account_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
    api_key_semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
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
        }
    }

    /// 获取用户信号量
    async fn get_user_semaphore(&self, user_id: &str) -> Arc<Semaphore> {
        let mut semaphores = self.user_semaphores.write().await;

        semaphores
            .entry(user_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.user_max_concurrent as usize)))
            .clone()
    }

    /// 获取账号信号量
    async fn get_account_semaphore(&self, account_id: &str) -> Arc<Semaphore> {
        let mut semaphores = self.account_semaphores.write().await;

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
        let _global_permit = self
            .global_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::GlobalLimitReached)?;

        // 2. 检查用户限制
        let user_semaphore = self.get_user_semaphore(user_id).await;
        let _user_permit = user_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::UserLimitReached)?;

        // 3. 检查账号限制
        let account_semaphore = self.get_account_semaphore(account_id).await;
        let _account_permit = account_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::AccountLimitReached)?;

        // 4. 检查 API Key 限制
        let api_key_semaphore = self.get_api_key_semaphore(api_key_id).await;
        let api_key_permit = api_key_semaphore
            .try_acquire_owned()
            .map_err(|_| ConcurrencyError::ApiKeyLimitReached)?;

        // 所有检查通过，返回槽位
        Ok(Some(ConcurrencySlot {
            _permit: api_key_permit,
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
        let _global_permit = self
            .global_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::GlobalLimitReached)?;

        let user_semaphore = self.get_user_semaphore(user_id).await;
        let _user_permit = user_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::UserLimitReached)?;

        let account_semaphore = self.get_account_semaphore(account_id).await;
        let _account_permit = account_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::AccountLimitReached)?;

        let api_key_semaphore = self.get_api_key_semaphore(api_key_id).await;
        let api_key_permit = api_key_semaphore
            .acquire_owned()
            .await
            .map_err(|_| ConcurrencyError::ApiKeyLimitReached)?;

        Ok(ConcurrencySlot {
            _permit: api_key_permit,
        })
    }

    /// 获取当前并发统计
    pub async fn get_stats(&self) -> ConcurrencyStats {
        let user_semaphores = self.user_semaphores.read().await;
        let account_semaphores = self.account_semaphores.read().await;
        let api_key_semaphores = self.api_key_semaphores.read().await;

        ConcurrencyStats {
            global_available: self.global_semaphore.available_permits(),
            total_users: user_semaphores.len(),
            total_accounts: account_semaphores.len(),
            total_api_keys: api_key_semaphores.len(),
        }
    }

    /// 清理不活跃的信号量
    pub async fn cleanup_inactive(&self) {
        // TODO: 实现 LRU 清理
    }
}

/// 并发错误
#[derive(Debug, Clone)]
pub enum ConcurrencyError {
    GlobalLimitReached,
    UserLimitReached,
    AccountLimitReached,
    ApiKeyLimitReached,
}

impl std::fmt::Display for ConcurrencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConcurrencyError::GlobalLimitReached => write!(f, "Global concurrency limit reached"),
            ConcurrencyError::UserLimitReached => write!(f, "User concurrency limit reached"),
            ConcurrencyError::AccountLimitReached => write!(f, "Account concurrency limit reached"),
            ConcurrencyError::ApiKeyLimitReached => write!(f, "API Key concurrency limit reached"),
        }
    }
}

impl std::error::Error for ConcurrencyError {}

/// 并发统计
#[derive(Debug, Clone)]
pub struct ConcurrencyStats {
    pub global_available: usize,
    pub total_users: usize,
    pub total_accounts: usize,
    pub total_api_keys: usize,
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
        let error = ConcurrencyError::UserLimitReached;
        let display = format!("{}", error);

        assert!(display.contains("User concurrency limit"));
    }
}
