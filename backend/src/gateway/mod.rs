//! 网关核心模块

mod test;

pub mod proxy;
pub mod stream;
pub mod middleware;
pub mod routes;
pub mod handler;
pub mod failover;
pub mod claude;

pub use proxy::*;
pub use stream::*;
pub use middleware::*;
pub use routes::build_app;
pub use handler::GatewayHandler;
pub use failover::FailoverManager;
pub use claude::{ClaudeHeaders, TLSFingerprint, get_beta_header};

use sea_orm::DatabaseConnection;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use crate::config::Config;

pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: ConnectionManager,
    pub config: Config,
}

pub type SharedState = Arc<AppState>;
