//! 网关核心模块

mod test;

pub mod claude;
pub mod failover;
pub mod gemini;
pub mod handler;
pub mod middleware;
pub mod models;
pub mod proxy;
pub mod request_rectifier;
pub mod responses;
pub mod responses_converter;
pub mod responses_handler;
pub mod routes;
pub mod scheduler;
pub mod sora;
pub mod stream;
pub mod waiting_queue;
pub mod websocket;

pub use failover::FailoverManager;
#[cfg(test)]
pub use proxy::UpstreamEndpoints;
pub use routes::build_app;
#[cfg(test)]
pub use stream::SseEvent;

// 重导出 Gemini 模块
pub use gemini::{GeminiClient, GeminiClientConfig, GeminiHandler};

// 重导出统一的 AppState
pub use crate::state::SharedState;
pub mod claude_shell;
