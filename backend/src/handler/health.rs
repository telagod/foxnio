//! 监控和健康检查

use actix_web::{web, HttpResponse};
use serde_json::json;
use std::sync::Arc;
use crate::state::AppState;

/// 健康检查端点
pub async fn health_check(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let mut checks = serde_json::Map::new();
    let mut all_healthy = true;
    
    // 检查数据库
    match state.db.health_check().await {
        Ok(true) => {
            checks.insert("database".to_string(), json!({
                "status": "healthy",
                "message": "Database connection is active"
            }));
        }
        Ok(false) | Err(_) => {
            checks.insert("database".to_string(), json!({
                "status": "unhealthy",
                "message": "Database connection failed"
            }));
            all_healthy = false;
        }
    }
    
    // 检查 Redis
    match state.redis.health_check().await {
        Ok(true) => {
            checks.insert("redis".to_string(), json!({
                "status": "healthy",
                "message": "Redis connection is active"
            }));
        }
        Ok(false) | Err(_) => {
            checks.insert("redis".to_string(), json!({
                "status": "unhealthy",
                "message": "Redis connection failed"
            }));
            all_healthy = false;
        }
    }
    
    let status = if all_healthy { "healthy" } else { "unhealthy" };
    
    HttpResponse::Ok().json(json!({
        "status": status,
        "checks": checks,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 就绪检查端点
pub async fn readiness_check(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let db_healthy = state.db.health_check().await.unwrap_or(false);
    let redis_healthy = state.redis.health_check().await.unwrap_or(false);
    
    if db_healthy && redis_healthy {
        HttpResponse::Ok().json(json!({
            "status": "ready",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(json!({
            "status": "not_ready",
            "database": db_healthy,
            "redis": redis_healthy,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

/// 存活检查端点
pub async fn liveness_check() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "alive",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 应用指标端点
pub async fn metrics(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let db_status = state.db.pool_status();
    
    HttpResponse::Ok().json(json!({
        "database": {
            "size": db_status.size,
            "num_idle": db_status.num_idle,
            "is_closed": db_status.is_closed,
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// 应用信息端点
pub async fn app_info() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "name": "FoxNIO",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "AI API Gateway",
        "rust_version": env!("CARGO_PKG_RUST_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use actix_web::{test, App};
    
    #[actix_web::test]
    async fn test_liveness_check() {
        let app = test::init_service(
            App::new().route("/live", actix_web::web::get().to(super::liveness_check))
        ).await;
        
        let req = test::TestRequest::get().uri("/live").to_request();
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
    }
}
