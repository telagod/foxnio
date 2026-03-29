//! 使用记录工作池 - Usage Record Worker Pool
//!
//! 异步处理使用记录的工作池

#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use super::usage_log::{UsageLog, UsageLogEntry};

/// 工作池配置
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    pub worker_count: usize,
    pub queue_size: usize,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            worker_count: 4,
            queue_size: 10000,
            batch_size: 100,
            flush_interval_ms: 1000,
        }
    }
}

/// 工作池统计
#[derive(Debug, Clone, Default)]
pub struct WorkerPoolStats {
    pub total_processed: u64,
    pub total_failed: u64,
    pub queue_size: usize,
    pub active_workers: usize,
}

/// 使用记录工作池
pub struct UsageRecordWorkerPool {
    usage_log: UsageLog,
    config: WorkerPoolConfig,
    
    // 任务队列
    sender: Option<mpsc::Sender<UsageLogEntry>>,
    
    // 统计信息
    stats: Arc<RwLock<WorkerPoolStats>>,
    
    // 停止信号
    stop_signal: Arc<RwLock<bool>>,
}

impl UsageRecordWorkerPool {
    /// 创建新的工作池
    pub fn new(
        db: sea_orm::DatabaseConnection,
        config: WorkerPoolConfig,
    ) -> Self {
        Self {
            usage_log: UsageLog::new(db),
            config,
            sender: None,
            stats: Arc::new(RwLock::new(WorkerPoolStats::default())),
            stop_signal: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 启动工作池
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!(
            "启动使用记录工作池，工作者数量: {}",
            self.config.worker_count
        );
        
        // 创建任务队列
        let (sender, receiver) = mpsc::channel(self.config.queue_size);
        self.sender = Some(sender);
        
        // 启动工作者
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        
        for worker_id in 0..self.config.worker_count {
            let receiver = receiver.clone();
            let usage_log = UsageLog::new(self.usage_log.db.clone());
            let stats = self.stats.clone();
            let stop_signal = self.stop_signal.clone();
            let batch_size = self.config.batch_size;
            let flush_interval_ms = self.config.flush_interval_ms;
            
            tokio::spawn(async move {
                Self::worker_loop(
                    worker_id,
                    receiver,
                    usage_log,
                    stats,
                    stop_signal,
                    batch_size,
                    flush_interval_ms,
                ).await;
            });
        }
        
        Ok(())
    }
    
    /// 停止工作池
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("停止使用记录工作池");
        
        let mut stop = self.stop_signal.write().await;
        *stop = true;
        
        // 等待队列清空
        // TODO: 实现优雅关闭
        
        Ok(())
    }
    
    /// 提交使用记录
    pub async fn submit(&self, entry: UsageLogEntry) -> Result<()> {
        if let Some(sender) = &self.sender {
            sender.send(entry).await?;
        }
        Ok(())
    }
    
    /// 批量提交
    pub async fn submit_batch(&self, entries: Vec<UsageLogEntry>) -> Result<()> {
        if let Some(sender) = &self.sender {
            for entry in entries {
                sender.send(entry).await?;
            }
        }
        Ok(())
    }
    
    /// 工作者循环
    async fn worker_loop(
        worker_id: usize,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<UsageLogEntry>>>,
        usage_log: UsageLog,
        stats: Arc<RwLock<WorkerPoolStats>>,
        stop_signal: Arc<RwLock<bool>>,
        batch_size: usize,
        flush_interval_ms: u64,
    ) {
        tracing::info!("工作者 {} 启动", worker_id);
        
        let mut batch = Vec::with_capacity(batch_size);
        let mut last_flush = std::time::Instant::now();
        
        loop {
            // 检查停止信号
            if *stop_signal.read().await {
                // 刷新剩余记录
                if !batch.is_empty() {
                    Self::flush_batch(&usage_log, &stats, &mut batch).await;
                }
                break;
            }
            
            // 尝试接收记录
            let entry = {
                let mut rx = receiver.lock().await;
                tokio::select! {
                    Some(entry) = rx.recv() => Some(entry),
                    _ = tokio::time::sleep(
                        std::time::Duration::from_millis(100)
                    ) => None,
                }
            };
            
            if let Some(entry) = entry {
                batch.push(entry);
                
                // 检查是否需要刷新
                if batch.len() >= batch_size {
                    Self::flush_batch(&usage_log, &stats, &mut batch).await;
                    last_flush = std::time::Instant::now();
                }
            } else {
                // 定期刷新
                if last_flush.elapsed().as_millis() as u64 >= flush_interval_ms
                    && !batch.is_empty()
                {
                    Self::flush_batch(&usage_log, &stats, &mut batch).await;
                    last_flush = std::time::Instant::now();
                }
            }
        }
        
        tracing::info!("工作者 {} 停止", worker_id);
    }
    
    /// 刷新批次
    async fn flush_batch(
        usage_log: &UsageLog,
        stats: &Arc<RwLock<WorkerPoolStats>>,
        batch: &mut Vec<UsageLogEntry>,
    ) {
        if batch.is_empty() {
            return;
        }
        
        let entries = std::mem::take(batch);
        let count = entries.len() as u64;
        
        match usage_log.insert_batch(entries).await {
            Ok(_) => {
                let mut s = stats.write().await;
                s.total_processed += count;
            }
            Err(e) => {
                tracing::error!("批量插入使用记录失败: {}", e);
                let mut s = stats.write().await;
                s.total_failed += count;
            }
        }
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> WorkerPoolStats {
        let stats = self.stats.read().await.clone();
        
        // 更新队列大小
        let queue_size = if let Some(sender) = &self.sender {
            sender.capacity()
        } else {
            0
        };
        
        WorkerPoolStats {
            queue_size,
            ..stats
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_worker_pool() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = WorkerPoolConfig::default();
        let mut pool = UsageRecordWorkerPool::new(db, config);
        
        pool.start().await.unwrap();
        
        let stats = pool.get_stats().await;
        assert_eq!(stats.total_processed, 0);
        
        pool.stop().await.unwrap();
    }
}
