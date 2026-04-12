//! Wait Queue Service
//!
//! 等待队列机制，支持粘性会话优先等待
//! Redis 持久化：每个 account 使用 hash `foxnio:wait_queue:{account_id}`

#![allow(dead_code)]

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::db::redis::RedisPool;

/// 等待请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitRequest {
    pub request_id: String,
    pub account_id: i64,
    pub session_hash: String,
    pub created_at: DateTime<Utc>,
    pub timeout: Duration,
    pub priority: WaitPriority,
}

/// 等待优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaitPriority {
    /// 粘性会话优先
    StickySession = 0,
    /// 普通等待
    Normal = 1,
    /// 降级等待
    Fallback = 2,
}

/// 队列状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub account_id: i64,
    pub queue_length: u32,
    pub max_capacity: u32,
    pub estimated_wait_time: Duration,
    pub sticky_sessions: u32,
    pub normal_requests: u32,
}

/// 等待队列服务
pub struct WaitQueueService {
    /// 内存队列（用于快速访问）
    queues: Arc<RwLock<std::collections::HashMap<i64, VecDeque<WaitRequest>>>>,
    /// Redis 连接池（用于分布式持久化）
    redis: Option<Arc<RedisPool>>,
    /// 默认超时时间
    default_timeout: Duration,
    /// 最大队列长度
    max_queue_length: u32,
    /// 清理间隔
    cleanup_interval: Duration,
}

/// Redis key prefix for wait queue hashes
const REDIS_KEY_PREFIX: &str = "foxnio:wait_queue";

impl WaitQueueService {
    pub fn new(
        redis: Option<Arc<RedisPool>>,
        default_timeout: Duration,
        max_queue_length: u32,
    ) -> Self {
        Self {
            queues: Arc::new(RwLock::new(std::collections::HashMap::new())),
            redis,
            default_timeout,
            max_queue_length,
            cleanup_interval: Duration::from_secs(60),
        }
    }

    /// Redis hash key for a given account
    fn redis_key(account_id: i64) -> String {
        format!("{REDIS_KEY_PREFIX}:{account_id}")
    }

    /// 将请求加入队列
    pub async fn enqueue(&self, mut req: WaitRequest) -> Result<u32> {
        // 设置默认超时
        if req.timeout.is_zero() {
            req.timeout = self.default_timeout;
        }

        let account_id = req.account_id;

        // 内存队列
        let mut queues = self.queues.write().await;
        let queue = queues.entry(account_id).or_insert_with(VecDeque::new);

        // 检查队列容量
        if queue.len() >= self.max_queue_length as usize {
            bail!("Queue is full for account {}", account_id);
        }

        // 按优先级插入（粘性会话优先）
        let position = if req.priority == WaitPriority::StickySession {
            // 找到第一个非粘性会话的位置
            let pos = queue
                .iter()
                .position(|r| r.priority != WaitPriority::StickySession)
                .unwrap_or(queue.len());
            queue.insert(pos, req.clone());
            pos as u32
        } else {
            queue.push_back(req.clone());
            (queue.len() - 1) as u32
        };

        // Redis 持久化
        if let Some(ref redis) = self.redis {
            let key = Self::redis_key(account_id);
            let value = serde_json::to_string(&req).unwrap_or_default();
            if let Err(e) = redis.hset(&key, &req.request_id, &value).await {
                error!("Failed to persist wait request to Redis: {}", e);
            }
        }

        debug!(
            "Enqueued request to account {} at position {}",
            account_id, position
        );

        Ok(position)
    }

    /// 尝试从队列获取请求
    pub async fn try_acquire(&self, account_id: i64) -> Option<WaitRequest> {
        let mut queues = self.queues.write().await;
        let queue = queues.get_mut(&account_id)?;

        // 移除过期的请求
        let now = Utc::now();
        while let Some(front) = queue.front() {
            let elapsed = (now - front.created_at).num_seconds() as u64;
            if elapsed > front.timeout.as_secs() {
                let expired = queue.pop_front();
                if let Some(ref expired_req) = expired {
                    self.remove_from_redis(account_id, &expired_req.request_id)
                        .await;
                }
                warn!("Removed expired request: {:?}", expired);
            } else {
                break;
            }
        }

        // 返回第一个有效的请求
        let acquired = queue.pop_front();
        if let Some(ref req) = acquired {
            self.remove_from_redis(account_id, &req.request_id).await;
        }
        acquired
    }

    /// 获取队列长度
    pub async fn get_queue_length(&self, account_id: i64) -> u32 {
        let queues = self.queues.read().await;
        queues.get(&account_id).map(|q| q.len() as u32).unwrap_or(0)
    }

    /// 获取队列状态
    pub async fn get_queue_status(&self, account_id: i64) -> QueueStatus {
        let queues = self.queues.read().await;
        let queue = queues.get(&account_id);

        if let Some(queue) = queue {
            let sticky_count = queue
                .iter()
                .filter(|r| r.priority == WaitPriority::StickySession)
                .count() as u32;

            let normal_count = queue
                .iter()
                .filter(|r| r.priority == WaitPriority::Normal)
                .count() as u32;

            // 估算等待时间（基于平均处理时间）
            let estimated_wait = Duration::from_secs(queue.len() as u64 * 5);

            QueueStatus {
                account_id,
                queue_length: queue.len() as u32,
                max_capacity: self.max_queue_length,
                estimated_wait_time: estimated_wait,
                sticky_sessions: sticky_count,
                normal_requests: normal_count,
            }
        } else {
            QueueStatus {
                account_id,
                queue_length: 0,
                max_capacity: self.max_queue_length,
                estimated_wait_time: Duration::from_secs(0),
                sticky_sessions: 0,
                normal_requests: 0,
            }
        }
    }

    /// 清理过期请求
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut queues = self.queues.write().await;
        let mut removed = 0;
        let now = Utc::now();
        let mut expired_ids: Vec<(i64, String)> = Vec::new();

        for (&account_id, queue) in queues.iter_mut() {
            let before = queue.len();

            // Collect expired request IDs before retaining
            for req in queue.iter() {
                let elapsed = (now - req.created_at).num_seconds() as u64;
                if elapsed > req.timeout.as_secs() {
                    expired_ids.push((account_id, req.request_id.clone()));
                }
            }

            queue.retain(|req| {
                let elapsed = (now - req.created_at).num_seconds() as u64;
                elapsed <= req.timeout.as_secs()
            });

            removed += before - queue.len();
        }

        // Clean expired entries from Redis
        if let Some(ref redis) = self.redis {
            for (account_id, request_id) in &expired_ids {
                let key = Self::redis_key(*account_id);
                if let Err(e) = redis.hdel(&key, request_id).await {
                    error!("Failed to remove expired request from Redis: {}", e);
                }
            }
        }

        if removed > 0 {
            info!("Cleaned up {} expired wait requests", removed);
        }

        Ok(removed)
    }

    /// 取消请求
    pub async fn cancel(&self, account_id: i64, request_id: &str) -> Result<bool> {
        let mut queues = self.queues.write().await;

        if let Some(queue) = queues.get_mut(&account_id) {
            let before = queue.len();
            queue.retain(|req| req.request_id != request_id);

            if queue.len() < before {
                self.remove_from_redis(account_id, request_id).await;
                debug!(
                    "Cancelled request {} from account {}",
                    request_id, account_id
                );
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 获取所有队列状态
    pub async fn get_all_status(&self) -> Vec<QueueStatus> {
        let queues = self.queues.read().await;
        let mut statuses = Vec::new();

        for &account_id in queues.keys() {
            let status = self.get_queue_status(account_id).await;
            statuses.push(status);
        }

        statuses
    }

    /// Remove a single request from Redis
    async fn remove_from_redis(&self, account_id: i64, request_id: &str) {
        if let Some(ref redis) = self.redis {
            let key = Self::redis_key(account_id);
            if let Err(e) = redis.hdel(&key, request_id).await {
                error!("Failed to remove request {} from Redis: {}", request_id, e);
            }
        }
    }

    /// Restore queue state from Redis on startup
    ///
    /// Reads all `foxnio:wait_queue:*` hashes and rebuilds the in-memory
    /// queues, skipping entries that have already expired.
    pub async fn restore_from_redis(&self, account_ids: &[i64]) -> Result<usize> {
        let redis = match self.redis {
            Some(ref r) => r,
            None => return Ok(0),
        };

        let now = Utc::now();
        let mut restored = 0usize;
        let mut queues = self.queues.write().await;

        for &account_id in account_ids {
            let key = Self::redis_key(account_id);
            let entries = match redis.hgetall(&key).await {
                Ok(m) => m,
                Err(e) => {
                    error!(
                        "Failed to read wait queue from Redis for account {}: {}",
                        account_id, e
                    );
                    continue;
                }
            };

            let queue = queues.entry(account_id).or_insert_with(VecDeque::new);

            for (request_id, value) in &entries {
                let req: WaitRequest = match serde_json::from_str(value) {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(
                            "Skipping malformed wait request {} in Redis: {}",
                            request_id, e
                        );
                        // Remove corrupt entry
                        let _ = redis.hdel(&key, request_id).await;
                        continue;
                    }
                };

                // Skip expired
                let elapsed = (now - req.created_at).num_seconds() as u64;
                if elapsed > req.timeout.as_secs() {
                    let _ = redis.hdel(&key, request_id).await;
                    continue;
                }

                // Insert respecting priority order
                if req.priority == WaitPriority::StickySession {
                    let pos = queue
                        .iter()
                        .position(|r| r.priority != WaitPriority::StickySession)
                        .unwrap_or(queue.len());
                    queue.insert(pos, req);
                } else {
                    queue.push_back(req);
                }
                restored += 1;
            }
        }

        if restored > 0 {
            info!("Restored {} wait requests from Redis", restored);
        }

        Ok(restored)
    }

    /// 启动后台清理任务
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(self.cleanup_interval).await;

                match self.cleanup_expired().await {
                    Ok(count) => {
                        if count > 0 {
                            debug!("Cleanup task removed {} expired requests", count);
                        }
                    }
                    Err(e) => {
                        warn!("Cleanup task failed: {}", e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_and_acquire() {
        let service = WaitQueueService::new(None, Duration::from_secs(300), 100);

        let req = WaitRequest {
            request_id: "req-1".into(),
            account_id: 1,
            session_hash: "session-1".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };

        let position = service.enqueue(req.clone()).await.unwrap();
        assert_eq!(position, 0);

        let length = service.get_queue_length(1).await;
        assert_eq!(length, 1);

        let acquired = service.try_acquire(1).await.unwrap();
        assert_eq!(acquired.request_id, "req-1");

        let length = service.get_queue_length(1).await;
        assert_eq!(length, 0);
    }

    #[tokio::test]
    async fn test_priority_queue() {
        let service = WaitQueueService::new(None, Duration::from_secs(300), 100);

        // 添加普通请求
        let normal_req = WaitRequest {
            request_id: "normal-1".into(),
            account_id: 1,
            session_hash: "session-1".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };
        service.enqueue(normal_req).await.unwrap();

        // 添加粘性会话请求
        let sticky_req = WaitRequest {
            request_id: "sticky-1".into(),
            account_id: 1,
            session_hash: "session-2".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::StickySession,
        };
        service.enqueue(sticky_req).await.unwrap();

        // 粘性会话应该排在前面
        let acquired = service.try_acquire(1).await.unwrap();
        assert_eq!(acquired.request_id, "sticky-1");
        assert_eq!(acquired.priority, WaitPriority::StickySession);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let service = WaitQueueService::new(
            None,
            Duration::from_secs(300),
            2, // 最大容量 2
        );

        let req1 = WaitRequest {
            request_id: "req-1".into(),
            account_id: 1,
            session_hash: "session-1".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };

        let req2 = WaitRequest {
            request_id: "req-2".into(),
            account_id: 1,
            session_hash: "session-2".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };

        let req3 = WaitRequest {
            request_id: "req-3".into(),
            account_id: 1,
            session_hash: "session-3".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };

        assert!(service.enqueue(req1).await.is_ok());
        assert!(service.enqueue(req2).await.is_ok());
        assert!(service.enqueue(req3).await.is_err()); // 队列已满
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let service = WaitQueueService::new(None, Duration::from_secs(300), 100);

        // 添加已过期的请求
        let expired_req = WaitRequest {
            request_id: "expired-1".into(),
            account_id: 1,
            session_hash: "session-1".into(),
            created_at: Utc::now() - chrono::Duration::seconds(100),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };
        service.enqueue(expired_req).await.unwrap();

        // 添加有效的请求
        let valid_req = WaitRequest {
            request_id: "valid-1".into(),
            account_id: 1,
            session_hash: "session-2".into(),
            created_at: Utc::now(),
            timeout: Duration::from_secs(60),
            priority: WaitPriority::Normal,
        };
        service.enqueue(valid_req).await.unwrap();

        assert_eq!(service.get_queue_length(1).await, 2);

        // 清理过期请求
        let removed = service.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        assert_eq!(service.get_queue_length(1).await, 1);
    }
}
