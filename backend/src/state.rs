//! 应用状态

use crate::alert::manager::AlertManager;
use crate::config::Config;
use crate::db::RedisPool;
use crate::service::concurrency::{ConcurrencyConfig, ConcurrencyController};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: Arc<RedisPool>,
    pub config: Arc<Config>,
    pub alert_manager: Arc<AlertManager>,
    pub concurrency: Arc<ConcurrencyController>,
}

impl AppState {
    pub fn new(db: DatabaseConnection, redis: RedisPool, config: Config) -> Self {
        Self {
            db,
            redis: Arc::new(redis),
            config: Arc::new(config),
            alert_manager: Arc::new(AlertManager::with_defaults()),
            concurrency: Arc::new(ConcurrencyController::new(ConcurrencyConfig::default())),
        }
    }
}

/// 共享状态
pub type SharedState = Arc<AppState>;
