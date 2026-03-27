//! 应用入口

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod entity;
mod gateway;
mod handler;
mod health;
mod model;
mod service;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "foxnio=info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 初始化加密服务
    if std::env::var("FOXNIO_MASTER_KEY").is_ok() {
        match utils::init_encryption_service() {
            Ok(()) => tracing::info!("Encryption service initialized"),
            Err(e) => {
                tracing::warn!("Failed to initialize encryption service: {}. Sensitive data will not be encrypted.", e);
            }
        }
    } else {
        tracing::warn!("FOXNIO_MASTER_KEY not set. Sensitive data will not be encrypted. Set FOXNIO_MASTER_KEY for production use.");
    }

    // 加载配置
    let config = config::Config::load()
        .unwrap_or_else(|_| {
            tracing::warn!("Using default config");
            config::Config::default()
        });
    
    tracing::info!("🦊 FoxNIO starting...");
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);

    // 连接数据库
    let db = db::connect(&config.database).await?;
    tracing::info!("Database connected");

    // 连接 Redis
    let redis = db::redis_connect(&config.redis).await?;
    tracing::info!("Redis connected");

    // 运行数据库迁移
    tracing::info!("Running migrations...");
    db::run_migrations(&db).await?;
    tracing::info!("Migrations completed");

    // 创建健康检查器
    let health_checker = Arc::new(health::HealthChecker::new()
        .with_timeout(std::time::Duration::from_secs(5))
        .with_retries(3));
    
    // 注册健康检查
    health_checker.register(Box::new(
        health::PostgresHealthCheck::new(db.sqlx.clone())
    )).await;
    
    health_checker.register(Box::new(
        health::RedisHealthCheck::new(redis.clone())
    )).await;
    
    health_checker.register(Box::new(
        health::SystemResourceHealthCheck::new()
            .with_cpu_threshold(90.0)
            .with_memory_threshold(90.0)
            .with_disk_threshold(90.0)
    )).await;
    
    tracing::info!("Health checker initialized with {} checks", health_checker.check_names().await.len());

    // 构建应用
    let app = gateway::build_app(gateway::AppState {
        db: db.clone(),
        redis: redis.clone(),
        config: config.clone(),
    }, health_checker);

    // 启动服务器
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    tracing::info!("🦊 FoxNIO listening on {}", addr);
    tracing::info!("API: http://{}/v1/models", addr);
    tracing::info!("Health: http://{}/health", addr);
    tracing::info!("Admin: http://{}/api/v1/admin/users", addr);
    
    axum::serve(listener, app).await?;

    Ok(())
}
