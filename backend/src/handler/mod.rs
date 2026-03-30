//! HTTP 处理器模块

pub mod admin;
pub mod admin_accounts;
pub mod admin_groups;
pub mod alerts;
pub mod announcement;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod batch;
pub mod dashboard;
pub mod error_passthrough_rule;
pub mod groups;
pub mod health;
pub mod metrics;
pub mod models;
pub mod promo_code;
pub mod proxy;
pub mod quota;
pub mod redeem;
pub mod scheduled_test_plan;
pub mod subscription;
pub mod user;
pub mod user_announcement;
pub mod user_attribute;
pub mod user_groups;
pub mod verify;
pub mod webhook;

use axum::Json;
use serde_json::json;

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

pub use health::{
    app_info, health_database, health_detailed, health_live, health_ready, health_redis,
    health_resources, health_simple,
};

pub use audit::{
    cleanup_audit_logs, get_audit_stats, list_audit_logs, list_my_audit_logs,
    list_sensitive_audit_logs, list_user_audit_logs,
};

/// 列出可用模型 (OpenAI 兼容 API) - 使用动态模型列表
pub async fn list_models() -> Result<Json<serde_json::Value>, ApiError> {
    // 使用静态模型列表作为回退
    use crate::gateway::models::{get_model_info, Model};

    let models: Vec<serde_json::Value> = Model::all()
        .into_iter()
        .filter_map(|m| {
            let info = get_model_info(m)?;
            Some(json!({
                "id": info.id,
                "object": "model",
                "created": 1700000000,
                "owned_by": info.provider.to_lowercase(),
                "permission": [{
                    "id": format!("modelperm-{}", info.id),
                    "object": "model_permission",
                    "created": 1700000000,
                    "allow_create_engine": false,
                    "allow_sampling": true,
                    "allow_logprobs": true,
                    "allow_search_indices": false,
                    "allow_view": true,
                    "allow_fine_tuning": false,
                    "organization": "*",
                    "group": null,
                    "is_blocking": false
                }],
                "root": info.id,
                "parent": null,
            }))
        })
        .collect();

    Ok(Json(json!({
        "object": "list",
        "data": models
    })))
}
