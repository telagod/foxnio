//! 健康检查 API 端点 - v0.2.0
//!
//! 端点:
//! - GET /health - 简单健康状态
//! - GET /health/live - 存活探针
//! - GET /health/ready - 就绪探针
//! - GET /health/detailed - 详细状态

use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use std::sync::Arc;

use crate::health::{AggregateHealthStatus, HealthChecker};
use crate::gateway::SharedState;

/// 简单健康状态
pub async fn health_simple(State(checker): State<Arc<HealthChecker>>) -> Json<serde_json::Value> {
    let status = checker.check_critical().await;

    let (status_str, code) = if status.healthy {
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
    State(checker): State<Arc<HealthChecker>>,
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
    State(checker): State<Arc<HealthChecker>>,
) -> Json<AggregateHealthStatus> {
    let status = checker.check_all().await;
    Json(status)
}

/// 系统资源状态
pub async fn health_resources(
    State(checker): State<Arc<HealthChecker>>,
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
pub async fn health_database(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let pool_status = state.db.pool_status();

    Json(json!({
        "status": if pool_status.is_closed { "closed" } else { "active" },
        "pool_size": pool_status.size,
        "idle_connections": pool_status.num_idle,
        "reuse_rate": format!("{:.1}%", pool_status.reuse_rate * 100.0),
        "total_requests": pool_status.total_requests,
        "leak_warnings": pool_status.leak_warnings,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Redis 状态
pub async fn health_redis(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let stats = state.redis.get_stats();

    Json(json!({
        "status": "active",
        "cache_hit_rate": format!("{:.1}%", stats.cache_hit_rate() * 100.0),
        "avg_latency_ms": format!("{:.1}", stats.avg_latency_ms()),
        "total_requests": stats.total_requests.load(std::sync::atomic::Ordering::Relaxed),
        "cache_hits": stats.cache_hits.load(std::sync::atomic::Ordering::Relaxed),
        "cache_misses": stats.cache_misses.load(std::sync::atomic::Ordering::Relaxed),
        "errors": stats.errors.load(std::sync::atomic::Ordering::Relaxed),
        "retries": stats.retries.load(std::sync::atomic::Ordering::Relaxed),
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
pub async fn metrics(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let db_status = state.db.pool_status();
    let redis_stats = state.redis.get_stats();

    Json(json!({
        "database": {
            "pool_size": db_status.size,
            "idle_connections": db_status.num_idle,
            "is_closed": db_status.is_closed,
            "reuse_rate": db_status.reuse_rate,
            "total_requests": db_status.total_requests,
            "leak_warnings": db_status.leak_warnings,
        },
        "redis": {
            "total_requests": redis_stats.total_requests.load(std::sync::atomic::Ordering::Relaxed),
            "cache_hit_rate": redis_stats.cache_hit_rate(),
            "avg_latency_ms": redis_stats.avg_latency_ms(),
            "errors": redis_stats.errors.load(std::sync::atomic::Ordering::Relaxed),
            "retries": redis_stats.retries.load(std::sync::atomic::Ordering::Relaxed),
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
