//! 分组级调度策略
//!
//! 每个分组可独立配置调度策略：Sticky / LoadBalance / Scoring
//! 防惊群：slow-start 权重爬坡 + jittered cooldown + Top-K 随机

use crate::entity::groups::GroupSchedulingPolicy;
use crate::gateway::scheduler::metrics::{AccountMetrics, SchedulerMetrics};
use chrono::{DateTime, Utc};
use lru::LruCache;
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 账号信息（调度用轻量结构）
#[derive(Debug, Clone)]
pub struct GroupAccountInfo {
    pub id: Uuid,
    pub provider: String,
    pub priority: i32,
    pub status: String,
    pub concurrent_limit: i32,
}

impl GroupAccountInfo {
    pub fn is_available(&self) -> bool {
        self.status == "active"
    }
}

/// 粘性会话
#[derive(Debug, Clone)]
pub struct GroupStickySession {
    pub account_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

/// 分组调度器状态
pub struct GroupSchedulerState {
    pub group_id: i64,
    pub policy: GroupSchedulingPolicy,
    rr_index: AtomicUsize,
    /// 账号恢复时间戳（防惊群 slow-start）
    recovery_timestamps: RwLock<HashMap<Uuid, Instant>>,
}

/// Slow-start 爬坡时长（秒）
const SLOW_START_DURATION_SECS: f64 = 60.0;
/// Scoring Top-K 候选数
const SCORING_TOP_K: usize = 3;

impl GroupSchedulerState {
    pub fn new(group_id: i64, policy: GroupSchedulingPolicy) -> Self {
        Self {
            group_id,
            policy,
            rr_index: AtomicUsize::new(0),
            recovery_timestamps: RwLock::new(HashMap::new()),
        }
    }

    /// 记录账号恢复时间（从 cooldown/inactive 变为 active）
    pub async fn mark_recovery(&self, account_id: Uuid) {
        let mut map = self.recovery_timestamps.write().await;
        map.insert(account_id, Instant::now());
    }

    /// 清理过期的恢复记录（超过爬坡时长的）
    pub async fn cleanup_recovery_timestamps(&self) {
        let mut map = self.recovery_timestamps.write().await;
        map.retain(|_, ts| ts.elapsed().as_secs_f64() < SLOW_START_DURATION_SECS * 2.0);
    }

    /// 计算 slow-start 有效权重 (0.1 ~ 1.0)
    async fn effective_weight(&self, account_id: Uuid) -> f64 {
        let map = self.recovery_timestamps.read().await;
        if let Some(recovery_time) = map.get(&account_id) {
            let elapsed = recovery_time.elapsed().as_secs_f64();
            let factor = (elapsed / SLOW_START_DURATION_SECS).min(1.0);
            return 0.1 + 0.9 * factor; // 10% → 100%
        }
        1.0
    }

    /// 根据分组策略选择账号
    pub async fn select(
        &self,
        accounts: &[GroupAccountInfo],
        session_key: Option<&str>,
        sticky_sessions: &RwLock<LruCache<String, GroupStickySession>>,
        metrics: &SchedulerMetrics,
    ) -> Option<Uuid> {
        let available: Vec<&GroupAccountInfo> =
            accounts.iter().filter(|a| a.is_available()).collect();
        if available.is_empty() {
            return None;
        }

        match self.policy {
            GroupSchedulingPolicy::Sticky => {
                self.select_sticky(&available, session_key, sticky_sessions)
                    .await
            }
            GroupSchedulingPolicy::LoadBalance => {
                self.select_load_balance(&available, metrics).await
            }
            GroupSchedulingPolicy::Scoring => {
                self.select_scoring(&available, metrics).await
            }
        }
    }

    /// Sticky 策略：强粘性，同 session 始终命中同一账号
    async fn select_sticky(
        &self,
        accounts: &[&GroupAccountInfo],
        session_key: Option<&str>,
        sticky_sessions: &RwLock<LruCache<String, GroupStickySession>>,
    ) -> Option<Uuid> {
        // 1. 有 session_key 时查 sticky session
        if let Some(key) = session_key {
            let mut sessions = sticky_sessions.write().await;
            if let Some(sticky) = sessions.get_mut(key) {
                // 检查账号是否仍在候选列表中
                if accounts.iter().any(|a| a.id == sticky.account_id) {
                    sticky.last_accessed = Utc::now();
                    return Some(sticky.account_id);
                }
                // 账号不可用，移除旧绑定
                sessions.pop(key);
            }
        }

        // 2. 未命中 → round-robin 选一个
        let idx = self.rr_index.fetch_add(1, Ordering::Relaxed) % accounts.len();
        let selected = accounts[idx].id;

        // 3. 绑定 sticky session
        if let Some(key) = session_key {
            let mut sessions = sticky_sessions.write().await;
            sessions.push(
                key.to_string(),
                GroupStickySession {
                    account_id: selected,
                    created_at: Utc::now(),
                    last_accessed: Utc::now(),
                },
            );
        }

        Some(selected)
    }

    /// LoadBalance 策略：跳过粘性，round-robin + slow-start
    async fn select_load_balance(
        &self,
        accounts: &[&GroupAccountInfo],
        _metrics: &SchedulerMetrics,
    ) -> Option<Uuid> {
        // 按 slow-start 权重过滤
        let mut candidates = Vec::with_capacity(accounts.len());
        for account in accounts {
            let weight = self.effective_weight(account.id).await;
            let pass = {
                let mut rng = rand::thread_rng();
                rng.gen::<f64>() < weight
            };
            if pass {
                candidates.push(*account);
            }
        }
        // 如果全被过滤（极端情况），退化为全量
        if candidates.is_empty() {
            candidates = accounts.to_vec();
        }

        let idx = self.rr_index.fetch_add(1, Ordering::Relaxed) % candidates.len();
        Some(candidates[idx].id)
    }

    /// Scoring 策略：多因子评分 + Top-K 随机
    async fn select_scoring(
        &self,
        accounts: &[&GroupAccountInfo],
        metrics: &SchedulerMetrics,
    ) -> Option<Uuid> {
        let mut scored: Vec<(Uuid, f64)> = Vec::with_capacity(accounts.len());

        for account in accounts {
            let account_metrics = metrics.get_account_metrics(account.id).await;
            let slow_start = self.effective_weight(account.id).await;

            let score = compute_score(account, account_metrics.as_ref()) * slow_start;
            scored.push((account.id, score));
        }

        // 降序排列
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Top-K 随机选择，避免惊群
        let top_k = scored.len().min(SCORING_TOP_K);
        if top_k == 0 {
            return None;
        }
        let idx = {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..top_k)
        };
        Some(scored[idx].0)
    }
}

/// 多因子评分
/// priority × 1.0 + (1 - load) × 1.0 + (1 - error_rate) × 0.8 + (1 - norm_latency) × 0.5
fn compute_score(account: &GroupAccountInfo, metrics: Option<&Arc<AccountMetrics>>) -> f64 {
    let priority_score = (account.priority as f64).max(0.0) / 100.0; // 归一化到 0~1

    let (load_score, error_score, latency_score) = if let Some(m) = metrics {
        let active = m.get_active_connections() as f64;
        let limit = account.concurrent_limit.max(1) as f64;
        let load = (1.0 - active / limit).max(0.0);

        let error_rate = m.get_error_rate();
        let error = 1.0 - error_rate;

        let latency_ms = m.get_avg_latency_ms() as f64;
        let latency = 1.0 - (latency_ms / 30_000.0).min(1.0); // 30s 为最差

        (load, error, latency)
    } else {
        (1.0, 1.0, 1.0) // 无指标时给满分
    };

    priority_score * 1.0 + load_score * 1.0 + error_score * 0.8 + latency_score * 0.5
}

/// 给 cooldown 加 jitter（防惊群）
pub fn jittered_cooldown_secs(base_secs: i64) -> i64 {
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0.0..0.2);
    (base_secs as f64 * (1.0 + jitter)).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn make_accounts(n: usize) -> Vec<GroupAccountInfo> {
        (0..n)
            .map(|i| GroupAccountInfo {
                id: Uuid::new_v4(),
                provider: "anthropic".into(),
                priority: (i * 10) as i32,
                status: "active".into(),
                concurrent_limit: 5,
            })
            .collect()
    }

    #[tokio::test]
    async fn test_sticky_binds_and_reuses() {
        let state = GroupSchedulerState::new(1, GroupSchedulingPolicy::Sticky);
        let accounts = make_accounts(3);
        let sticky = RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
        let metrics = SchedulerMetrics::new();

        let first = state
            .select(&accounts, Some("sess-1"), &sticky, &metrics)
            .await
            .unwrap();
        let second = state
            .select(&accounts, Some("sess-1"), &sticky, &metrics)
            .await
            .unwrap();
        assert_eq!(first, second, "Sticky should return same account");
    }

    #[tokio::test]
    async fn test_load_balance_rotates() {
        let state = GroupSchedulerState::new(1, GroupSchedulingPolicy::LoadBalance);
        let accounts = make_accounts(3);
        let sticky = RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
        let metrics = SchedulerMetrics::new();

        let mut seen = std::collections::HashSet::new();
        for _ in 0..30 {
            let id = state
                .select(&accounts, None, &sticky, &metrics)
                .await
                .unwrap();
            seen.insert(id);
        }
        assert!(seen.len() > 1, "LoadBalance should rotate across accounts");
    }

    #[tokio::test]
    async fn test_scoring_selects() {
        let state = GroupSchedulerState::new(1, GroupSchedulingPolicy::Scoring);
        let accounts = make_accounts(5);
        let sticky = RwLock::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
        let metrics = SchedulerMetrics::new();

        let result = state
            .select(&accounts, None, &sticky, &metrics)
            .await;
        assert!(result.is_some());
    }

    #[test]
    fn test_jittered_cooldown() {
        let base = 60;
        for _ in 0..100 {
            let jittered = jittered_cooldown_secs(base);
            assert!(jittered >= base);
            assert!(jittered <= (base as f64 * 1.2).ceil() as i64);
        }
    }
}

