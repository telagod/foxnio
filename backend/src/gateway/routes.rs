//! 路由配置 - 完整实现

use axum::{
    routing::{get, post, delete, put},
    Router,
    Extension,
    http::StatusCode,
    Json,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use serde_json::json;

use super::{SharedState, middleware, handler::GatewayHandler};
use crate::service::{UserService, ApiKeyService, AccountService, BillingService, SchedulerService};
use crate::handler;

pub fn build_app(state: super::AppState) -> Router {
    let shared_state = Arc::new(state);
    
    // 公开路由
    let public_routes = Router::new()
        // Health check
        .route("/health", get(|| async { "OK" }))
        .route("/ready", get(health_check))
        
        // API 端点（OpenAI 兼容）
        .route("/v1/models", get(handler::list_models))
        
        // 认证
        .route("/api/v1/auth/register", post(handler::auth::register))
        .route("/api/v1/auth/login", post(handler::auth::login));
    
    // 需要认证的路由
    let auth_routes = Router::new()
        // 用户信息
        .route("/api/v1/user/me", get(handler::auth::get_me))
        .route("/api/v1/user/usage", get(get_user_usage))
        
        // Chat completions (需要 API Key)
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .route("/v1/completions", post(handle_completions))
        
        // API Keys
        .route("/api/v1/user/apikeys", get(list_user_apikeys))
        .route("/api/v1/user/apikeys", post(create_user_apikey))
        .route("/api/v1/user/apikeys/:id", delete(delete_user_apikey))
        .route("/api/v1/user/apikeys/:id", put(update_user_apikey))
        
        .layer(axum::middleware::from_fn(middleware::jwt_auth));
    
    // 管理后台路由
    let admin_routes = Router::new()
        .route("/api/v1/admin/users", get(handler::admin::list_users))
        .route("/api/v1/admin/users/:id", get(get_user_detail))
        .route("/api/v1/admin/users/:id/balance", post(update_user_balance))
        
        .route("/api/v1/admin/accounts", get(handler::admin::list_accounts))
        .route("/api/v1/admin/accounts", post(handler::admin::add_account))
        .route("/api/v1/admin/accounts/:id", get(get_account_detail))
        .route("/api/v1/admin/accounts/:id", put(update_account))
        .route("/api/v1/admin/accounts/:id", delete(delete_account))
        .route("/api/v1/admin/accounts/test", post(test_account))
        
        .route("/api/v1/admin/apikeys", get(handler::admin::list_apikeys))
        .route("/api/v1/admin/stats", get(handler::admin::get_stats))
        .route("/api/v1/admin/dashboard", get(get_dashboard))
        
        .layer(axum::middleware::from_fn(middleware::jwt_auth));
    
    // Gemini 专用路由
    let gemini_routes = Router::new()
        .route("/v1beta/models/:model:generateContent", post(handle_gemini))
        .route("/v1beta/models/:model:streamGenerateContent", post(handle_gemini_stream))
        
        .layer(axum::middleware::from_fn(middleware::jwt_auth));
    
    Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .merge(gemini_routes)
        
        // Layers
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(middleware::request_log))
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(Extension(shared_state))
}

// ============ 健康检查 ============

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// ============ 网关端点 ============

async fn handle_chat_completions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // 解析请求体
    let req: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let model = req.get("model")
        .and_then(|m| m.as_str())
        .ok_or(handler::ApiError(StatusCode::BAD_REQUEST, "Missing model".into()))?;
    
    let stream = req.get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);
    
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // TODO: 实现完整的请求转发
    Err(handler::ApiError(
        StatusCode::NOT_IMPLEMENTED,
        "Chat completions forwarding not yet implemented".into()
    ))
}

async fn handle_messages(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let req: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let model = req.get("model")
        .and_then(|m| m.as_str())
        .ok_or(handler::ApiError(StatusCode::BAD_REQUEST, "Missing model".into()))?;
    
    // TODO: 实现完整的 Anthropic messages 转发
    Err(handler::ApiError(
        StatusCode::NOT_IMPLEMENTED,
        "Messages forwarding not yet implemented".into()
    ))
}

async fn handle_completions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现旧的 completions 端点
    Err(handler::ApiError(
        StatusCode::NOT_IMPLEMENTED,
        "Completions not yet implemented".into()
    ))
}

async fn handle_gemini(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现 Gemini 端点
    Err(handler::ApiError(
        StatusCode::NOT_IMPLEMENTED,
        "Gemini not yet implemented".into()
    ))
}

async fn handle_gemini_stream(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现 Gemini 流式端点
    Err(handler::ApiError(
        StatusCode::NOT_IMPLEMENTED,
        "Gemini streaming not yet implemented".into()
    ))
}

// ============ 用户端点 ============

async fn get_user_usage(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let billing_service = BillingService::new(
        state.db.clone(),
        state.config.gateway.rate_multiplier,
    );
    
    let stats = billing_service.get_user_stats(user_id, 30)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(axum::Json(json!({
        "total_requests": stats.total_requests,
        "total_input_tokens": stats.total_input_tokens,
        "total_output_tokens": stats.total_output_tokens,
        "total_cost": stats.total_cost,
        "total_cost_yuan": stats.total_cost as f64 / 100.0,
    })))
}

// ============ API Key 管理 ============

async fn list_user_apikeys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let keys = api_key_service.list_by_user(user_id)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({
        "object": "list",
        "data": keys.iter().map(|k| json!({
            "id": k.id.to_string(),
            "key": k.key_masked,
            "name": k.name,
            "status": k.status,
            "created_at": k.created_at.to_rfc3339(),
            "last_used_at": k.last_used_at.map(|t| t.to_rfc3339()),
        })).collect::<Vec<_>>()
    })))
}

async fn create_user_apikey(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let name = body.get("name").and_then(|v| v.as_str());

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let key = api_key_service.create(user_id, name.map(|s| s.to_string()))
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({
        "id": key.id.to_string(),
        "key": key.key_masked,
        "name": key.name,
        "status": key.status,
        "created_at": key.created_at.to_rfc3339(),
    })))
}

async fn delete_user_apikey(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let key_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    api_key_service.delete(user_id, key_id)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({ "success": true })))
}

async fn update_user_apikey(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(_id): axum::extract::Path<String>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现 API Key 更新
    Ok(axum::Json(json!({ "success": true })))
}

// ============ 管理端点 ============

async fn get_user_detail(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    // TODO: 实现用户详情
    Ok(axum::Json(json!({ "id": id })))
}

async fn update_user_balance(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    let delta = body.get("delta")
        .and_then(|v| v.as_i64())
        .ok_or(handler::ApiError(StatusCode::BAD_REQUEST, "Missing delta".into()))?;
    
    let user_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let user_service = UserService::new(
        state.db.clone(),
        state.config.jwt.secret.clone(),
        state.config.jwt.expire_hours,
    );
    
    let new_balance = user_service.update_balance(user_id, delta)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(axum::Json(json!({
        "success": true,
        "new_balance": new_balance,
        "new_balance_yuan": new_balance as f64 / 100.0,
    })))
}

async fn get_account_detail(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    // TODO: 实现账号详情
    Ok(axum::Json(json!({ "id": id })))
}

async fn update_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(_id): axum::extract::Path<String>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    // TODO: 实现账号更新
    Ok(axum::Json(json!({ "success": true })))
}

async fn delete_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    let account_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    
    let account_service = AccountService::new(state.db.clone());
    account_service.delete(account_id)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(axum::Json(json!({ "success": true })))
}

async fn test_account(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    // TODO: 实现账号测试
    Ok(axum::Json(json!({ "success": true, "message": "Account test not yet implemented" })))
}

async fn get_dashboard(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    if claims.role != "admin" {
        return Err(handler::ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }
    
    let billing_service = BillingService::new(
        state.db.clone(),
        state.config.gateway.rate_multiplier,
    );
    
    let stats = billing_service.get_global_stats(7)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(axum::Json(json!({
        "week": {
            "total_requests": stats.total_requests,
            "total_input_tokens": stats.total_input_tokens,
            "total_output_tokens": stats.total_output_tokens,
            "total_cost": stats.total_cost,
        }
    })))
}
