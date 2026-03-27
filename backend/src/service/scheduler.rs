//! 智能调度服务 - 完整实现

use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::entity::accounts;
use super::{AccountService, FailoverManager};

/// 调度策略
#[derive(Debug, Clone)]
pub enum SchedulingStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 加权轮询
    WeightedRoundRobin,
    /// 优先级优先
    PriorityFirst,
    /// 随机
    Random,
}

/// 账号运行时状态
#[derive(Debug, Clone)]
pub struct AccountRuntimeState {
    pub account_id: uuid::Uuid,
    pub current_connections: i32,
    pub total_requests: i64,
    pub total_errors: i64,
    pub last_used: Option<DateTime<Utc>>,
    pub is_available: bool,
}

/// 粘性会话信息
#[derive(Debug, Clone)]
pub struct StickySession {
    pub account_id: uuid::Uuid,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub request_count: i64,
}

/// 调度服务
pub struct SchedulerService {
    db: DatabaseConnection,
    account_service: AccountService,
    failover_manager: FailoverManager,
    strategy: SchedulingStrategy,
    
    // 运行时状态
    runtime_states: Arc<RwLock<HashMap<uuid::Uuid, AccountRuntimeState>>>,
    sticky_sessions: Arc<RwLock<HashMap<String, StickySession>>>,
    round_robin_index: Arc<RwLock<usize>>,
}

impl SchedulerService {
    pub fn new(
        db: DatabaseConnection,
        account_service: AccountService,
        strategy: SchedulingStrategy,
    ) -> Self {
        Self {
            db,
            account_service,
            failover_manager: FailoverManager::new(Default::default()),
            strategy,
            runtime_states: Arc::new(RwLock::new(HashMap::new())),
            sticky_sessions: Arc::new(RwLock::new(HashMap::new())),
            round_robin_index: Arc::new(RwLock::new(0)),
        }
    }
    
    /// 选择最佳账号
    pub async fn select_account(
        &self,
        model: &str,
        session_id: Option<&str>,
        user_concurrent_limit: i32,
    ) -> Result<Option<accounts::Model>> {
        // 1. 检查粘性会话
        if let Some(sid) = session_id {
            if let Some(account) = self.get_sticky_account(sid).await? {
                // 验证账号仍然可用
                if self.is_account_available(&account).await {
                    return Ok(Some(account));
                }
            }
        }
        
        // 2. 获取可用账号列表
        let mut accounts = self.account_service.get_for_model(model).await?;
        
        if accounts.is_empty() {
            return Ok(None);
        }
        
        // 3. 过滤可用账号
        accounts = self.filter_available_accounts(accounts).await;
        
        if accounts.is_empty() {
            return Ok(None);
        }
        
        // 4. 根据策略选择账号
        let selected = match &self.strategy {
            SchedulingStrategy::RoundRobin => self.select_round_robin(accounts).await,
            SchedulingStrategy::LeastConnections => self.select_least_connections(accounts).await,
            SchedulingStrategy::PriorityFirst => self.select_priority_first(accounts).await,
            SchedulingStrategy::Random => self.select_random(accounts).await,
            SchedulingStrategy::WeightedRoundRobin => self.select_weighted(accounts).await,
        };
        
        // 5. 设置粘性会话
        if let (Some(ref account), Some(sid)) = (&selected, session_id) {
            self.set_sticky_session(sid.to_string(), account.id).await;
        }
        
        // 6. 更新运行时状态
        if let Some(ref account) = &selected {
            self.increment_connections(account.id).await;
        }
        
        Ok(selected)
    }
    
    /// 获取粘性会话的账号
    async fn get_sticky_account(&self, session_id: &str) -> Result<Option<accounts::Model>> {
        let sessions = self.sticky_sessions.read().await;
        
        if let Some(sticky) = sessions.get(session_id) {
            // 检查会话是否过期 (默认 1 小时)
            let now = Utc::now();
            if (now - sticky.last_accessed).num_seconds() > 3600 {
                return Ok(None);
            }
            
            // 获取账号
            if let Some(account) = self.account_service.get_with_credential(sticky.account_id).await? {
                return Ok(Some(account));
            }
        }
        
        Ok(None)
    }
    
    /// 设置粘性会话
    pub async fn set_sticky_session(&self, session_id: String, account_id: uuid::Uuid) {
        let mut sessions = self.sticky_sessions.write().await;
        
        let now = Utc::now();
        let sticky = sessions.entry(session_id).or_insert(StickySession {
            account_id,
            created_at: now,
            last_accessed: now,
            request_count: 0,
        });
        
        sticky.last_accessed = now;
        sticky.request_count += 1;
    }
    
    /// 清除粘性会话
    pub async fn clear_sticky_session(&self, session_id: &str) {
        let mut sessions = self.sticky_sessions.write().await;
        sessions.remove(session_id);
    }
    
    /// 检查账号是否可用
    async fn is_account_available(&self, account: &accounts::Model) -> bool {
        // 检查状态
        if account.status != "active" {
            return false;
        }
        
        // 检查健康状态
        if !self.failover_manager.is_account_healthy(&account.id).await {
            return false;
        }
        
        // 检查并发限制
        let states = self.runtime_states.read().await;
        if let Some(state) = states.get(&account.id) {
            let limit = account.concurrent_limit.unwrap_or(5);
            if state.current_connections >= limit {
                return false;
            }
        }
        
        true
    }
    
    /// 过滤可用账号
    async fn filter_available_accounts(&self, accounts: Vec<accounts::Model>) -> Vec<accounts::Model> {
        let mut available = Vec::new();
        
        for account in accounts {
            if self.is_account_available(&account).await {
                available.push(account);
            }
        }
        
        available
    }
    
    /// 轮询选择
    async fn select_round_robin(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        if accounts.is_empty() {
            return None;
        }
        
        let mut index = self.round_robin_index.write().await;
        *index = (*index + 1) % accounts.len();
        
        Some(accounts[*index].clone())
    }
    
    /// 最少连接选择
    async fn select_least_connections(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        let states = self.runtime_states.read().await;
        
        accounts.into_iter()
            .min_by_key(|a| {
                states.get(&a.id)
                    .map(|s| s.current_connections)
                    .unwrap_or(0)
            })
    }
    
    /// 优先级优先选择
    async fn select_priority_first(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        accounts.into_iter()
            .max_by_key(|a| a.priority)
    }
    
    /// 随机选择
    async fn select_random(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        if accounts.is_empty() {
            return None;
        }
        
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..accounts.len());
        
        Some(accounts[index].clone())
    }
    
    /// 加权轮询选择
    async fn select_weighted(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        // 优先级越高，权重越大
        let total_weight: i32 = accounts.iter().map(|a| a.priority + 1).sum();
        
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total_weight);
        
        for account in accounts {
            target -= account.priority + 1;
            if target <= 0 {
                return Some(account);
            }
        }
        
        None
    }
    
    /// 增加连接计数
    pub async fn increment_connections(&self, account_id: uuid::Uuid) {
        let mut states = self.runtime_states.write().await;
        
        let state = states.entry(account_id).or_insert(AccountRuntimeState {
            account_id,
            current_connections: 0,
            total_requests: 0,
            total_errors: 0,
            last_used: None,
            is_available: true,
        });
        
        state.current_connections += 1;
        state.total_requests += 1;
        state.last_used = Some(Utc::now());
    }
    
    /// 减少连接计数
    pub async fn decrement_connections(&self, account_id: uuid::Uuid) {
        let mut states = self.runtime_states.write().await;
        
        if let Some(state) = states.get_mut(&account_id) {
            state.current_connections = (state.current_connections - 1).max(0);
        }
    }
    
    /// 记录错误
    pub async fn record_error(&self, account_id: uuid::Uuid) {
        let mut states = self.runtime_states.write().await;
        
        if let Some(state) = states.get_mut(&account_id) {
            state.total_errors += 1;
        }
        
        // 同时标记到故障转移管理器
        self.failover_manager.mark_failure(account_id, "Request failed".to_string()).await;
    }
    
    /// 获取运行时统计
    pub async fn get_runtime_stats(&self) -> HashMap<uuid::Uuid, AccountRuntimeState> {
        self.runtime_states.read().await.clone()
    }
    
    /// 获取粘性会话统计
    pub async fn get_sticky_stats(&self) -> HashMap<String, StickySession> {
        self.sticky_sessions.read().await.clone()
    }
    
    /// 清理过期会话
    pub async fn cleanup_expired_sessions(&self, max_age_seconds: i64) {
        let mut sessions = self.sticky_sessions.write().await;
        let now = Utc::now();
        
        sessions.retain(|_, session| {
            (now - session.last_accessed).num_seconds() <= max_age_seconds
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scheduling_strategy() {
        let strategy = SchedulingStrategy::RoundRobin;
        assert!(matches!(strategy, SchedulingStrategy::RoundRobin));
    }
    
    #[test]
    fn test_account_runtime_state() {
        let state = AccountRuntimeState {
            account_id: uuid::Uuid::nil(),
            current_connections: 5,
            total_requests: 100,
            total_errors: 2,
            last_used: Some(Utc::now()),
            is_available: true,
        };
        
        assert_eq!(state.current_connections, 5);
        assert_eq!(state.total_requests, 100);
    }
}
