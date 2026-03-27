//! 应用状态

use std::sync::Arc;
use crate::db::RedisPool;
use crate::config::Config;
use sea_orm::DatabaseConnection;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Arc<RedisPool>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: DatabaseConnection, redis: RedisPool, config: Config) -> Self {
        Self {
            db,
            redis: Arc::new(redis),
            config: Arc::new(config),
        }
    }
}

/// 共享状态
pub type SharedState = Arc<AppState>;
