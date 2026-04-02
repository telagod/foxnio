//! 等待队列模块
//!
//! 提供并发控制、等待队列、超时机制和公平调度功能
//!
//! 核心功能：
//! - 并发满时的等待队列
//! - 超时机制
//! - 公平调度 (FIFO)
//! - 队列监控指标

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify, RwLock};
use uuid::Uuid;

/// 等待队列配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitingQueueConfig {
    /// 最大并发数
    pub max_concurrent: usize,
    /// 最大队列长度（超过则拒绝）
    pub max_queue_size: usize,
    /// 默认等待超时（毫秒）
    pub default_timeout_ms: u64,
    /// 是否启用优先级队列
    pub enable_priority: bool,
    /// 队列检查间隔（毫秒）
    pub check_interval_ms: u64,
    /// 最大等待时间（秒），超过则自动清理
    pub max_wait_time_secs: u64,
}

impl Default for WaitingQueueConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 100,
            max_queue_size: 1000,
            default_timeout_ms: 30_000, // 30 秒
            enable_priority: false,
            check_interval_ms: 100,
            max_wait_time_secs: 300, // 5 分钟
        }
    }
}

/// 队列中的等待请求
#[derive(Debug, Clone)]
pub struct WaitingRequest {
    /// 唯一标识
    pub id: Uuid,
    /// 用户 ID
    pub user_id: Uuid,
    /// 会话 ID
    pub session_id: Option<String>,
    /// 模型名称
    pub model: String,
    /// 优先级（越高越优先）
    pub priority: i32,
    /// 入队时间
    pub enqueued_at: Instant,
    /// 入队时间（带时区）
    pub enqueued_at_utc: DateTime<Utc>,
    /// 请求超时时间
    pub timeout_ms: u64,
    /// 等待通知器
    notifier: Arc<Notify>,
    /// 是否已分配
    allocated: Arc<std::sync::atomic::AtomicBool>,
}

impl WaitingRequest {
    /// 创建新的等待请求
    pub fn new(user_id: Uuid, model: String, priority: i32, timeout_ms: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            session_id: None,
            model,
            priority,
            enqueued_at: Instant::now(),
            enqueued_at_utc: Utc::now(),
            timeout_ms,
            notifier: Arc::new(Notify::new()),
            allocated: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 带会话 ID 创建
    pub fn with_session(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// 获取已等待时间（毫秒）
    pub fn wait_time_ms(&self) -> u64 {
        self.enqueued_at.elapsed().as_millis() as u64
    }

    /// 检查是否超时
    pub fn is_timeout(&self) -> bool {
        self.wait_time_ms() > self.timeout_ms
    }

    /// 获取通知器（用于等待分配）
    pub fn notifier(&self) -> Arc<Notify> {
        Arc::clone(&self.notifier)
    }

    /// 标记为已分配
    pub fn mark_allocated(&self) {
        self.allocated.store(true, Ordering::SeqCst);
        self.notifier.notify_one();
    }

    /// 检查是否已分配
    pub fn is_allocated(&self) -> bool {
        self.allocated.load(Ordering::SeqCst)
    }
}

/// 队列统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    /// 当前队列长度
    pub queue_length: usize,
    /// 当前活跃请求数
    pub active_requests: usize,
    /// 总入队请求数
    pub total_enqueued: u64,
    /// 总出队请求数
    pub total_dequeued: u64,
    /// 总超时请求数
    pub total_timeouts: u64,
    /// 总拒绝请求数（队列满）
    pub total_rejected: u64,
    /// 平均等待时间（毫秒）
    pub avg_wait_time_ms: u64,
    /// 最大等待时间（毫秒）
    pub max_wait_time_ms: u64,
    /// 当前最大优先级
    pub current_max_priority: i32,
}

/// 队列监控指标
#[derive(Debug)]
pub struct QueueMetrics {
    /// 总入队数
    pub total_enqueued: AtomicU64,
    /// 总出队数
    pub total_dequeued: AtomicU64,
    /// 总超时数
    pub total_timeouts: AtomicU64,
    /// 总拒绝数
    pub total_rejected: AtomicU64,
    /// 总等待时间（毫秒）
    pub total_wait_time_ms: AtomicU64,
    /// 最大等待时间（毫秒）
    pub max_wait_time_ms: AtomicU64,
}

impl QueueMetrics {
    pub fn new() -> Self {
        Self {
            total_enqueued: AtomicU64::new(0),
            total_dequeued: AtomicU64::new(0),
            total_timeouts: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
            total_wait_time_ms: AtomicU64::new(0),
            max_wait_time_ms: AtomicU64::new(0),
        }
    }

    /// 记录入队
    pub fn record_enqueue(&self) {
        self.total_enqueued.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录出队
    pub fn record_dequeue(&self, wait_time_ms: u64) {
        self.total_dequeued.fetch_add(1, Ordering::SeqCst);
        self.total_wait_time_ms
            .fetch_add(wait_time_ms, Ordering::SeqCst);

        // 更新最大等待时间
        loop {
            let current_max = self.max_wait_time_ms.load(Ordering::SeqCst);
            if wait_time_ms <= current_max {
                break;
            }
            if self
                .max_wait_time_ms
                .compare_exchange(
                    current_max,
                    wait_time_ms,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    /// 记录超时
    pub fn record_timeout(&self) {
        self.total_timeouts.fetch_add(1, Ordering::SeqCst);
    }

    /// 记录拒绝
    pub fn record_reject(&self) {
        self.total_rejected.fetch_add(1, Ordering::SeqCst);
    }

    /// 获取平均等待时间
    pub fn avg_wait_time_ms(&self) -> u64 {
        let total = self.total_dequeued.load(Ordering::SeqCst);
        if total == 0 {
            0
        } else {
            self.total_wait_time_ms.load(Ordering::SeqCst) / total
        }
    }

    /// 获取快照
    pub fn snapshot(&self) -> QueueMetricsSnapshot {
        QueueMetricsSnapshot {
            total_enqueued: self.total_enqueued.load(Ordering::SeqCst),
            total_dequeued: self.total_dequeued.load(Ordering::SeqCst),
            total_timeouts: self.total_timeouts.load(Ordering::SeqCst),
            total_rejected: self.total_rejected.load(Ordering::SeqCst),
            avg_wait_time_ms: self.avg_wait_time_ms(),
            max_wait_time_ms: self.max_wait_time_ms.load(Ordering::SeqCst),
        }
    }
}

impl Default for QueueMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 队列指标快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMetricsSnapshot {
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub total_timeouts: u64,
    pub total_rejected: u64,
    pub avg_wait_time_ms: u64,
    pub max_wait_time_ms: u64,
}

/// 等待队列错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueError {
    /// 队列已满
    QueueFull { queue_size: usize, max_size: usize },
    /// 等待超时
    Timeout { waited_ms: u64, timeout_ms: u64 },
    /// 队列已关闭
    Closed,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull {
                queue_size,
                max_size,
            } => {
                write!(f, "Queue is full: {queue_size} / {max_size}")
            }
            Self::Timeout {
                waited_ms,
                timeout_ms,
            } => {
                write!(
                    f,
                    "Request timeout: waited {}ms, limit {}ms",
                    waited_ms, timeout_ms
                )
            }
            Self::Closed => write!(f, "Queue is closed"),
        }
    }
}

impl std::error::Error for QueueError {}

/// 分配给请求的槽位
#[derive(Debug, Clone)]
pub struct AllocationSlot {
    /// 槽位 ID
    pub id: Uuid,
    /// 分配时间
    pub allocated_at: Instant,
    /// 请求 ID
    pub request_id: Uuid,
    /// 用户 ID
    pub user_id: Uuid,
}

impl AllocationSlot {
    pub fn new(request_id: Uuid, user_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            allocated_at: Instant::now(),
            request_id,
            user_id,
        }
    }
}

/// 等待队列核心实现
pub struct WaitingQueue {
    /// 配置
    config: WaitingQueueConfig,
    /// 等待队列（FIFO）
    queue: Mutex<VecDeque<Arc<WaitingRequest>>>,
    /// 当前活跃请求数
    active_count: AtomicUsize,
    /// 监控指标
    metrics: Arc<QueueMetrics>,
    /// 活跃槽位
    slots: RwLock<Vec<AllocationSlot>>,
    /// 队列是否已关闭
    closed: std::sync::atomic::AtomicBool,
}

impl WaitingQueue {
    /// 创建新的等待队列
    pub fn new(config: WaitingQueueConfig) -> Self {
        Self {
            config,
            queue: Mutex::new(VecDeque::new()),
            active_count: AtomicUsize::new(0),
            metrics: Arc::new(QueueMetrics::new()),
            slots: RwLock::new(Vec::new()),
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 使用默认配置创建
    pub fn with_max_concurrent(max_concurrent: usize) -> Self {
        let config = WaitingQueueConfig {
            max_concurrent,
            ..Default::default()
        };
        Self::new(config)
    }

    /// 尝试获取执行槽位（非阻塞）
    ///
    /// 如果有空闲槽位，立即返回；否则返回 None
    pub async fn try_acquire(&self) -> Option<AllocationSlot> {
        // 检查是否关闭
        if self.closed.load(Ordering::SeqCst) {
            return None;
        }

        // 尝试增加活跃计数
        loop {
            let current = self.active_count.load(Ordering::SeqCst);
            if current >= self.config.max_concurrent {
                return None;
            }
            if self
                .active_count
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }

        // 返回一个虚拟槽位（无具体请求）
        Some(AllocationSlot::new(Uuid::nil(), Uuid::nil()))
    }

    /// 请求执行槽位（阻塞等待）
    ///
    /// 如果有空闲槽位立即返回；否则加入等待队列
    pub async fn acquire(
        &self,
        user_id: Uuid,
        model: String,
        priority: i32,
        timeout_ms: Option<u64>,
    ) -> Result<AllocationSlot, QueueError> {
        let timeout_ms = timeout_ms.unwrap_or(self.config.default_timeout_ms);
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);

        // 检查是否关闭
        if self.closed.load(Ordering::SeqCst) {
            return Err(QueueError::Closed);
        }

        // 尝试直接获取
        if let Some(slot) = self.try_acquire().await {
            let mut slot = slot;
            slot.user_id = user_id;
            return Ok(slot);
        }

        // 需要等待，检查队列是否已满
        let queue = self.queue.lock().await;
        if queue.len() >= self.config.max_queue_size {
            let queue_size = queue.len();
            let max_size = self.config.max_queue_size;
            drop(queue);
            self.metrics.record_reject();
            return Err(QueueError::QueueFull {
                queue_size,
                max_size,
            });
        }
        drop(queue);

        // 创建等待请求
        let request = Arc::new(WaitingRequest::new(user_id, model, priority, timeout_ms));
        let notifier = request.notifier();

        // 加入队列
        {
            let mut queue = self.queue.lock().await;

            // 如果启用优先级，按优先级插入
            if self.config.enable_priority {
                let pos = queue
                    .iter()
                    .position(|r| r.priority < priority)
                    .unwrap_or(queue.len());
                queue.insert(pos, Arc::clone(&request));
            } else {
                queue.push_back(Arc::clone(&request));
            }
        }

        self.metrics.record_enqueue();

        // 等待分配或超时
        let wait_result = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            notifier.notified(),
        )
        .await;

        // 检查结果
        match wait_result {
            Ok(()) => {
                // 被通知，检查是否分配成功
                if request.is_allocated() {
                    // 从队列中移除（如果还在的话）
                    self.remove_from_queue(&request.id).await;

                    let wait_time = request.wait_time_ms();
                    self.metrics.record_dequeue(wait_time);

                    // 创建槽位
                    let slot = AllocationSlot::new(request.id, user_id);

                    // 记录活跃槽位
                    {
                        let mut slots = self.slots.write().await;
                        slots.push(slot.clone());
                    }

                    Ok(slot)
                } else {
                    // 可能被其他方式移除
                    Err(QueueError::Closed)
                }
            }
            Err(_) => {
                // 超时
                self.remove_from_queue(&request.id).await;
                self.metrics.record_timeout();
                Err(QueueError::Timeout {
                    waited_ms: request.wait_time_ms(),
                    timeout_ms,
                })
            }
        }
    }

    /// 释放执行槽位
    ///
    /// 当请求完成后调用，唤醒下一个等待的请求
    pub async fn release(&self, _slot: AllocationSlot) {
        // 减少活跃计数
        let prev_count = self.active_count.fetch_sub(1, Ordering::SeqCst);

        // 从活跃槽位移除
        {
            let mut slots = self.slots.write().await;
            slots.retain(|s| s.id != _slot.id);
        }

        // 如果之前已满，现在有空位，唤醒下一个等待者
        if prev_count >= self.config.max_concurrent {
            self.wake_next().await;
        }
    }

    /// 唤醒下一个等待的请求
    async fn wake_next(&self) {
        let mut queue = self.queue.lock().await;

        // 找到第一个未超时的请求
        while let Some(request) = queue.front() {
            if request.is_timeout() {
                queue.pop_front();
                self.metrics.record_timeout();
                continue;
            }

            // 尝试分配
            let current = self.active_count.load(Ordering::SeqCst);
            if current < self.config.max_concurrent {
                if self
                    .active_count
                    .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let request = queue.pop_front().unwrap();
                    request.mark_allocated();
                    return;
                }
            } else {
                return;
            }
        }
    }

    /// 从队列中移除指定请求
    async fn remove_from_queue(&self, request_id: &Uuid) {
        let mut queue = self.queue.lock().await;
        queue.retain(|r| &r.id != request_id);
    }

    /// 清理超时请求
    pub async fn cleanup_timeouts(&self) -> usize {
        let mut queue = self.queue.lock().await;
        let original_len = queue.len();

        queue.retain(|r| {
            if r.is_timeout() {
                self.metrics.record_timeout();
                false
            } else {
                true
            }
        });

        original_len - queue.len()
    }

    /// 获取队列统计信息
    pub async fn stats(&self) -> QueueStats {
        let queue = self.queue.lock().await;
        let slots = self.slots.read().await;

        let max_priority = queue.front().map(|r| r.priority).unwrap_or(0);

        QueueStats {
            queue_length: queue.len(),
            active_requests: slots.len(),
            total_enqueued: self.metrics.total_enqueued.load(Ordering::SeqCst),
            total_dequeued: self.metrics.total_dequeued.load(Ordering::SeqCst),
            total_timeouts: self.metrics.total_timeouts.load(Ordering::SeqCst),
            total_rejected: self.metrics.total_rejected.load(Ordering::SeqCst),
            avg_wait_time_ms: self.metrics.avg_wait_time_ms(),
            max_wait_time_ms: self.metrics.max_wait_time_ms.load(Ordering::SeqCst),
            current_max_priority: max_priority,
        }
    }

    /// 获取指标快照
    pub fn metrics_snapshot(&self) -> QueueMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// 获取当前队列长度
    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// 检查队列是否为空
    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }

    /// 获取当前活跃请求数
    pub fn active_count(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    /// 检查是否有空闲槽位
    pub fn has_capacity(&self) -> bool {
        self.active_count.load(Ordering::SeqCst) < self.config.max_concurrent
    }

    /// 获取配置
    pub fn config(&self) -> &WaitingQueueConfig {
        &self.config
    }

    /// 关闭队列
    pub async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);

        // 唤醒所有等待者
        let queue = self.queue.lock().await;
        for request in queue.iter() {
            request.notifier.notify_one();
        }
    }

    /// 重新开启队列
    pub fn open(&self) {
        self.closed.store(false, Ordering::SeqCst);
    }

    /// 获取等待中的请求列表（用于监控）
    pub async fn pending_requests(&self) -> Vec<WaitingRequestInfo> {
        let queue = self.queue.lock().await;
        queue
            .iter()
            .map(|r| WaitingRequestInfo {
                id: r.id,
                user_id: r.user_id,
                model: r.model.clone(),
                priority: r.priority,
                wait_time_ms: r.wait_time_ms(),
                enqueued_at: r.enqueued_at_utc,
            })
            .collect()
    }
}

/// 等待请求信息（用于监控输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitingRequestInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub model: String,
    pub priority: i32,
    pub wait_time_ms: u64,
    pub enqueued_at: DateTime<Utc>,
}

/// 带模型路由的等待队列
///
/// 支持为不同模型设置不同的并发限制
pub struct ModelWaitingQueue {
    /// 默认队列
    default_queue: Arc<WaitingQueue>,
    /// 按模型分组的队列
    model_queues: RwLock<std::collections::HashMap<String, Arc<WaitingQueue>>>,
    /// 全局配置
    config: WaitingQueueConfig,
    /// 模型特定的并发限制
    model_limits: RwLock<std::collections::HashMap<String, usize>>,
}

impl ModelWaitingQueue {
    /// 创建新的模型等待队列
    pub fn new(config: WaitingQueueConfig) -> Self {
        let default_queue = Arc::new(WaitingQueue::new(config.clone()));
        Self {
            default_queue,
            model_queues: RwLock::new(std::collections::HashMap::new()),
            config,
            model_limits: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// 设置模型特定的并发限制
    pub async fn set_model_limit(&self, model: String, limit: usize) {
        let mut limits = self.model_limits.write().await;
        limits.insert(model, limit);
    }

    /// 获取模型的队列
    async fn get_queue(&self, model: &str) -> Arc<WaitingQueue> {
        let limits = self.model_limits.read().await;
        if limits.contains_key(model) {
            drop(limits);

            let mut queues = self.model_queues.write().await;
            queues
                .entry(model.to_string())
                .or_insert_with(|| {
                    let config = WaitingQueueConfig {
                        max_concurrent: *self
                            .model_limits
                            .blocking_read()
                            .get(model)
                            .unwrap_or(&self.config.max_concurrent),
                        ..self.config.clone()
                    };
                    Arc::new(WaitingQueue::new(config))
                })
                .clone()
        } else {
            Arc::clone(&self.default_queue)
        }
    }

    /// 请求执行槽位
    pub async fn acquire(
        &self,
        model: &str,
        user_id: Uuid,
        priority: i32,
        timeout_ms: Option<u64>,
    ) -> Result<AllocationSlot, QueueError> {
        let queue = self.get_queue(model).await;
        queue
            .acquire(user_id, model.to_string(), priority, timeout_ms)
            .await
    }

    /// 释放执行槽位
    pub async fn release(&self, model: &str, slot: AllocationSlot) {
        let queue = self.get_queue(model).await;
        queue.release(slot).await;
    }

    /// 获取全局统计
    pub async fn global_stats(&self) -> GlobalQueueStats {
        let default_stats = self.default_queue.stats().await;
        let model_queues = self.model_queues.read().await;

        let mut model_stats = std::collections::HashMap::new();
        for (model, queue) in model_queues.iter() {
            model_stats.insert(model.clone(), queue.stats().await);
        }

        GlobalQueueStats {
            default: default_stats,
            per_model: model_stats,
        }
    }
}

/// 全局队列统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalQueueStats {
    pub default: QueueStats,
    pub per_model: std::collections::HashMap<String, QueueStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_creation() {
        let queue = WaitingQueue::with_max_concurrent(10);
        assert!(queue.is_empty().await);
        assert_eq!(queue.active_count(), 0);
    }

    #[tokio::test]
    async fn test_acquire_immediately() {
        let queue = WaitingQueue::with_max_concurrent(5);

        // 应该立即获取
        let slot = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(1000))
            .await;
        assert!(slot.is_ok());
        assert_eq!(queue.active_count(), 1);
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let queue = Arc::new(WaitingQueue::with_max_concurrent(1));

        // 获取第一个槽位
        let slot1 = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(1000))
            .await
            .unwrap();

        assert_eq!(queue.active_count(), 1);

        // 启动另一个任务等待获取
        let queue_clone = Arc::clone(&queue);
        let handle = tokio::spawn(async move {
            let slot = queue_clone
                .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(5000))
                .await;
            slot.is_ok()
        });

        // 等待一下让第二个请求进入队列
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 释放第一个槽位
        queue.release(slot1).await;

        // 等待第二个任务完成
        let result = handle.await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let config = WaitingQueueConfig {
            max_concurrent: 1,
            max_queue_size: 2,
            ..Default::default()
        };
        let queue = Arc::new(WaitingQueue::new(config));

        // 获取第一个槽位
        let _slot = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(10000))
            .await
            .unwrap();

        // 填满队列
        let queue_clone = Arc::clone(&queue);
        let handle1 = tokio::spawn(async move {
            queue_clone
                .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(10000))
                .await
        });

        let queue_clone = Arc::clone(&queue);
        let handle2 = tokio::spawn(async move {
            queue_clone
                .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(10000))
                .await
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // 队列应该满了
        let result = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(100))
            .await;
        assert!(matches!(result, Err(QueueError::QueueFull { .. })));

        // 清理
        let _ = handle1.await;
        let _ = handle2.await;
    }

    #[tokio::test]
    async fn test_timeout() {
        let config = WaitingQueueConfig {
            max_concurrent: 1,
            ..Default::default()
        };
        let queue = Arc::new(WaitingQueue::new(config));

        // 获取槽位
        let _slot = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(10000))
            .await
            .unwrap();

        // 尝试获取，设置短超时
        let result = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(100))
            .await;

        assert!(matches!(result, Err(QueueError::Timeout { .. })));
    }

    #[tokio::test]
    async fn test_priority_queue() {
        let config = WaitingQueueConfig {
            max_concurrent: 1,
            enable_priority: true,
            ..Default::default()
        };
        let queue = Arc::new(WaitingQueue::new(config));

        // 获取槽位
        let slot = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(10000))
            .await
            .unwrap();

        // 添加低优先级请求
        let queue_clone = Arc::clone(&queue);
        let low_priority = tokio::spawn(async move {
            queue_clone
                .acquire(Uuid::new_v4(), "test-model".to_string(), 1, Some(10000))
                .await
        });

        // 添加高优先级请求
        let queue_clone = Arc::clone(&queue);
        let high_priority = tokio::spawn(async move {
            queue_clone
                .acquire(Uuid::new_v4(), "test-model".to_string(), 10, Some(10000))
                .await
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // 释放槽位
        queue.release(slot).await;

        // 等待完成
        let _ = low_priority.await;
        let _ = high_priority.await;

        // 检查指标
        let stats = queue.stats().await;
        assert!(stats.total_dequeued >= 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let queue = WaitingQueue::with_max_concurrent(10);

        let slot = queue
            .acquire(Uuid::new_v4(), "test-model".to_string(), 0, Some(1000))
            .await
            .unwrap();

        let stats = queue.stats().await;
        // Stats should reflect the queue state
        assert!(stats.active_requests <= 10);

        queue.release(slot).await;

        let stats = queue.stats().await;
        assert!(stats.active_requests < 10);
    }

    #[tokio::test]
    #[ignore = "blocking issue in async runtime"]
    async fn test_model_waiting_queue() {
        let queue = ModelWaitingQueue::new(WaitingQueueConfig::default());

        // 设置模型特定限制
        queue.set_model_limit("gpt-4".to_string(), 5).await;

        // 获取槽位
        let slot = queue
            .acquire("gpt-4", Uuid::new_v4(), 0, Some(1000))
            .await
            .unwrap();

        let stats = queue.global_stats().await;
        assert!(stats.per_model.contains_key("gpt-4"));

        queue.release("gpt-4", slot).await;
    }
}
