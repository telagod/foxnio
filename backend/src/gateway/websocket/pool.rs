//! WebSocket 连接池管理
//!
//! 提供连接池管理、连接复用、健康检查等功能
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use super::{WSConfig, WSConnectionState, WSError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::debug;

/// WebSocket 连接 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(String);

impl ConnectionId {
    pub fn new(account_id: i64, seq: u64) -> Self {
        Self(format!("ws_{account_id}_{seq}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 连接统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后使用时间
    pub last_used_at: DateTime<Utc>,
    /// 发送消息数
    pub messages_sent: u64,
    /// 接收消息数
    pub messages_received: u64,
    /// 错误次数
    pub error_count: u64,
    /// 是否预热连接
    pub is_prewarmed: bool,
}

/// 内部连接状态（使用原子操作实现内部可变性）
#[derive(Debug)]
struct ConnectionState {
    /// 连接 ID
    id: ConnectionId,
    /// 账户 ID
    account_id: i64,
    /// WebSocket URL
    ws_url: String,
    /// 连接状态（原子存储）
    state: AtomicU8,
    /// 等待者数量
    waiters: AtomicU32,
    /// 创建时间戳（纳秒）
    created_at_nano: AtomicI64,
    /// 最后使用时间戳（纳秒）
    last_used_nano: AtomicI64,
    /// 是否已释放
    released: AtomicBool,
}

impl ConnectionState {
    fn new(id: ConnectionId, account_id: i64, ws_url: String) -> Self {
        let now_nano = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);

        Self {
            id,
            account_id,
            ws_url,
            state: AtomicU8::new(Self::state_to_u8(WSConnectionState::Connecting)),
            waiters: AtomicU32::new(0),
            created_at_nano: AtomicI64::new(now_nano),
            last_used_nano: AtomicI64::new(now_nano),
            released: AtomicBool::new(false),
        }
    }

    fn state_to_u8(state: WSConnectionState) -> u8 {
        match state {
            WSConnectionState::Connecting => 0,
            WSConnectionState::Connected => 1,
            WSConnectionState::Idle => 2,
            WSConnectionState::Busy => 3,
            WSConnectionState::Closing => 4,
            WSConnectionState::Closed => 5,
        }
    }

    fn u8_to_state(value: u8) -> WSConnectionState {
        match value {
            0 => WSConnectionState::Connecting,
            1 => WSConnectionState::Connected,
            2 => WSConnectionState::Idle,
            3 => WSConnectionState::Busy,
            4 => WSConnectionState::Closing,
            _ => WSConnectionState::Closed,
        }
    }

    fn get_state(&self) -> WSConnectionState {
        Self::u8_to_state(self.state.load(Ordering::Acquire))
    }

    fn set_state(&self, state: WSConnectionState) {
        self.state
            .store(Self::state_to_u8(state), Ordering::Release);
    }

    fn touch(&self) {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        self.last_used_nano.store(now, Ordering::Release);
    }

    fn age(&self) -> Duration {
        let created = self.created_at_nano.load(Ordering::Acquire);
        if created <= 0 {
            return Duration::ZERO;
        }
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Duration::from_nanos((now - created).max(0) as u64)
    }

    fn idle_duration(&self) -> Duration {
        let last_used = self.last_used_nano.load(Ordering::Acquire);
        if last_used <= 0 {
            return Duration::ZERO;
        }
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Duration::from_nanos((now - last_used).max(0) as u64)
    }

    fn is_leased(&self) -> bool {
        matches!(self.get_state(), WSConnectionState::Busy)
    }
}

/// 池化连接包装器
pub struct PooledConnection {
    /// 内部状态
    state: Arc<ConnectionState>,
    /// 连接池引用
    pool: Arc<ConnectionPoolInner>,
    /// 排队等待时间
    queue_wait: Duration,
    /// 连接选择时间
    conn_pick: Duration,
    /// 是否复用
    reused: bool,
}

impl PooledConnection {
    /// 获取连接 ID
    pub fn id(&self) -> &ConnectionId {
        &self.state.id
    }

    /// 获取排队等待时间
    pub fn queue_wait(&self) -> Duration {
        self.queue_wait
    }

    /// 获取连接选择时间
    pub fn conn_pick(&self) -> Duration {
        self.conn_pick
    }

    /// 是否复用连接
    pub fn is_reused(&self) -> bool {
        self.reused
    }

    /// 标记为损坏
    pub fn mark_broken(&self) {
        self.state.released.store(true, Ordering::Release);
        self.state.set_state(WSConnectionState::Closed);
    }

    /// 释放连接回池
    pub fn release(&self) {
        if self.state.released.swap(true, Ordering::AcqRel) {
            return;
        }
        self.state.set_state(WSConnectionState::Idle);
        self.state.touch();
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        self.release();
    }
}

/// 账户连接池
#[derive(Debug, Default)]
struct AccountPool {
    /// 连接映射
    connections: HashMap<ConnectionId, Arc<ConnectionState>>,
    /// 正在创建的连接数
    creating: usize,
    /// 最后清理时间
    last_cleanup: Option<Instant>,
}

/// 连接池内部实现
struct ConnectionPoolInner {
    /// 配置
    config: WSConfig,
    /// 账户池映射
    accounts: Mutex<HashMap<i64, AccountPool>>,
    /// 序列号生成器
    seq: AtomicU64,
    /// 池统计信息
    stats: PoolStats,
    /// 关闭信号
    shutdown: AtomicBool,
}

/// 池统计信息
#[derive(Debug, Default)]
struct PoolStats {
    /// 总获取次数
    acquire_total: AtomicU64,
    /// 复用次数
    acquire_reuse: AtomicU64,
    /// 新建次数
    acquire_create: AtomicU64,
    /// 排队等待次数
    queue_wait_total: AtomicU64,
    /// 排队等待总毫秒数
    queue_wait_ms: AtomicU64,
    /// 扩容次数
    scale_up: AtomicU64,
    /// 缩容次数
    scale_down: AtomicU64,
}

/// 连接池
pub struct ConnectionPool {
    inner: Arc<ConnectionPoolInner>,
}

impl ConnectionPool {
    /// 创建新连接池
    pub fn new(config: WSConfig) -> Self {
        let inner = Arc::new(ConnectionPoolInner {
            config,
            accounts: Mutex::new(HashMap::new()),
            seq: AtomicU64::new(0),
            stats: PoolStats::default(),
            shutdown: AtomicBool::new(false),
        });

        // 启动后台任务
        let inner_clone = inner.clone();
        tokio::spawn(async move {
            inner_clone.run_background_tasks().await;
        });

        Self { inner }
    }

    /// 获取连接
    pub async fn acquire(
        &self,
        account_id: i64,
        ws_url: String,
        preferred_conn_id: Option<&ConnectionId>,
        force_new: bool,
    ) -> Result<PooledConnection, WSError> {
        self.inner
            .stats
            .acquire_total
            .fetch_add(1, Ordering::Relaxed);

        // 检查是否已关闭
        if self.inner.shutdown.load(Ordering::Acquire) {
            return Err(WSError::ConnectionClosed);
        }

        let pick_start = tokio::time::Instant::now();

        // 尝试复用现有连接
        if !force_new {
            if let Some(conn) = self
                .try_reuse_connection(account_id, preferred_conn_id)
                .await?
            {
                let conn_pick = pick_start.elapsed();
                self.inner
                    .stats
                    .acquire_reuse
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(PooledConnection {
                    state: conn,
                    pool: self.inner.clone(),
                    queue_wait: Duration::ZERO,
                    conn_pick,
                    reused: true,
                });
            }
        }

        // 创建新连接
        let conn_pick = pick_start.elapsed();
        let conn = self.create_connection(account_id, ws_url).await?;
        self.inner
            .stats
            .acquire_create
            .fetch_add(1, Ordering::Relaxed);

        Ok(PooledConnection {
            state: conn,
            pool: self.inner.clone(),
            queue_wait: Duration::ZERO,
            conn_pick,
            reused: false,
        })
    }

    /// 尝试复用连接
    async fn try_reuse_connection(
        &self,
        account_id: i64,
        preferred_conn_id: Option<&ConnectionId>,
    ) -> Result<Option<Arc<ConnectionState>>, WSError> {
        let mut accounts = self.inner.accounts.lock().await;

        if let Some(account_pool) = accounts.get_mut(&account_id) {
            // 优先使用指定的连接
            if let Some(conn_id) = preferred_conn_id {
                if let Some(conn) = account_pool.connections.get(conn_id) {
                    if !conn.is_leased() && conn.waiters.load(Ordering::Acquire) == 0 {
                        conn.set_state(WSConnectionState::Busy);
                        conn.touch();
                        return Ok(Some(conn.clone()));
                    }
                }
            }

            // 选择最少等待的空闲连接
            let best = account_pool
                .connections
                .values()
                .filter(|c| !c.is_leased() && c.waiters.load(Ordering::Acquire) == 0)
                .min_by_key(|c| c.waiters.load(Ordering::Acquire));

            if let Some(conn) = best {
                conn.set_state(WSConnectionState::Busy);
                conn.touch();
                return Ok(Some(conn.clone()));
            }
        }

        Ok(None)
    }

    /// 创建新连接
    async fn create_connection(
        &self,
        account_id: i64,
        ws_url: String,
    ) -> Result<Arc<ConnectionState>, WSError> {
        // 生成连接 ID
        let seq = self.inner.seq.fetch_add(1, Ordering::Relaxed);
        let conn_id = ConnectionId::new(account_id, seq);

        // 创建连接状态
        let conn = Arc::new(ConnectionState::new(
            conn_id.clone(),
            account_id,
            ws_url.clone(),
        ));

        // 添加到池
        {
            let mut accounts = self.inner.accounts.lock().await;
            let account_pool = accounts
                .entry(account_id)
                .or_insert_with(AccountPool::default);

            // 检查是否达到上限
            if account_pool.connections.len() + account_pool.creating
                >= self.inner.config.max_connections
            {
                return Err(WSError::PoolFull);
            }

            account_pool.creating += 1;
        }

        // 执行连接（这里先创建状态，实际连接由 handler 处理）
        conn.set_state(WSConnectionState::Idle);

        // 添加到池
        {
            let mut accounts = self.inner.accounts.lock().await;
            if let Some(account_pool) = accounts.get_mut(&account_id) {
                account_pool.creating -= 1;
                account_pool.connections.insert(conn_id, conn.clone());
            }
        }

        self.inner.stats.scale_up.fetch_add(1, Ordering::Relaxed);
        Ok(conn)
    }

    /// 获取池统计信息
    pub fn stats(&self) -> PoolStatsSnapshot {
        PoolStatsSnapshot {
            acquire_total: self.inner.stats.acquire_total.load(Ordering::Relaxed),
            acquire_reuse: self.inner.stats.acquire_reuse.load(Ordering::Relaxed),
            acquire_create: self.inner.stats.acquire_create.load(Ordering::Relaxed),
            queue_wait_total: self.inner.stats.queue_wait_total.load(Ordering::Relaxed),
            queue_wait_ms: self.inner.stats.queue_wait_ms.load(Ordering::Relaxed),
            scale_up: self.inner.stats.scale_up.load(Ordering::Relaxed),
            scale_down: self.inner.stats.scale_down.load(Ordering::Relaxed),
        }
    }

    /// 关闭连接池
    pub fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Release);
    }
}

/// 池统计快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatsSnapshot {
    pub acquire_total: u64,
    pub acquire_reuse: u64,
    pub acquire_create: u64,
    pub queue_wait_total: u64,
    pub queue_wait_ms: u64,
    pub scale_up: u64,
    pub scale_down: u64,
}

impl ConnectionPoolInner {
    /// 运行后台任务
    async fn run_background_tasks(&self) {
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(self.config.heartbeat_interval_seconds));
        let mut cleanup_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    self.run_heartbeat().await;
                }
                _ = cleanup_interval.tick() => {
                    self.run_cleanup().await;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if self.shutdown.load(Ordering::Acquire) {
                        break;
                    }
                }
            }
        }
    }

    /// 运行心跳检查
    async fn run_heartbeat(&self) {
        let accounts = self.accounts.lock().await;

        for (account_id, account_pool) in accounts.iter() {
            for (conn_id, conn) in &account_pool.connections {
                if conn.is_leased() || conn.waiters.load(Ordering::Acquire) > 0 {
                    continue;
                }

                // 检查空闲时间
                if conn.idle_duration() >= Duration::from_secs(self.config.idle_timeout_seconds) {
                    debug!(
                        account_id = %account_id,
                        conn_id = %conn_id,
                        "Connection idle, will be cleaned up"
                    );
                }
            }
        }
    }

    /// 运行清理任务
    async fn run_cleanup(&self) {
        let mut accounts = self.accounts.lock().await;
        let now = Instant::now();
        let max_age = Duration::from_secs(self.config.max_age_seconds);

        for (account_id, account_pool) in accounts.iter_mut() {
            let mut to_remove = Vec::new();

            for (conn_id, conn) in &account_pool.connections {
                // 清理已关闭的连接
                if matches!(conn.get_state(), WSConnectionState::Closed) {
                    to_remove.push(conn_id.clone());
                    continue;
                }

                // 清理过期连接
                if !conn.is_leased() && conn.age() > max_age {
                    to_remove.push(conn_id.clone());
                }
            }

            for conn_id in to_remove {
                account_pool.connections.remove(&conn_id);
                self.stats.scale_down.fetch_add(1, Ordering::Relaxed);
                debug!(
                    account_id = %account_id,
                    conn_id = %conn_id,
                    "Connection removed from pool"
                );
            }

            account_pool.last_cleanup = Some(now);
        }
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
