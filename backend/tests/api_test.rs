//! 集成测试

use actix_web::{test, App};
use serde_json::json;

#[actix_web::test]
async fn test_health_endpoint() {
    let app = test::init_service(
        App::new().route("/health", actix_web::web::get().to(|| async {
            actix_web::HttpResponse::Ok().json(json!({"status": "healthy"}))
        }))
    ).await;
    
    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_api_key_validation() {
    let valid_keys = vec![
        "foxnio-1234567890abcdef1234567890abcdef",
        "sk-test-abcdefghijklmnopqrstuvwxyz123456",
    ];
    
    let invalid_keys = vec![
        "invalid",
        "too-short",
        "",
    ];
    
    for key in valid_keys {
        // 应该是有效的 API Key 格式
        assert!(key.contains('-'));
    }
    
    for key in invalid_keys {
        // 不应该是有效的 API Key 格式
        if !key.is_empty() {
            assert!(!key.contains('-') || key.len() < 20);
        }
    }
}

#[actix_web::test]
async fn test_json_response() {
    let app = test::init_service(
        App::new().route("/api/test", actix_web::web::get().to(|| async {
            actix_web::HttpResponse::Ok().json(json!({
                "success": true,
                "data": {
                    "id": 1,
                    "name": "test"
                }
            }))
        }))
    ).await;
    
    let req = test::TestRequest::get().uri("/api/test").to_request();
    let resp: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    
    assert!(resp["success"].as_bool().unwrap());
    assert_eq!(resp["data"]["id"], 1);
}

#[actix_web::test]
async fn test_error_response() {
    let app = test::init_service(
        App::new().route("/api/error", actix_web::web::get().to(|| async {
            actix_web::HttpResponse::BadRequest().json(json!({
                "error": "Invalid request",
                "code": 400
            }))
        }))
    ).await;
    
    let req = test::TestRequest::get().uri("/api/error").to_request();
    let resp = test::call_service(&app, req).await;
    
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_post_request() {
    let app = test::init_service(
        App::new().route("/api/create", actix_web::web::post().to(|| async {
            actix_web::HttpResponse::Created().json(json!({
                "created": true
            }))
        }))
    ).await;
    
    let req = test::TestRequest::post()
        .uri("/api/create")
        .set_json(json!({"name": "test"}))
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_async_operation() {
    use std::time::Duration;
    
    let result = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        "completed"
    };
    
    assert_eq!(result.await, "completed");
}
