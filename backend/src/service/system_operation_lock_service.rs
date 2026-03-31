//! System operation lock service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Lock state
#[derive(Debug, Clone)]
pub struct LockState {
    /// Lock key
    pub key: String,
    /// Lock owner
    pub owner: String,
    /// Acquired at
    pub acquired_at: Instant,
    /// TTL
    pub ttl: Duration,
}

/// System operation lock service
pub struct SystemOperationLockService {
    /// Locks
    locks: Arc<RwLock<HashMap<String, LockState>>>,
    /// Default lock TTL
    default_ttl: Duration,
}

impl Default for SystemOperationLockService {
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}

impl SystemOperationLockService {
    /// Create a new lock service
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    /// Try to acquire a lock
    pub async fn try_acquire(&self, key: &str, owner: &str) -> Result<LockGuard, String> {
        self.try_acquire_with_ttl(key, owner, self.default_ttl)
            .await
    }

    /// Try to acquire a lock with custom TTL
    pub async fn try_acquire_with_ttl(
        &self,
        key: &str,
        owner: &str,
        ttl: Duration,
    ) -> Result<LockGuard, String> {
        let mut locks = self.locks.write().await;

        // Check if lock exists and is valid
        if let Some(existing) = locks.get(key) {
            if existing.acquired_at.elapsed() < existing.ttl {
                return Err(format!("Lock '{}' is held by {}", key, existing.owner));
            }
        }

        // Acquire lock
        let state = LockState {
            key: key.to_string(),
            owner: owner.to_string(),
            acquired_at: Instant::now(),
            ttl,
        };

        locks.insert(key.to_string(), state.clone());

        Ok(LockGuard {
            service: Arc::new(self.clone()),
            key: key.to_string(),
            owner: owner.to_string(),
        })
    }

    /// Release a lock
    pub async fn release(&self, key: &str, owner: &str) -> Result<(), String> {
        let mut locks = self.locks.write().await;

        match locks.get(key) {
            Some(lock) if lock.owner == owner => {
                locks.remove(key);
                Ok(())
            }
            Some(lock) => Err(format!(
                "Lock '{}' is held by {}, not {}",
                key, lock.owner, owner
            )),
            None => Err(format!("Lock '{key}' not found")),
        }
    }

    /// Check if a lock is held
    pub async fn is_locked(&self, key: &str) -> bool {
        let locks = self.locks.read().await;
        if let Some(lock) = locks.get(key) {
            lock.acquired_at.elapsed() < lock.ttl
        } else {
            false
        }
    }

    /// Get lock owner
    pub async fn get_owner(&self, key: &str) -> Option<String> {
        let locks = self.locks.read().await;
        locks.get(key).map(|l| l.owner.clone())
    }

    /// Clear all locks
    pub async fn clear_all(&self) {
        let mut locks = self.locks.write().await;
        locks.clear();
    }
}

impl Clone for SystemOperationLockService {
    fn clone(&self) -> Self {
        Self {
            locks: self.locks.clone(),
            default_ttl: self.default_ttl,
        }
    }
}

/// Lock guard (RAII pattern)
pub struct LockGuard {
    service: Arc<SystemOperationLockService>,
    key: String,
    owner: String,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Best effort release
        let service = self.service.clone();
        let key = self.key.clone();
        let owner = self.owner.clone();

        tokio::spawn(async move {
            let _ = service.release(&key, &owner).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_release() {
        let service = SystemOperationLockService::new(Duration::from_secs(10));

        let guard = service.try_acquire("test-lock", "owner-1").await;
        assert!(guard.is_ok());

        let second = service.try_acquire("test-lock", "owner-2").await;
        assert!(second.is_err());
    }

    #[tokio::test]
    async fn test_is_locked() {
        let service = SystemOperationLockService::new(Duration::from_secs(10));

        assert!(!service.is_locked("test-lock").await);

        let _guard = service.try_acquire("test-lock", "owner-1").await.unwrap();
        assert!(service.is_locked("test-lock").await);
    }
}
