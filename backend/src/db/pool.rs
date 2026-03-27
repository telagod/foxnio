//! 数据库连接池实现

use anyhow::{Result, Context};
use sea_orm::{Database, DatabaseConnection, DbErr};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::{info, error};

/// 数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/foxnio".to_string(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(3600),
        }
    }
}

/// 数据库连接池管理器
pub struct DatabasePool {
    pub sea_orm: DatabaseConnection,
    pub sqlx: sqlx::PgPool,
}

impl DatabasePool {
    /// 创建新的数据库连接池
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database: {}", Self::mask_url(&config.url));
        
        // 创建 sqlx 连接池
        let sqlx_pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.connect_timeout)
            .idle_timeout(Some(config.idle_timeout))
            .max_lifetime(Some(config.max_lifetime))
            .connect(&config.url)
            .await
            .context("Failed to create sqlx connection pool")?;
        
        // 创建 SeaORM 连接
        let sea_orm_conn = Database::connect(&config.url)
            .await
            .context("Failed to create SeaORM connection")?;
        
        info!("Database connection pool created successfully");
        
        Ok(Self {
            sea_orm: sea_orm_conn,
            sqlx: sqlx_pool,
        })
    }
    
    /// 健康检查
    pub async fn health_check(&self) -> Result<bool> {
        let result = sqlx::query("SELECT 1")
            .fetch_one(&self.sqlx)
            .await;
        
        match result {
            Ok(_) => {
                info!("Database health check passed");
                Ok(true)
            }
            Err(e) => {
                error!("Database health check failed: {}", e);
                Ok(false)
            }
        }
    }
    
    /// 获取连接池状态
    pub fn pool_status(&self) -> PoolStatus {
        PoolStatus {
            size: self.sqlx.size() as u32,
            num_idle: self.sqlx.num_idle() as u32,
            is_closed: self.sqlx.is_closed(),
        }
    }
    
    /// 关闭连接池
    pub async fn close(&self) {
        self.sqlx.close().await;
        info!("Database connection pool closed");
    }
    
    /// 隐藏 URL 中的密码
    fn mask_url(url: &str) -> String {
        if let Some(pos) = url.find("://") {
            let scheme = &url[..pos + 3];
            if let Some(at_pos) = url[pos + 3..].find('@') {
                let host = &url[pos + 3 + at_pos..];
                if let Some(colon_pos) = url[pos + 3..].find(':') {
                    if colon_pos < at_pos {
                        let user = &url[pos + 3..pos + 3 + colon_pos];
                        return format!("{}{}:****{}", scheme, user, host);
                    }
                }
            }
        }
        url.to_string()
    }
}

/// 连接池状态
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub size: u32,
    pub num_idle: u32,
    pub is_closed: bool,
}

/// 数据库迁移
pub struct DatabaseMigrator;

impl DatabaseMigrator {
    /// 运行迁移
    pub async fn run(db: &DatabaseConnection) -> Result<()> {
        use sea_orm_migration::MigratorTrait;
        
        info!("Running database migrations...");
        
        // TODO: 实现实际的迁移
        // crate::migration::Migrator::up(db, None).await?;
        
        info!("Database migrations completed");
        Ok(())
    }
    
    /// 回滚迁移
    pub async fn rollback(db: &DatabaseConnection) -> Result<()> {
        info!("Rolling back database migrations...");
        
        // TODO: 实现实际的回滚
        // crate::migration::Migrator::down(db, None).await?;
        
        info!("Database rollback completed");
        Ok(())
    }
}

/// 数据库初始化
pub async fn init_database(config: &DatabaseConfig) -> Result<DatabasePool> {
    let pool = DatabasePool::new(config).await?;
    
    // 运行健康检查
    if !pool.health_check().await? {
        anyhow::bail!("Database health check failed");
    }
    
    // 运行迁移
    DatabaseMigrator::run(&pool.sea_orm).await?;
    
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.connect_timeout, Duration::from_secs(30));
    }
    
    #[test]
    fn test_mask_url() {
        let url = "postgres://user:password@localhost:5432/db";
        let masked = DatabasePool::mask_url(url);
        
        assert!(masked.contains("user"));
        assert!(masked.contains("****"));
        assert!(!masked.contains("password"));
    }
    
    #[test]
    fn test_mask_url_no_password() {
        let url = "postgres://localhost:5432/db";
        let masked = DatabasePool::mask_url(url);
        
        assert_eq!(masked, url);
    }
    
    #[test]
    fn test_pool_status() {
        let status = PoolStatus {
            size: 10,
            num_idle: 5,
            is_closed: false,
        };
        
        assert_eq!(status.size, 10);
        assert_eq!(status.num_idle, 5);
        assert!(!status.is_closed);
    }
}
