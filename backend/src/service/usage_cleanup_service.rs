//! 使用量清理服务 - Usage Cleanup Service
//!
//! 后台运行使用量清理任务

#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::usage_cleanup::{UsageCleanup, UsageCleanupTask, USAGE_CLEANUP_STATUS_RUNNING};

/// 清理服务配置
#[derive(Debug, Clone)]
pub struct UsageCleanupServiceConfig {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub batch_size: u64,
    pub stale_task_timeout_secs: i64,
    pub max_concurrent_tasks: usize,
}

impl Default for UsageCleanupServiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 30,
            batch_size: 1000,
            stale_task_timeout_secs: 3600,
            max_concurrent_tasks: 3,
        }
    }
}

/// 使用量清理服务
pub struct UsageCleanupService {
    cleanup: UsageCleanup,
    config: UsageCleanupServiceConfig,
    stop_signal: Arc<RwLock<bool>>,
    running_tasks: Arc<RwLock<Vec<i64>>>,
}

impl UsageCleanupService {
    /// 创建新的清理服务
    pub fn new(
        db: sea_orm::DatabaseConnection,
        config: UsageCleanupServiceConfig,
    ) -> Self {
        Self {
            cleanup: UsageCleanup::new(db),
            config,
            stop_signal: Arc::new(RwLock::new(false)),
            running_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// 启动清理服务
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("使用量清理服务已禁用");
            return Ok(());
        }
        
        tracing::info!("启动使用量清理服务");
        
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.poll_interval_secs)
        );
        
        loop {
            if *self.stop_signal.read().await {
                break;
            }
            
            interval.tick().await;
            
            // 检查并发任务数
            let running_count = self.running_tasks.read().await.len();
            if running_count >= self.config.max_concurrent_tasks {
                continue;
            }
            
            // 尝试抢占任务
            if let Err(e) = self.try_claim_and_run_task().await {
                tracing::error!("处理清理任务失败: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// 停止清理服务
    pub async fn stop(&self) -> Result<()> {
        let mut stop = self.stop_signal.write().await;
        *stop = true;
        Ok(())
    }
    
    /// 尝试抢占并运行任务
    async fn try_claim_and_run_task(&self) -> Result<()> {
        // 抢占任务
        let task = self.cleanup
            .claim_next_pending_task(self.config.stale_task_timeout_secs)
            .await?;
        
        let task = match task {
            Some(t) => t,
            None => return Ok(()),
        };
        
        // 添加到运行列表
        {
            let mut running = self.running_tasks.write().await;
            running.push(task.id);
        }
        
        // 运行任务
        let result = self.run_task(&task).await;
        
        // 从运行列表移除
        {
            let mut running = self.running_tasks.write().await;
            running.retain(|&id| id != task.id);
        }
        
        // 处理结果
        match result {
            Ok(deleted_rows) => {
                self.cleanup
                    .mark_task_succeeded(task.id, deleted_rows)
                    .await?;
                
                tracing::info!("清理任务 {} 完成，删除 {} 行", task.id, deleted_rows);
            }
            Err(e) => {
                let error_msg = e.to_string();
                let deleted_rows = 0; // TODO: 获取已删除的行数
                
                self.cleanup
                    .mark_task_failed(task.id, deleted_rows, error_msg)
                    .await?;
                
                tracing::error!("清理任务 {} 失败: {}", task.id, e);
            }
        }
        
        Ok(())
    }
    
    /// 运行单个清理任务
    async fn run_task(&self, task: &UsageCleanupTask) -> Result<i64> {
        tracing::info!("开始运行清理任务 {}", task.id);
        
        let mut total_deleted = 0i64;
        let mut deleted;
        
        loop {
            // 检查任务是否被取消
            let status = self.cleanup.get_task_status(task.id).await?;
            if status != USAGE_CLEANUP_STATUS_RUNNING {
                tracing::info!("清理任务 {} 被取消", task.id);
                break;
            }
            
            // 批量删除
            deleted = self.cleanup
                .delete_usage_logs_batch(&task.filters, self.config.batch_size)
                .await?;
            
            if deleted == 0 {
                break;
            }
            
            total_deleted += deleted;
            
            // 更新进度
            self.cleanup
                .update_task_progress(task.id, total_deleted)
                .await?;
            
            // 短暂休眠避免过载
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        Ok(total_deleted)
    }
    
    /// 获取运行中的任务数
    pub async fn get_running_task_count(&self) -> usize {
        self.running_tasks.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore = "SQLite driver not compiled in, requires real database"]
    async fn test_usage_cleanup_service() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let config = UsageCleanupServiceConfig::default();
        let service = UsageCleanupService::new(db, config);
        
        let count = service.get_running_task_count().await;
        assert_eq!(count, 0);
    }
}
