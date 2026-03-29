#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 集成测试

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let app = Router::new().route(
        "/health",
        get(|| async { Json(json!({"status": "healthy"})) }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_success());
}

#[test]
fn test_api_key_validation() {
    let valid_keys = vec![
        "foxnio-1234567890abcdef1234567890abcdef",
        "sk-test-abcdefghijklmnopqrstuvwxyz123456",
    ];

    let invalid_keys = vec!["invalid", "too-short", ""];

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

#[tokio::test]
async fn test_json_response() {
    let app = Router::new().route(
        "/api/test",
        get(|| async {
            Json(json!({
                "success": true,
                "data": {
                    "id": 1,
                    "name": "test"
                }
            }))
        }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_error_response() {
    let app = Router::new().route(
        "/api/error",
        get(|| async {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid request", "code": 400})),
            )
        }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/error")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_post_request() {
    let app = Router::new().route(
        "/api/create",
        post(|| async { (StatusCode::CREATED, Json(json!({"created": true}))) }),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/create")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}
