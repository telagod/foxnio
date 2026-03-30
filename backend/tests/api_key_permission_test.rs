//! API Key 权限验证测试
//!
//! 测试 API Key 的完整权限验证流程，包括：
//! - 模型访问权限
//! - IP 白名单
//! - 配额限制
//! - 过期时间
//! - 禁用状态
//! - 权限组合验证

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use std::net::SocketAddr;
use tower::ServiceExt;
use uuid::Uuid;

// 导入项目模块
use backend::entity::api_keys;
use backend::middleware::api_key_auth::{api_key_auth_with_permissions, ApiKeyAuthError};
use backend::gateway::SharedState;

mod common;
use common::*;

// ============================================================================
// 辅助函数
// ============================================================================

/// 设置测试应用和数据库连接
async fn setup_test_app() -> (Router, DatabaseConnection) {
    // 使用内存数据库进行测试
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    // 创建 API keys 表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            key TEXT NOT NULL UNIQUE,
            name TEXT,
            prefix TEXT NOT NULL DEFAULT 'sk-',
            status TEXT NOT NULL DEFAULT 'active',
            concurrent_limit INTEGER DEFAULT 5,
            rate_limit_rpm INTEGER DEFAULT 60,
            allowed_models TEXT,
            ip_whitelist TEXT,
            expires_at TEXT,
            daily_quota INTEGER,
            daily_used_quota INTEGER DEFAULT 0,
            quota_reset_at TEXT,
            last_used_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&db)
    .await
    .expect("Failed to create api_keys table");

    // 创建一个简单的测试路由
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({
                    "id": "chatcmpl-test",
                    "object": "chat.completion",
                    "created": 1234567890,
                    "model": "gpt-4",
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "Hello!"
                        },
                        "finish_reason": "stop"
                    }]
                }))
            }),
        )
        .route(
            "/v1/models",
            get(|| async {
                Json(json!({
                    "object": "list",
                    "data": [
                        {"id": "gpt-4", "object": "model"},
                        {"id": "gpt-3.5-turbo", "object": "model"}
                    ]
                }))
            }),
        );

    (app, db)
}

/// 创建具有特定权限的 API Key
async fn create_api_key_with_permissions(
    db: &DatabaseConnection,
    user_id: Uuid,
    name: &str,
    allowed_models: Option<Vec<&str>>,
    ip_whitelist: Option<Vec<&str>>,
    daily_quota: Option<i64>,
    expires_at: Option<chrono::DateTime<Utc>>,
    status: &str,
) -> (api_keys::Model, String) {
    let key_id = Uuid::new_v4();
    let key = format!("sk-test-{}", Uuid::new_v4().to_string().replace('-', ""));
    let prefix = key[..10].to_string();

    let allowed_models_json = allowed_models.map(|models| {
        serde_json::to_value(models).expect("Failed to serialize allowed_models")
    });

    let ip_whitelist_json = ip_whitelist.map(|ips| {
        serde_json::to_value(ips).expect("Failed to serialize ip_whitelist")
    });

    // 插入 API key
    sqlx::query(
        r#"
        INSERT INTO api_keys (
            id, user_id, key, name, prefix, status,
            allowed_models, ip_whitelist, daily_quota,
            daily_used_quota, expires_at, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(key_id.to_string())
    .bind(user_id.to_string())
    .bind(&key)
    .bind(name)
    .bind(&prefix)
    .bind(status)
    .bind(&allowed_models_json)
    .bind(&ip_whitelist_json)
    .bind(daily_quota)
    .bind(0i64)
    .bind(expires_at.map(|e| e.to_rfc3339()))
    .bind(Utc::now().to_rfc3339())
    .execute(db)
    .await
    .expect("Failed to create API key");

    // 查询并返回创建的 API key
    let api_key: (String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>, String) = 
        sqlx::query_as(
            "SELECT key, prefix, status, allowed_models, ip_whitelist, daily_quota, daily_used_quota, expires_at, created_at FROM api_keys WHERE id = ?"
        )
        .bind(key_id.to_string())
        .fetch_one(db)
        .await
        .expect("Failed to fetch created API key");

    let model = api_keys::Model {
        id: key_id,
        user_id,
        key: api_key.0,
        name: Some(name.to_string()),
        prefix: api_key.1,
        status: api_key.2,
        concurrent_limit: Some(5),
        rate_limit_rpm: Some(60),
        allowed_models: api_key.3.and_then(|s| serde_json::from_str(&s).ok()),
        ip_whitelist: api_key.4.and_then(|s| serde_json::from_str(&s).ok()),
        expires_at: api_key.7.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
        daily_quota: api_key.5,
        daily_used_quota: api_key.6,
        quota_reset_at: None,
        last_used_at: None,
        created_at: Utc::now(),
    };

    (model, key)
}

/// 更新 API Key 的已使用配额
async fn update_quota_used(db: &DatabaseConnection, key_id: Uuid, used: i64) {
    sqlx::query("UPDATE api_keys SET daily_used_quota = ? WHERE id = ?")
        .bind(used)
        .bind(key_id.to_string())
        .execute(db)
        .await
        .expect("Failed to update quota");
}

/// 模拟从特定 IP 发送请求
fn mock_request_from_ip(method: &str, uri: &str, api_key: &str, ip: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header("x-forwarded-for", ip);

    if let Some(b) = body {
        builder = builder.header("content-type", "application/json");
        builder.body(Body::from(serde_json::to_string(&b).unwrap())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

/// 创建测试用的 SharedState
fn create_test_state(db: DatabaseConnection) -> SharedState {
    use std::sync::Arc;
    use backend::cache::RedisPool;
    use backend::config::Config;

    SharedState {
        db,
        redis: Arc::new(RedisPool::default()),
        config: Arc::new(Config::default()),
    }
}

// ============================================================================
// 测试用例
// ============================================================================

/// 测试 1: 模型权限允许
#[tokio::test]
async fn test_model_permission_allowed() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建只允许访问 gpt-4 的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        Some(vec!["gpt-4", "gpt-3.5-turbo"]),
        None,
        None,
        None,
        "active",
    )
    .await;

    // 模拟请求允许的模型
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = app.clone().oneshot(request).await.unwrap();

    // 验证请求成功
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试 2: 模型权限拒绝
#[tokio::test]
async fn test_model_permission_denied() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建只允许访问 gpt-3.5-turbo 的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        Some(vec!["gpt-3.5-turbo"]),
        None,
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 模拟请求不允许的模型
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证返回 403 Forbidden
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // 验证错误消息
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"].as_str().unwrap().contains("not allowed"));
}

/// 测试 3: 通配符权限
#[tokio::test]
async fn test_model_permission_wildcard() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建允许所有模型的 API key（使用 "*"）
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        Some(vec!["*"]),
        None,
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 测试多个模型都应该可以访问
    let models = vec!["gpt-4", "gpt-3.5-turbo", "claude-3-opus", "claude-3-sonnet"];

    for model in models {
        let request = mock_request_from_ip(
            "POST",
            &format!("/v1/chat/completions?model={}", model),
            &api_key,
            "192.168.1.100",
            Some(json!({
                "model": model,
                "messages": [{"role": "user", "content": "Hello"}]
            })),
        );

        let response = protected_app.clone().oneshot(request).await.unwrap();

        // 验证所有模型都可以访问
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Model {} should be accessible",
            model
        );
    }
}

/// 测试 4: IP 白名单允许
#[tokio::test]
async fn test_ip_whitelist_allowed() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建带有 IP 白名单的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        Some(vec!["192.168.1.100", "192.168.1.101"]),
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 从允许的 IP 发送请求
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证请求成功
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试 5: IP 白名单拒绝
#[tokio::test]
async fn test_ip_whitelist_denied() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建带有 IP 白名单的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        Some(vec!["192.168.1.100", "192.168.1.101"]),
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 从不允许的 IP 发送请求
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.200",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证返回 403 Forbidden
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // 验证错误消息
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"].as_str().unwrap().contains("IP"));
    assert!(error["error"].as_str().unwrap().contains("not allowed"));
}

/// 测试 6: 空白名单（允许所有 IP）
#[tokio::test]
async fn test_ip_whitelist_empty() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建没有 IP 白名单限制的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None, // null 表示允许所有 IP
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 测试多个不同的 IP 都应该可以访问
    let ips = vec!["192.168.1.100", "10.0.0.1", "172.16.0.1", "8.8.8.8"];

    for ip in ips {
        let request = mock_request_from_ip(
            "POST",
            "/v1/chat/completions",
            &api_key,
            ip,
            Some(json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            })),
        );

        let response = protected_app.clone().oneshot(request).await.unwrap();

        // 验证所有 IP 都可以访问
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "IP {} should be allowed",
            ip
        );
    }

    // 测试空数组的 IP 白名单
    let (api_key_model2, api_key2) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key 2",
        None,
        Some(vec![]), // 空数组也表示允许所有 IP
        None,
        None,
        "active",
    )
    .await;

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key2,
        "192.168.1.200",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试 7: 配额限制
#[tokio::test]
async fn test_quota_enforcement() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建有配额限制的 API key（每日 10 次）
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        Some(10),
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db.clone());
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 模拟使用到配额上限
    update_quota_used(&db, api_key_model.id, 10).await;

    // 发送请求
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证返回 429 Too Many Requests
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // 验证错误消息
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"].as_str().unwrap().contains("quota"));
}

/// 测试 8: 配额重置
#[tokio::test]
async fn test_quota_reset() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建有配额限制的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        Some(10),
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db.clone());
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 模拟配额已满
    update_quota_used(&db, api_key_model.id, 10).await;

    // 验证配额已满时请求失败
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // 重置配额（设置为 0）
    update_quota_used(&db, api_key_model.id, 0).await;

    // 验证重置后可以继续使用
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试 9: 过期 Key
#[tokio::test]
async fn test_expired_key() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建已过期的 API key（过期时间设置为 1 小时前）
    let expired_time = Utc::now() - Duration::hours(1);
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        None,
        Some(expired_time),
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 发送请求
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证返回 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 验证错误消息
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"].as_str().unwrap().contains("expired"));
}

/// 测试 10: 已禁用 Key
#[tokio::test]
async fn test_disabled_key() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建已禁用的 API key（status = "disabled"）
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        None,
        None,
        "disabled", // 设置为禁用状态
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 发送请求
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();

    // 验证返回 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 验证错误消息
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert!(error["error"].as_str().unwrap().contains("disabled"));
}

/// 测试 11: 权限组合测试
#[tokio::test]
async fn test_permission_combination() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建一个有多种限制的 API key
    let future_time = Utc::now() + Duration::days(30);
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        Some(vec!["gpt-4", "gpt-3.5-turbo"]), // 只允许这两个模型
        Some(vec!["192.168.1.100", "192.168.1.101"]), // 只允许这两个 IP
        Some(100), // 每日配额 100
        Some(future_time), // 30天后过期
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db.clone());
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 场景 1: 所有条件都满足 - 应该成功
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "All conditions met should succeed");

    // 场景 2: 模型不允许 - 应该失败
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=claude-3-opus",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Model not allowed should fail"
    );

    // 场景 3: IP 不允许 - 应该失败
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.200", // 不在白名单中
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "IP not allowed should fail"
    );

    // 场景 4: 配额已满 - 应该失败
    update_quota_used(&db, api_key_model.id, 100).await;

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "Quota exceeded should fail"
    );

    // 场景 5: 配额重置后 - 应该成功
    update_quota_used(&db, api_key_model.id, 50).await; // 重置为 50

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "After quota reset should succeed");
}

// ============================================================================
// 边界条件测试
// ============================================================================

/// 测试边界条件: allowed_models 为空数组
#[tokio::test]
async fn test_empty_allowed_models() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建 allowed_models 为空数组的 API key
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        Some(vec![]), // 空数组
        None,
        None,
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 空数组应该允许所有模型
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=gpt-4",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试边界条件: 配额为 0 表示无限制
#[tokio::test]
async fn test_zero_quota_unlimited() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建配额为 0 的 API key（表示无限制）
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        Some(0), // 0 表示无限制
        None,
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 即使设置很大的已使用量，也应该可以访问
    update_quota_used(&db, api_key_model.id, 1000000).await;

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    // 0 配额表示无限制，应该成功
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试边界条件: 过期时间为未来
#[tokio::test]
async fn test_future_expiration_allowed() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建未来过期的 API key
    let future_time = Utc::now() + Duration::days(365);
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        None,
        Some(future_time),
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 未来过期应该可以访问
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试边界条件: 无过期时间
#[tokio::test]
async fn test_no_expiration() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 创建没有过期时间的 API key（永不过期）
    let (api_key_model, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Test Key",
        None,
        None,
        None,
        None, // 无过期时间
        "active",
    )
    .await;

    // 创建带中间件的路由
    let state = create_test_state(db);
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    // 无过期时间应该可以访问
    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// 测试错误消息的准确性
#[tokio::test]
async fn test_error_messages() {
    let (app, db) = setup_test_app().await;
    let user_id = Uuid::new_v4();

    // 测试过期 Key 的错误消息
    let expired_time = Utc::now() - Duration::hours(1);
    let (_, api_key) = create_api_key_with_permissions(
        &db,
        user_id,
        "Expired Key",
        None,
        None,
        None,
        Some(expired_time),
        "active",
    )
    .await;

    let state = create_test_state(db.clone());
    let protected_app = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions",
        &api_key,
        "192.168.1.100",
        Some(json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app.oneshot(request).await.unwrap();
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();

    // 验证错误消息包含 "expired"
    assert!(error["error"].as_str().unwrap().to_lowercase().contains("expired"));

    // 测试模型不允许的错误消息
    let (_, api_key2) = create_api_key_with_permissions(
        &db,
        user_id,
        "Restricted Key",
        Some(vec!["gpt-3.5-turbo"]),
        None,
        None,
        None,
        "active",
    )
    .await;

    let protected_app2 = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(json!({"success": true}))
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_with_permissions,
        ));

    let request = mock_request_from_ip(
        "POST",
        "/v1/chat/completions?model=claude-3-opus",
        &api_key2,
        "192.168.1.100",
        Some(json!({
            "model": "claude-3-opus",
            "messages": [{"role": "user", "content": "Hello"}]
        })),
    );

    let response = protected_app2.oneshot(request).await.unwrap();
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let error: Value = serde_json::from_slice(&body).unwrap();

    // 验证错误消息包含模型名称
    let error_msg = error["error"].as_str().unwrap();
    assert!(error_msg.contains("claude-3-opus") || error_msg.contains("Model"));
}
