use crate::model::account::Account;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{query, query_as, PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// OpenAI account scheduler for intelligent account selection
pub struct OpenAIAccountScheduler {
    pool: PgPool,
    account_states: Arc<RwLock<HashMap<i64, AccountState>>>,
    config: SchedulerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub account_id: i64,
    pub is_available: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub request_count: u64,
    pub error_count: u64,
    pub rpm: u32, // requests per minute
    pub current_load: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_rpm: u32,
    pub max_load: f32,
    pub cooldown_seconds: u64,
    pub sticky_session_ttl: u64,
    pub enable_sticky_session: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_rpm: 100,
            max_load: 0.8,
            cooldown_seconds: 60,
            sticky_session_ttl: 300,
            enable_sticky_session: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchedulingContext {
    pub user_id: i64,
    pub model: String,
    pub api_key_id: Option<i64>,
    pub session_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("No available accounts")]
    NoAvailableAccounts,
    #[error("Account not found: {0}")]
    AccountNotFound(i64),
    #[error("Account rate limited: {0}")]
    RateLimited(i64),
    #[error("Account overloaded: {0}")]
    Overloaded(i64),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl OpenAIAccountScheduler {
    pub fn new(pool: PgPool, config: SchedulerConfig) -> Self {
        Self {
            pool,
            account_states: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Select best account for request
    pub async fn select_account(&self, ctx: &SchedulingContext) -> Result<Account, SchedulerError> {
        // Try sticky session first
        if self.config.enable_sticky_session {
            if let Some(session_id) = &ctx.session_id {
                if let Some(account) = self.get_sticky_account(session_id).await? {
                    debug!("Using sticky session account: {}", account.id);
                    return Ok(account);
                }
            }
        }

        // Get all available accounts
        let accounts = self.get_available_accounts(&ctx.model).await?;
        if accounts.is_empty() {
            return Err(SchedulerError::NoAvailableAccounts);
        }

        // Select account using weighted round-robin
        let selected = self.select_by_weight(&accounts).await?;

        // Update sticky session
        if self.config.enable_sticky_session {
            if let Some(session_id) = &ctx.session_id {
                self.set_sticky_account(session_id, selected.id).await?;
            }
        }

        // Update account state
        self.update_account_usage(selected.id).await?;

        Ok(selected)
    }

    /// Get account for sticky session
    async fn get_sticky_account(
        &self,
        session_id: &str,
    ) -> Result<Option<Account>, SchedulerError> {
        let result = query(
            r#"
            SELECT account_id FROM sticky_sessions
            WHERE session_id = $1 AND created_at > NOW() - INTERVAL '5 minutes'
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            let account_id: i64 = row.try_get("account_id")?;
            let account = query_as::<_, Account>("SELECT * FROM accounts WHERE id = $1")
                .bind(account_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| SchedulerError::AccountNotFound(account_id))?;

            // Check if account is still available
            if self.is_account_available(&account).await? {
                return Ok(Some(account));
            }
        }

        Ok(None)
    }

    /// Set sticky session
    async fn set_sticky_account(
        &self,
        session_id: &str,
        account_id: i64,
    ) -> Result<(), SchedulerError> {
        query(
            r#"
            INSERT INTO sticky_sessions (session_id, account_id, created_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (session_id) DO UPDATE SET
                account_id = EXCLUDED.account_id,
                created_at = NOW()
            "#,
        )
        .bind(session_id)
        .bind(account_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get available accounts for model
    async fn get_available_accounts(&self, model: &str) -> Result<Vec<Account>, SchedulerError> {
        let accounts = query_as::<_, Account>(
            r#"
            SELECT * FROM accounts
            WHERE status = 'active'
            AND models @> $1
            AND deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .bind(&[model])
        .fetch_all(&self.pool)
        .await?;

        // Filter by availability
        let mut available = Vec::new();
        for account in accounts {
            if self.is_account_available(&account).await? {
                available.push(account);
            }
        }

        Ok(available)
    }

    /// Check if account is available
    async fn is_account_available(&self, account: &Account) -> Result<bool, SchedulerError> {
        let states = self.account_states.read().await;

        if let Some(state) = states.get(&account.id) {
            // Check rate limit
            if state.rpm >= self.config.max_rpm {
                return Ok(false);
            }

            // Check load
            if state.current_load >= self.config.max_load {
                return Ok(false);
            }

            // Check cooldown
            if let Some(last_used) = state.last_used_at {
                let elapsed = (Utc::now() - last_used).num_seconds();
                if elapsed < self.config.cooldown_seconds as i64 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Select account by weight (load factor)
    async fn select_by_weight(&self, accounts: &[Account]) -> Result<Account, SchedulerError> {
        if accounts.is_empty() {
            return Err(SchedulerError::NoAvailableAccounts);
        }

        // Get account states for load calculation
        let states = self.account_states.read().await;

        // Simple weighted selection based on current load from account_states
        let selected = accounts
            .iter()
            .min_by(|a, b| {
                let load_a = states.get(&a.id).map(|s| s.current_load).unwrap_or(0.0);
                let load_b = states.get(&b.id).map(|s| s.current_load).unwrap_or(0.0);
                load_a
                    .partial_cmp(&load_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or(SchedulerError::NoAvailableAccounts)?
            .clone();

        Ok(selected)
    }

    /// Update account usage
    async fn update_account_usage(&self, account_id: i64) -> Result<(), SchedulerError> {
        let mut states = self.account_states.write().await;
        let state = states.entry(account_id).or_insert(AccountState {
            account_id,
            is_available: true,
            last_used_at: None,
            request_count: 0,
            error_count: 0,
            rpm: 0,
            current_load: 0.0,
        });

        state.last_used_at = Some(Utc::now());
        state.request_count += 1;
        state.rpm += 1;

        Ok(())
    }

    /// Report account error
    pub async fn report_error(&self, account_id: i64) -> Result<(), SchedulerError> {
        let mut states = self.account_states.write().await;
        if let Some(state) = states.get_mut(&account_id) {
            state.error_count += 1;
            if state.request_count > 0 {
                state.current_load =
                    (state.error_count as f32 / state.request_count as f32).min(1.0);
            }
        }

        Ok(())
    }

    /// Refresh account states
    pub async fn refresh_states(&self) -> Result<(), SchedulerError> {
        let accounts = query_as::<_, Account>(
            "SELECT * FROM accounts WHERE status = 'active' AND deleted_at IS NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut states = self.account_states.write().await;
        for account in accounts {
            states.entry(account.id).or_insert_with(|| AccountState {
                account_id: account.id,
                is_available: true,
                last_used_at: None,
                request_count: 0,
                error_count: 0,
                rpm: 0,
                current_load: 0.0,
            });
        }

        Ok(())
    }

    /// Clear stale sticky sessions
    pub async fn clear_stale_sessions(&self) -> Result<(), SchedulerError> {
        query("DELETE FROM sticky_sessions WHERE created_at < NOW() - INTERVAL '5 minutes'")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.max_rpm, 100);
        assert!(config.enable_sticky_session);
    }
}
