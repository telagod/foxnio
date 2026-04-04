//! 应用入口

#![allow(dead_code)]

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod alert;
mod config;
mod db;
mod entity;
mod gateway;
mod handler;
mod health;
mod metrics;
mod model;
mod openapi;
mod service;
mod state;
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

    // 初始化指标系统
    metrics::init_metrics();

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
    let config = config::Config::load().unwrap_or_else(|error| {
        tracing::warn!("Failed to load config: {}. Using default config", error);
        config::Config::default()
    });

    tracing::info!("🦊 FoxNIO starting...");

    // 创建数据库配置
    let db_config = db::pool::DatabaseConfig {
        url: config.database_url(),
        max_connections: config.database.max_connections,
        ..Default::default()
    };

    // 连接数据库
    let db_pool = db::init_database(&db_config).await?;
    tracing::info!("Database connected");

    // 创建 Redis 配置
    let redis_config = db::redis::RedisConfig {
        url: config.redis_url(),
        ..Default::default()
    };

    // 连接 Redis
    let redis = db::init_redis(&redis_config).await?;
    tracing::info!("Redis connected");

    // 创建健康检查器
    let health_checker = Arc::new(
        health::HealthChecker::new()
            .with_timeout(std::time::Duration::from_secs(5))
            .with_retries(3),
    );

    // 注册健康检查
    health_checker
        .register(Box::new(health::PostgresHealthCheck::new(
            db_pool.sqlx.clone(),
        )))
        .await;

    health_checker
        .register(Box::new(health::RedisHealthCheck::new(redis.clone())))
        .await;

    health_checker
        .register(Box::new(
            health::SystemResourceHealthCheck::new()
                .with_cpu_threshold(90.0)
                .with_memory_threshold(90.0)
                .with_disk_threshold(90.0),
        ))
        .await;

    tracing::info!(
        "Health checker initialized with {} checks",
        health_checker.check_names().await.len()
    );

    // 构建应用
    let app = gateway::build_app(
        state::AppState::new(db_pool.sea_orm.clone(), redis, config.clone()),
        health_checker,
    );

    // 启动服务器
    let addr = config.server.bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("🦊 FoxNIO listening on {}", addr);
    tracing::info!("API: http://{}/v1/models", addr);
    tracing::info!("Health: http://{}/health", addr);
    tracing::info!("Admin: http://{}/api/v1/admin/users", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
