//! 智能调度服务 - 完整实现
//!
//! 支持多种调度策略，集成健康评分和粘性会话

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use lru::LruCache;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::health_scorer::HealthScorer;
use super::LegacyAccountService as AccountService;
use crate::entity::{accounts, groups};
use crate::gateway::scheduler::group_policy::{
    GroupAccountInfo, GroupSchedulerState, GroupStickySession,
};
use crate::gateway::scheduler::metrics::SchedulerMetrics;
use crate::gateway::FailoverManager;

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
    /// 健康感知调度（优先选择健康账号）
    HealthAware,
    /// 智能调度（综合考虑健康、负载、延迟）
    Smart,
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
    /// 健康分数缓存
    pub health_score: f64,
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
    health_scorer: Option<Arc<HealthScorer>>,

    // 运行时状态
    runtime_states: Arc<RwLock<HashMap<uuid::Uuid, AccountRuntimeState>>>,
    sticky_sessions: Arc<RwLock<HashMap<String, StickySession>>>,
    round_robin_index: Arc<RwLock<usize>>,

    // 分组级调度状态
    group_states: Arc<RwLock<HashMap<i64, Arc<GroupSchedulerState>>>>,
    group_sticky_sessions: Arc<RwLock<LruCache<String, GroupStickySession>>>,
    scheduler_metrics: Arc<SchedulerMetrics>,
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
            health_scorer: None,
            runtime_states: Arc::new(RwLock::new(HashMap::new())),
            sticky_sessions: Arc::new(RwLock::new(HashMap::new())),
            round_robin_index: Arc::new(RwLock::new(0)),
            group_states: Arc::new(RwLock::new(HashMap::new())),
            group_sticky_sessions: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(10_000).unwrap(),
            ))),
            scheduler_metrics: Arc::new(SchedulerMetrics::new()),
        }
    }

    /// 创建带健康评分的调度器
    pub fn with_health_scorer(
        db: DatabaseConnection,
        account_service: AccountService,
        strategy: SchedulingStrategy,
        health_scorer: Arc<HealthScorer>,
    ) -> Self {
        Self {
            db,
            account_service,
            failover_manager: FailoverManager::new(Default::default()),
            strategy,
            health_scorer: Some(health_scorer),
            runtime_states: Arc::new(RwLock::new(HashMap::new())),
            sticky_sessions: Arc::new(RwLock::new(HashMap::new())),
            round_robin_index: Arc::new(RwLock::new(0)),
            group_states: Arc::new(RwLock::new(HashMap::new())),
            group_sticky_sessions: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(10_000).unwrap(),
            ))),
            scheduler_metrics: Arc::new(SchedulerMetrics::new()),
        }
    }

    /// 选择最佳账号（支持分组级调度策略）
    pub async fn select_account(
        &self,
        model: &str,
        session_id: Option<&str>,
        _user_concurrent_limit: i32,
    ) -> Result<Option<accounts::Model>> {
        // 1. 获取可用账号列表
        let mut accounts = self.account_service.get_for_model(model).await?;

        if accounts.is_empty() {
            return Ok(None);
        }

        // 2. 过滤可用账号
        accounts = self.filter_available_accounts(accounts).await;

        if accounts.is_empty() {
            return Ok(None);
        }

        // 3. 尝试按分组调度（如果账号有 group_id）
        if let Some(selected) = self
            .try_group_aware_select(&accounts, session_id)
            .await
        {
            // 更新运行时状态
            self.increment_connections(selected.id).await;
            return Ok(Some(selected));
        }

        // 4. Fallback: 无分组 — 检查全局粘性会话
        if let Some(sid) = session_id {
            if let Some(account) = self.get_sticky_account(sid).await? {
                if self.is_account_available(&account).await {
                    return Ok(Some(account));
                }
            }
        }

        // 5. 根据全局策略选择账号
        let selected = match &self.strategy {
            SchedulingStrategy::RoundRobin => self.select_round_robin(accounts).await,
            SchedulingStrategy::LeastConnections => self.select_least_connections(accounts).await,
            SchedulingStrategy::PriorityFirst => self.select_priority_first(accounts).await,
            SchedulingStrategy::Random => self.select_random(accounts).await,
            SchedulingStrategy::WeightedRoundRobin => self.select_weighted(accounts).await,
            SchedulingStrategy::HealthAware => self.select_health_aware(accounts).await,
            SchedulingStrategy::Smart => self.select_smart(accounts).await,
        };

        // 6. 设置全局粘性会话
        if let (Some(ref account), Some(sid)) = (&selected, session_id) {
            self.set_sticky_session(sid.to_string(), account.id).await;
        }

        // 7. 更新运行时状态
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
            if let Some(account) = self
                .account_service
                .get_with_credential(sticky.account_id)
                .await?
            {
                return Ok(Some(account));
            }
        }

        Ok(None)
    }

    /// 获取模型的可用账号列表（用于 failover 重试）
    pub async fn get_available_accounts_for_model(
        &self,
        model: &str,
    ) -> Result<Vec<accounts::Model>> {
        let accounts = self.account_service.get_for_model(model).await?;
        Ok(self.filter_available_accounts(accounts).await)
    }

    /// 尝试分组级调度：按账号 group_id 分组，查 group 的调度策略，走 GroupSchedulerState
    async fn try_group_aware_select(
        &self,
        accounts: &[accounts::Model],
        session_id: Option<&str>,
    ) -> Option<accounts::Model> {
        // 收集所有有 group_id 的账号的分组 ID
        let group_ids: Vec<i64> = accounts
            .iter()
            .filter_map(|a| a.group_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if group_ids.is_empty() {
            return None;
        }

        // 按 group_id 分组账号
        for group_id in &group_ids {
            let group_accounts: Vec<GroupAccountInfo> = accounts
                .iter()
                .filter(|a| a.group_id == Some(*group_id))
                .map(|a| GroupAccountInfo {
                    id: a.id,
                    provider: a.provider.clone(),
                    priority: a.priority,
                    status: a.status.clone(),
                    concurrent_limit: a.concurrent_limit.unwrap_or(5),
                })
                .collect();

            if group_accounts.is_empty() {
                continue;
            }

            // 获取或创建分组调度状态
            let group_state = self.get_or_create_group_state(*group_id).await;

            // 构建 session key（带分组前缀）
            let session_key = session_id.map(|sid| format!("g:{group_id}:{sid}"));

            // 通过分组策略选择账号
            if let Some(selected_id) = group_state
                .select(
                    &group_accounts,
                    session_key.as_deref(),
                    &self.group_sticky_sessions,
                    &self.scheduler_metrics,
                )
                .await
            {
                // 找到对应的完整 account Model
                if let Some(account) = accounts.iter().find(|a| a.id == selected_id) {
                    return Some(account.clone());
                }
            }
        }

        None
    }

    /// 获取或创建分组调度状态
    async fn get_or_create_group_state(&self, group_id: i64) -> Arc<GroupSchedulerState> {
        // 先尝试读缓存
        {
            let states = self.group_states.read().await;
            if let Some(state) = states.get(&group_id) {
                return Arc::clone(state);
            }
        }

        // 从 DB 加载分组配置
        let policy = match groups::Entity::find_by_id(group_id)
            .one(&self.db)
            .await
        {
            Ok(Some(group)) => group.policy(),
            _ => crate::entity::groups::GroupSchedulingPolicy::default(),
        };

        let state = Arc::new(GroupSchedulerState::new(group_id, policy));
        let mut states = self.group_states.write().await;
        states.entry(group_id).or_insert(Arc::clone(&state));
        state
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
    async fn filter_available_accounts(
        &self,
        accounts: Vec<accounts::Model>,
    ) -> Vec<accounts::Model> {
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
    async fn select_least_connections(
        &self,
        accounts: Vec<accounts::Model>,
    ) -> Option<accounts::Model> {
        let states = self.runtime_states.read().await;

        accounts.into_iter().min_by_key(|a| {
            states
                .get(&a.id)
                .map(|s| s.current_connections)
                .unwrap_or(0)
        })
    }

    /// 优先级优先选择
    async fn select_priority_first(
        &self,
        accounts: Vec<accounts::Model>,
    ) -> Option<accounts::Model> {
        accounts.into_iter().max_by_key(|a| a.priority)
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
            health_score: 100.0,
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
        self.failover_manager
            .mark_failure(account_id, "Request failed".to_string())
            .await;
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

        sessions
            .retain(|_, session| (now - session.last_accessed).num_seconds() <= max_age_seconds);
    }

    /// 健康感知调度 - 优先选择健康分数高的账号
    async fn select_health_aware(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        if accounts.is_empty() {
            return None;
        }

        // 如果有健康评分器，使用健康分数
        if let Some(ref scorer) = self.health_scorer {
            let mut scored_accounts = Vec::new();
            for account in accounts {
                let score = scorer.get_score(account.id).await;
                scored_accounts.push((account, score));
            }

            // 按健康分数降序排序，选择最健康的
            scored_accounts
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            return scored_accounts.into_iter().next().map(|(a, _)| a);
        }

        // 回退到优先级优先
        self.select_priority_first(accounts).await
    }

    /// 智能调度 - 综合考虑健康分数、连接数、优先级
    async fn select_smart(&self, accounts: Vec<accounts::Model>) -> Option<accounts::Model> {
        if accounts.is_empty() {
            return None;
        }

        let states = self.runtime_states.read().await;

        // 计算每个账号的综合得分
        let mut scored_accounts: Vec<(accounts::Model, f64)> = accounts
            .into_iter()
            .map(|account| {
                let state = states.get(&account.id);

                // 健康分数（权重 40%）
                let health_score = if let Some(ref scorer) = self.health_scorer {
                    futures::executor::block_on(scorer.get_score(account.id))
                } else {
                    // 从运行时状态估算
                    state
                        .map(|s| {
                            if s.total_requests > 0 {
                                let success_rate =
                                    1.0 - (s.total_errors as f64 / s.total_requests as f64);
                                success_rate * 100.0
                            } else {
                                100.0
                            }
                        })
                        .unwrap_or(100.0)
                };

                // 连接负载（权重 30%）
                let conn_score = state
                    .map(|s| {
                        let limit = account.concurrent_limit.unwrap_or(5) as f64;
                        let used = s.current_connections as f64;
                        // 使用率越低，得分越高
                        (1.0 - (used / limit).min(1.0)) * 100.0
                    })
                    .unwrap_or(100.0);

                // 优先级（权重 30%）
                let priority_score = (account.priority as f64 + 1.0) / 10.0 * 100.0;

                // 综合得分
                let total = health_score * 0.4 + conn_score * 0.3 + priority_score * 0.3;

                (account, total)
            })
            .collect();

        // 按综合得分降序排序
        scored_accounts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored_accounts.into_iter().next().map(|(a, _)| a)
    }

    /// 更新账号健康分数缓存
    pub async fn update_health_scores(&self) {
        if let Some(ref scorer) = self.health_scorer {
            let mut states = self.runtime_states.write().await;
            for (account_id, state) in states.iter_mut() {
                state.health_score = scorer.get_score(*account_id).await;
            }
        }
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
            health_score: 100.0,
        };

        assert_eq!(state.current_connections, 5);
        assert_eq!(state.total_requests, 100);
    }
}
