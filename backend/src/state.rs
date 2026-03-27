//! 应用状态

use std::sync::Arc;
use crate::db::{DatabasePool, RedisPool};
use crate::config::Config;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: DatabasePool,
    pub redis: RedisPool,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: DatabasePool, redis: RedisPool, config: Config) -> Self {
        Self {
            db,
            redis,
            config: Arc::new(config),
        }
    }
}

/// 共享状态
pub type SharedState = Arc<AppState>;
