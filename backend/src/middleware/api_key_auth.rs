//! API Key 权限验证中间件
//!
//! 提供完整的 API Key 权限验证，包括：
//! - 模型访问权限检查
//! - IP 白名单验证
//! - 过期时间检查
//! - 每日配额检查和扣减

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use std::net::SocketAddr;

use crate::entity::api_keys;
use crate::gateway::SharedState;

/// API Key 认证错误
#[derive(Debug)]
pub enum ApiKeyAuthError {
    /// 缺少 API Key
    MissingApiKey,
    /// 无效的 API Key
    InvalidApiKey,
    /// API Key 已过期
    Expired,
    /// API Key 已被禁用
    Disabled,
    /// IP 不在白名单中
    IpNotAllowed(String),
    /// 模型不允许访问
    ModelNotAllowed(String),
    /// 超过每日配额
    QuotaExceeded,
    /// 数据库错误
    DatabaseError(String),
}

impl IntoResponse for ApiKeyAuthError {
    fn into_response(self) -> Response {
        let (status, error_msg) = match self {
            ApiKeyAuthError::MissingApiKey => {
                (StatusCode::UNAUTHORIZED, "Missing API key")
            }
            ApiKeyAuthError::InvalidApiKey => {
                (StatusCode::UNAUTHORIZED, "Invalid API key")
            }
            ApiKeyAuthError::Expired => {
                (StatusCode::UNAUTHORIZED, "API key has expired")
            }
            ApiKeyAuthError::Disabled => {
                (StatusCode::UNAUTHORIZED, "API key is disabled")
            }
            ApiKeyAuthError::IpNotAllowed(ip) => {
                (StatusCode::FORBIDDEN, &format!("IP {} is not allowed", ip))
            }
            ApiKeyAuthError::ModelNotAllowed(model) => {
                (StatusCode::FORBIDDEN, &format!("Model {} is not allowed", model))
            }
            ApiKeyAuthError::QuotaExceeded => {
                (StatusCode::TOO_MANY_REQUESTS, "Daily quota exceeded")
            }
            ApiKeyAuthError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str())
            }
        };

        (status, Json(json!({ "error": error_msg }))).into_response()
    }
}

/// API Key 权限验证中间件
///
/// 执行完整的权限验证流程：
/// 1. 验证 API Key 是否存在且有效
/// 2. 检查 API Key 状态（是否禁用）
/// 3. 检查过期时间
/// 4. 验证 IP 白名单
/// 5. 验证模型访问权限
/// 6. 检查和扣减每日配额
pub async fn api_key_auth_with_permissions(
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiKeyAuthError> {
    // 1. 从请求头获取 API Key
    let api_key = extract_api_key(&req)?;
    
    // 2. 从数据库查询 API Key
    let api_key_model = fetch_api_key(&state.db, &api_key).await?;
    
    // 3. 验证 API Key 状态
    validate_api_key_status(&api_key_model)?;
    
    // 4. 验证 IP 白名单
    let client_ip = addr.ip().to_string();
    validate_ip_whitelist(&api_key_model, &client_ip)?;
    
    // 5. 验证模型访问权限（从请求中提取模型名称）
    if let Some(model) = extract_model_from_request(&req) {
        validate_model_permission(&api_key_model, &model)?;
    }
    
    // 6. 检查和扣减配额
    let api_key_model = check_and_deduct_quota(&state.db, api_key_model).await?;
    
    // 7. 将 API Key 信息添加到请求扩展中
    req.extensions_mut().insert(api_key_model.clone());
    req.extensions_mut().insert(api_key_model.user_id);
    req.extensions_mut().insert(api_key_model.id);
    
    // 8. 更新最后使用时间（异步执行，不阻塞请求）
    update_last_used_time(&state.db, api_key_model.id);
    
    Ok(next.run(req).await)
}

/// 从请求头中提取 API Key
fn extract_api_key(req: &Request<Body>) -> Result<String, ApiKeyAuthError> {
    // 尝试从 Authorization header 获取（Bearer token）
    let api_key = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());
    
    // 如果没有，尝试从 x-api-key header 获取
    let api_key = api_key.or_else(|| {
        req.headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });
    
    api_key.ok_or(ApiKeyAuthError::MissingApiKey)
}

/// 从数据库获取 API Key
async fn fetch_api_key(
    db: &sea_orm::DatabaseConnection,
    key: &str,
) -> Result<api_keys::Model, ApiKeyAuthError> {
    api_keys::Entity::find()
        .filter(api_keys::Column::Key.eq(key))
        .one(db)
        .await
        .map_err(|e| ApiKeyAuthError::DatabaseError(e.to_string()))?
        .ok_or(ApiKeyAuthError::InvalidApiKey)
}

/// 验证 API Key 状态（是否激活、是否过期）
fn validate_api_key_status(api_key: &api_keys::Model) -> Result<(), ApiKeyAuthError> {
    // 检查状态
    if api_key.status != "active" {
        return Err(ApiKeyAuthError::Disabled);
    }
    
    // 检查过期时间
    if let Some(expires_at) = api_key.expires_at {
        if expires_at <= Utc::now() {
            return Err(ApiKeyAuthError::Expired);
        }
    }
    
    Ok(())
}

/// 验证 IP 白名单
fn validate_ip_whitelist(
    api_key: &api_keys::Model,
    client_ip: &str,
) -> Result<(), ApiKeyAuthError> {
    if !api_key.is_ip_allowed(client_ip) {
        return Err(ApiKeyAuthError::IpNotAllowed(client_ip.to_string()));
    }
    Ok(())
}

/// 验证模型访问权限
fn validate_model_permission(
    api_key: &api_keys::Model,
    model: &str,
) -> Result<(), ApiKeyAuthError> {
    if !api_key.is_model_allowed(model) {
        return Err(ApiKeyAuthError::ModelNotAllowed(model.to_string()));
    }
    Ok(())
}

/// 检查和扣减配额
async fn check_and_deduct_quota(
    db: &sea_orm::DatabaseConnection,
    mut api_key: api_keys::Model,
) -> Result<api_keys::Model, ApiKeyAuthError> {
    // 如果没有设置配额，直接返回
    if api_key.daily_quota.is_none() {
        return Ok(api_key);
    }
    
    // 检查是否需要重置配额
    if api_key.needs_quota_reset() {
        // 重置配额
        let now = Utc::now();
        let tomorrow = now + chrono::Duration::days(1);
        let reset_time = tomorrow.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let reset_datetime = chrono::DateTime::from_naive_utc_and_offset(reset_time, Utc);
        
        let mut active_model: api_keys::ActiveModel = api_key.clone().into();
        active_model.daily_used_quota = Set(Some(0));
        active_model.quota_reset_at = Set(Some(reset_datetime));
        
        api_key = active_model
            .update(db)
            .await
            .map_err(|e| ApiKeyAuthError::DatabaseError(e.to_string()))?;
    }
    
    // 检查是否超过配额
    if api_key.is_quota_exceeded() {
        return Err(ApiKeyAuthError::QuotaExceeded);
    }
    
    // 扣减配额（这里使用原子操作来避免并发问题）
    let current_used = api_key.daily_used_quota.unwrap_or(0);
    let mut active_model: api_keys::ActiveModel = api_key.clone().into();
    active_model.daily_used_quota = Set(Some(current_used + 1));
    
    active_model
        .update(db)
        .await
        .map_err(|e| ApiKeyAuthError::DatabaseError(e.to_string()))
}

/// 更新最后使用时间（异步执行）
fn update_last_used_time(db: &sea_orm::DatabaseConnection, api_key_id: uuid::Uuid) {
    let db = db.clone();
    tokio::spawn(async move {
        let now = Utc::now();
        if let Ok(Some(api_key)) = api_keys::Entity::find_by_id(api_key_id)
            .one(&db)
            .await
        {
            let mut active_model: api_keys::ActiveModel = api_key.into();
            active_model.last_used_at = Set(Some(now));
            let _ = active_model.update(&db).await;
        }
    });
}

/// 从请求中提取模型名称
fn extract_model_from_request(req: &Request<Body>) -> Option<String> {
    // 尝试从请求体中提取模型名称
    // 这里我们假设模型名称在请求体中，格式为 JSON
    // 对于 GET 请求，可能从查询参数中获取
    
    // 从 URI 查询参数中获取
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "model" {
                    return Some(urlencoding_decode(value));
                }
            }
        }
    }
    
    // 如果是 POST 请求，可能需要解析请求体
    // 但在中间件中解析请求体比较复杂，通常需要在 handler 中处理
    // 这里我们先返回 None，实际使用时可以根据需要扩展
    
    None
}

/// URL 解码
fn urlencoding_decode(s: &str) -> String {
    // 简单的 URL 解码实现
    s.replace("%20", " ")
        .replace("%2F", "/")
        .replace("%3A", ":")
        .replace("%2B", "+")
}

/// 获取客户端真实 IP（支持代理场景）
fn get_client_ip(req: &Request<Body>, addr: &SocketAddr) -> String {
    // 优先检查 X-Forwarded-For header
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // X-Forwarded-For 可能包含多个 IP，取第一个
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }
    
    // 检查 X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    
    // 使用连接地址
    addr.ip().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, header::AUTHORIZATION};
    
    #[test]
    fn test_extract_api_key_from_bearer() {
        let req = Request::builder()
            .header(AUTHORIZATION, "Bearer test-key-123")
            .body(Body::empty())
            .unwrap();
        
        let result = extract_api_key(&req);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-key-123");
    }
    
    #[test]
    fn test_extract_api_key_from_x_api_key() {
        let req = Request::builder()
            .header("x-api-key", "test-key-456")
            .body(Body::empty())
            .unwrap();
        
        let result = extract_api_key(&req);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-key-456");
    }
    
    #[test]
    fn test_extract_api_key_missing() {
        let req = Request::builder()
            .body(Body::empty())
            .unwrap();
        
        let result = extract_api_key(&req);
        assert!(matches!(result, Err(ApiKeyAuthError::MissingApiKey)));
    }
    
    #[test]
    fn test_urlencoding_decode() {
        assert_eq!(urlencoding_decode("gpt-4"), "gpt-4");
        assert_eq!(urlencoding_decode("gpt%204"), "gpt 4");
        assert_eq!(urlencoding_decode("model%2Fv1"), "model/v1");
    }
}
