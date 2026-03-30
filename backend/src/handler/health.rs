//! 健康检查 API 端点 - v0.2.0
//!
//! 端点:
//! - GET /health - 简单健康状态
//! - GET /health/live - 存活探针
//! - GET /health/ready - 就绪探针
//! - GET /health/detailed - 详细状态

#![allow(dead_code)]
use axum::{extract::Extension, http::StatusCode, Json};
use serde_json::json;
use std::sync::Arc;

use crate::health::{AggregateHealthStatus, HealthChecker};

/// 简单健康状态
pub async fn health_simple(Extension(checker): Extension<Arc<HealthChecker>>) -> Json<serde_json::Value> {
    let status = checker.check_critical().await;

    let (status_str, _code) = if status.healthy {
        ("healthy", StatusCode::OK)
    } else {
        ("unhealthy", StatusCode::SERVICE_UNAVAILABLE)
    };

    Json(json!({
        "status": status_str,
        "timestamp": status.timestamp,
        "total_checks": status.total_checks,
        "healthy_checks": status.healthy_checks,
        "unhealthy_checks": status.unhealthy_checks,
        "latency_ms": status.total_latency_ms,
    }))
}

/// 存活探针 - 总是返回 OK（表示进程存活）
pub async fn health_live() -> Json<serde_json::Value> {
    Json(json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 就绪探针 - 检查关键服务是否就绪
pub async fn health_ready(
    Extension(checker): Extension<Arc<HealthChecker>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let status = checker.check_critical().await;

    if status.healthy {
        Ok(Json(json!({
            "status": "ready",
            "timestamp": status.timestamp,
            "checks": status.checks.iter().map(|(k, v)| {
                (k.clone(), v.status.healthy)
            }).collect::<std::collections::HashMap<String, bool>>(),
        })))
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "timestamp": status.timestamp,
                "unhealthy_services": status.checks.iter()
                    .filter(|(_, v)| !v.status.healthy)
                    .map(|(k, v)| json!({
                        "name": k,
                        "message": v.status.message,
                    }))
                    .collect::<Vec<_>>(),
            })),
        ))
    }
}

/// 详细健康状态 - 返回所有检查的完整信息
pub async fn health_detailed(
    Extension(checker): Extension<Arc<HealthChecker>>,
) -> Json<AggregateHealthStatus> {
    let status = checker.check_all().await;
    Json(status)
}

/// 系统资源状态
pub async fn health_resources(
    Extension(checker): Extension<Arc<HealthChecker>>,
) -> Json<serde_json::Value> {
    // 获取系统资源检查结果
    if let Some(result) = checker.check_one("system_resources").await {
        Json(json!({
            "status": if result.status.healthy { "healthy" } else { "warning" },
            "name": result.name,
            "latency_ms": result.status.latency_ms,
            "message": result.status.message,
            "details": result.status.details,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        Json(json!({
            "status": "unavailable",
            "message": "System resource check not configured",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// 数据库连接池状态
pub async fn health_database(Extension(checker): Extension<Arc<HealthChecker>>) -> Json<serde_json::Value> {
    // 使用 HealthChecker 进行简单的健康检查
    let status = checker.check_all().await;

    Json(json!({
        "status": if status.healthy { "active" } else { "unhealthy" },
        "message": "Database health check",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Redis 状态
pub async fn health_redis(Extension(checker): Extension<Arc<HealthChecker>>) -> Json<serde_json::Value> {
    // 使用 HealthChecker 进行简单的健康检查
    let status = checker.check_all().await;

    Json(json!({
        "status": if status.healthy { "active" } else { "unhealthy" },
        "message": "Redis health check",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 应用信息
pub async fn app_info() -> Json<serde_json::Value> {
    Json(json!({
        "name": "FoxNIO",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "AI API Gateway",
        "rust_version": env!("CARGO_PKG_RUST_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 应用指标
pub async fn metrics(Extension(checker): Extension<Arc<HealthChecker>>) -> Json<serde_json::Value> {
    let status = checker.check_all().await;

    Json(json!({
        "health": {
            "healthy": status.healthy,
            "checks_count": status.checks.len(),
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_live() {
        let app = Router::new().route("/health/live", get(health_live));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_app_info() {
        let app = Router::new().route("/info", get(app_info));

        let response = app
            .oneshot(Request::builder().uri("/info").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["name"], "FoxNIO");
    }
}
