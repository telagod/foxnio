//! 时间轮服务
//!
//! 高效的定时任务调度，基于时间轮算法

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Common task action types for the timing wheel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskAction {
    /// HTTP health-check: GET the URL and log success/failure.
    HttpHealthCheck { url: String },
    /// Clean up expired records (tokens, sessions, etc.).
    CleanupExpired,
    /// Refresh authentication tokens.
    RefreshTokens,
    /// Generic / custom action — falls back to task_type + payload.
    Custom { kind: String },
}

/// 定时任务
#[derive(Debug, Clone)]
pub struct TimerTask {
    pub id: u64,
    pub execute_at: DateTime<Utc>,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub action: Option<TaskAction>,
    pub repeat_interval: Option<std::time::Duration>,
}

/// 时间轮槽位
#[derive(Debug, Clone, Default)]
pub struct WheelSlot {
    pub tasks: VecDeque<TimerTask>,
}

/// 时间轮配置
#[derive(Debug, Clone)]
pub struct TimingWheelConfig {
    pub tick_duration_ms: u64,
    pub wheel_size: usize,
    pub max_tasks: usize,
}

impl Default for TimingWheelConfig {
    fn default() -> Self {
        Self {
            tick_duration_ms: 100, // 100ms 每个槽位
            wheel_size: 3600,      // 3600 个槽位 = 6 分钟周期
            max_tasks: 100000,     // 最多 10 万任务
        }
    }
}

/// 时间轮统计
#[derive(Debug, Clone, Default)]
pub struct TimingWheelStats {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub completed_tasks: u64,
    pub expired_tasks: u64,
    pub current_tick: u64,
}

/// 时间轮服务
pub struct TimingWheelService {
    config: TimingWheelConfig,
    wheel: Arc<RwLock<Vec<WheelSlot>>>,
    stats: Arc<RwLock<TimingWheelStats>>,
    current_tick: Arc<RwLock<u64>>,
    next_task_id: Arc<RwLock<u64>>,
}

impl TimingWheelService {
    /// 创建新的时间轮服务
    pub fn new(config: TimingWheelConfig) -> Self {
        let wheel = (0..config.wheel_size)
            .map(|_| WheelSlot::default())
            .collect();

        Self {
            config,
            wheel: Arc::new(RwLock::new(wheel)),
            stats: Arc::new(RwLock::new(TimingWheelStats::default())),
            current_tick: Arc::new(RwLock::new(0)),
            next_task_id: Arc::new(RwLock::new(1)),
        }
    }

    /// 添加定时任务
    pub async fn add_task(
        &self,
        execute_at: DateTime<Utc>,
        task_type: String,
        payload: serde_json::Value,
        repeat_interval: Option<std::time::Duration>,
    ) -> u64 {
        self.add_task_with_action(execute_at, task_type, payload, None, repeat_interval)
            .await
    }

    /// 添加带 action 的定时任务
    pub async fn add_task_with_action(
        &self,
        execute_at: DateTime<Utc>,
        task_type: String,
        payload: serde_json::Value,
        action: Option<TaskAction>,
        repeat_interval: Option<std::time::Duration>,
    ) -> u64 {
        let task_id = {
            let mut next_id = self.next_task_id.write().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let task = TimerTask {
            id: task_id,
            execute_at,
            task_type,
            payload,
            action,
            repeat_interval,
        };

        // 计算槽位
        let slot = self.calculate_slot(&execute_at).await;

        // 添加到槽位
        {
            let mut wheel = self.wheel.write().await;
            wheel[slot].tasks.push_back(task);
        }

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.total_tasks += 1;
        stats.pending_tasks += 1;

        task_id
    }

    /// 移除任务
    pub async fn remove_task(&self, task_id: u64) -> bool {
        let mut wheel = self.wheel.write().await;

        for slot in wheel.iter_mut() {
            if let Some(pos) = slot.tasks.iter().position(|t| t.id == task_id) {
                slot.tasks.remove(pos);

                let mut stats = self.stats.write().await;
                stats.pending_tasks = stats.pending_tasks.saturating_sub(1);

                return true;
            }
        }

        false
    }

    /// 执行 tick
    pub async fn tick(&self) -> Vec<TimerTask> {
        let slot_index = {
            let current_tick = *self.current_tick.read().await;
            (current_tick % self.config.wheel_size as u64) as usize
        };

        // Increment tick after calculating slot index
        let current_tick = {
            let mut tick = self.current_tick.write().await;
            *tick += 1;
            *tick
        };

        let mut tasks_to_execute = Vec::new();
        let mut tasks_to_reschedule = Vec::new();

        {
            let mut wheel = self.wheel.write().await;
            let slot = &mut wheel[slot_index];

            while let Some(task) = slot.tasks.pop_front() {
                let now = Utc::now();

                if task.execute_at <= now {
                    // 任务到期，执行
                    if let Some(interval) = task.repeat_interval {
                        // 重复任务，重新调度
                        let mut new_task = task.clone();
                        new_task.execute_at = now + chrono::Duration::from_std(interval).unwrap();
                        tasks_to_reschedule.push(new_task);
                    }
                    tasks_to_execute.push(task);
                } else {
                    // 任务未到期，保留
                    slot.tasks.push_back(task);
                }
            }
        }

        // 重新调度重复任务
        for task in tasks_to_reschedule {
            self.add_task_with_action(
                task.execute_at,
                task.task_type,
                task.payload,
                task.action,
                task.repeat_interval,
            )
            .await;
        }

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.completed_tasks += tasks_to_execute.len() as u64;
        stats.pending_tasks = stats.pending_tasks.saturating_sub(tasks_to_execute.len());
        stats.current_tick = current_tick;

        tasks_to_execute
    }

    /// 计算槽位
    async fn calculate_slot(&self, execute_at: &DateTime<Utc>) -> usize {
        let now = Utc::now();
        let delay_ms = (*execute_at - now).num_milliseconds().max(0) as u64;
        let ticks_ahead = delay_ms / self.config.tick_duration_ms;

        let current_tick = *self.current_tick.read().await;
        let target_tick = current_tick + ticks_ahead;

        (target_tick % self.config.wheel_size as u64) as usize
    }

    /// 获取待执行任务数量
    pub async fn pending_count(&self) -> usize {
        self.stats.read().await.pending_tasks
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> TimingWheelStats {
        self.stats.read().await.clone()
    }

    /// 清空所有任务
    pub async fn clear(&self) {
        let mut wheel = self.wheel.write().await;
        for slot in wheel.iter_mut() {
            slot.tasks.clear();
        }

        let mut stats = self.stats.write().await;
        stats.pending_tasks = 0;
    }

    /// 获取下一个执行时间
    pub async fn next_execution_time(&self) -> Option<DateTime<Utc>> {
        let wheel = self.wheel.read().await;

        for slot in wheel.iter() {
            if let Some(task) = slot.tasks.front() {
                return Some(task.execute_at);
            }
        }

        None
    }

    /// 启动后台 tick 任务
    pub fn start_background_tick(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let svc = Arc::clone(&self);
        svc.start_ticker()
    }

    /// Spawn a tokio task that calls `tick()` at the configured resolution
    /// and executes each fired task's action.
    pub fn start_ticker(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let tick_ms = self.config.tick_duration_ms;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(tick_ms));

            loop {
                interval.tick().await;
                let tasks = self.tick().await;

                for task in tasks {
                    let action = task.action.clone();
                    // Fire-and-forget per task so one slow task doesn't block the wheel.
                    tokio::spawn(async move {
                        if let Err(e) = execute_task_action(&task, action.as_ref()).await {
                            tracing::error!(
                                task_id = task.id,
                                task_type = %task.task_type,
                                "task execution failed: {e:#}"
                            );
                        }
                    });
                }
            }
        })
    }
}

impl Default for TimingWheelService {
    fn default() -> Self {
        Self::new(TimingWheelConfig::default())
    }
}

/// Execute the concrete action associated with a fired timer task.
async fn execute_task_action(task: &TimerTask, action: Option<&TaskAction>) -> anyhow::Result<()> {
    match action {
        Some(TaskAction::HttpHealthCheck { url }) => {
            tracing::info!(task_id = task.id, url = %url, "http health check");
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()?;
            let resp = client.get(url).send().await?;
            let status = resp.status();
            if status.is_success() {
                tracing::info!(task_id = task.id, status = %status, "health check ok");
            } else {
                tracing::warn!(task_id = task.id, status = %status, "health check failed");
            }
        }
        Some(TaskAction::CleanupExpired) => {
            tracing::info!(task_id = task.id, "cleanup expired records");
            // Placeholder — concrete cleanup logic depends on the caller's DB handle.
            // Integrators should replace this with actual cleanup queries.
        }
        Some(TaskAction::RefreshTokens) => {
            tracing::info!(task_id = task.id, "refresh tokens");
            // Placeholder — token refresh requires service-specific credentials.
        }
        Some(TaskAction::Custom { kind }) => {
            tracing::info!(task_id = task.id, kind = %kind, "custom task action");
        }
        None => {
            // Legacy path: no structured action, log and move on.
            tracing::debug!(
                task_id = task.id,
                task_type = %task.task_type,
                "executed task (no action defined)"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_task() {
        let service = TimingWheelService::default();

        let execute_at = Utc::now() + chrono::Duration::seconds(1);
        let task_id = service
            .add_task(execute_at, "test".to_string(), serde_json::json!({}), None)
            .await;

        assert!(task_id > 0);

        let stats = service.get_stats().await;
        assert_eq!(stats.pending_tasks, 1);
    }

    #[tokio::test]
    async fn test_remove_task() {
        let service = TimingWheelService::default();

        let execute_at = Utc::now() + chrono::Duration::seconds(1);
        let task_id = service
            .add_task(execute_at, "test".to_string(), serde_json::json!({}), None)
            .await;

        let removed = service.remove_task(task_id).await;
        assert!(removed);

        let stats = service.get_stats().await;
        assert_eq!(stats.pending_tasks, 0);
    }

    #[tokio::test]
    async fn test_tick() {
        let service = TimingWheelService::default();

        // 添加已过期的任务
        let execute_at = Utc::now() - chrono::Duration::seconds(1);
        service
            .add_task(execute_at, "test".to_string(), serde_json::json!({}), None)
            .await;

        // 执行 tick
        let tasks = service.tick().await;
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_repeat_task() {
        let service = TimingWheelService::default();

        let execute_at = Utc::now() - chrono::Duration::seconds(1);
        service
            .add_task(
                execute_at,
                "test".to_string(),
                serde_json::json!({}),
                Some(std::time::Duration::from_secs(1)),
            )
            .await;

        // 执行 tick
        let tasks = service.tick().await;
        assert_eq!(tasks.len(), 1);

        // 任务应该被重新调度
        let stats = service.get_stats().await;
        assert!(stats.pending_tasks > 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let service = TimingWheelService::default();

        for _ in 0..5 {
            service
                .add_task(
                    Utc::now() + chrono::Duration::seconds(1),
                    "test".to_string(),
                    serde_json::json!({}),
                    None,
                )
                .await;
        }

        service.clear().await;

        let stats = service.get_stats().await;
        assert_eq!(stats.pending_tasks, 0);
    }
}
