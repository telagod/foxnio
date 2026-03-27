//! HTTP 处理器模块

pub mod auth;
pub mod admin;
pub mod health;
pub mod audit;

// ApiError 定义
#[derive(Debug)]
pub struct ApiError(pub axum::http::StatusCode, pub String);

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({
            "error": self.1,
        });
        (self.0, axum::Json(body)).into_response()
    }
}

// 重新导出 auth 子模块
pub use auth::{register, login, get_me, refresh, logout, logout_all};

pub use health::{
    health_simple,
    health_live,
    health_ready,
    health_detailed,
    health_resources,
    health_database,
    health_redis,
    app_info,
    metrics,
};

pub use audit::{
    list_audit_logs,
    list_user_audit_logs,
    list_sensitive_audit_logs,
    list_my_audit_logs,
    cleanup_audit_logs,
    get_audit_stats,
};
