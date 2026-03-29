//! Account RPM (Requests Per Minute) service

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Account RPM config
#[derive(Debug, Clone)]
pub struct AccountRpmConfig {
    pub account_id: i64,
    pub max_rpm: u32,
    pub current_count: u32,
    pub window_start: Instant,
}

/// Account RPM service
pub struct AccountRpmService {
    configs: Arc<RwLock<HashMap<i64, AccountRpmConfig>>>,
    window_duration: Duration,
}

impl Default for AccountRpmService {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl AccountRpmService {
    pub fn new(window_duration: Duration) -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
            window_duration,
        }
    }

    pub async fn set_rpm_limit(&self, account_id: i64, max_rpm: u32) {
        let mut configs = self.configs.write().await;
        configs.insert(
            account_id,
            AccountRpmConfig {
                account_id,
                max_rpm,
                current_count: 0,
                window_start: Instant::now(),
            },
        );
    }

    pub async fn check_and_increment(&self, account_id: i64) -> Result<(), String> {
        let mut configs = self.configs.write().await;
        let config = configs
            .get_mut(&account_id)
            .ok_or("Account not configured")?;

        // Reset window if expired
        if config.window_start.elapsed() > self.window_duration {
            config.current_count = 0;
            config.window_start = Instant::now();
        }

        if config.current_count >= config.max_rpm {
            return Err("RPM limit exceeded".to_string());
        }

        config.current_count += 1;
        Ok(())
    }

    pub async fn get_current_rpm(&self, account_id: i64) -> Option<u32> {
        let configs = self.configs.read().await;
        configs.get(&account_id).map(|c| c.current_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpm_service() {
        let service = AccountRpmService::new(Duration::from_secs(60));

        service.set_rpm_limit(123, 5).await;

        for _ in 0..5 {
            assert!(service.check_and_increment(123).await.is_ok());
        }

        assert!(service.check_and_increment(123).await.is_err());
    }
}
