//! 审计中间件 - Audit Middleware
//!
//! 自动记录 API 请求和敏感操作的审计日志
//!
//! 注意：部分功能正在开发中，暂未完全使用

#![allow(dead_code)]

use axum::{
    body::Body, http::Request, middleware::Next, response::Response, Extension,
};
use std::time::Instant;
use uuid::Uuid;

use crate::entity::audit_logs::AuditAction;
use crate::gateway::SharedState;
use crate::service::{AuditEntry, AuditService};

/// 审计中间件配置
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// 是否记录所有请求
    pub log_all_requests: bool,
    /// 是否记录请求体
    pub log_request_body: bool,
    /// 需要排除的路径（不记录审计日志）
    pub excluded_paths: Vec<String>,
    /// 敏感路径（需要详细记录）
    pub sensitive_paths: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_all_requests: true,
            log_request_body: false, // 默认不记录请求体，避免敏感数据泄露
            excluded_paths: vec![
                "/health".to_string(),
                "/health/live".to_string(),
                "/health/ready".to_string(),
                "/metrics".to_string(),
            ],
            sensitive_paths: vec![
                "/api/v1/auth/login".to_string(),
                "/api/v1/auth/register".to_string(),
                "/api/v1/auth/change-password".to_string(),
                "/api/v1/user/apikeys".to_string(),
            ],
        }
    }
}

/// 审计中间件
pub async fn audit_middleware(
    Extension(state): Extension<SharedState>,
    Extension(audit_config): Extension<AuditConfig>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    // 检查是否为排除路径
    if audit_config
        .excluded_paths
        .iter()
        .any(|p| path.starts_with(p))
    {
        return next.run(req).await;
    }

    // 提取用户信息
    let user_id = req.extensions().get::<Uuid>().copied();

    // 提取客户端信息
    let ip_address = extract_ip(&req);
    let user_agent = extract_user_agent(&req);

    // 判断是否为敏感操作
    let is_sensitive = audit_config
        .sensitive_paths
        .iter()
        .any(|p| path.starts_with(p));

    // 读取请求体（如果需要）
    let request_data = if audit_config.log_request_body && is_sensitive {
        let body_bytes = axum::body::to_bytes(std::mem::take(req.body_mut()), 1024 * 1024)
            .await
            .ok();
        body_bytes.and_then(|b| serde_json::from_slice(&b).ok())
    } else {
        None
    };

    // 如果读取了请求体，需要重新设置请求体
    if request_data.is_some() {
        if let Some(ref data) = request_data {
            let body = serde_json::to_vec(data).unwrap_or_default();
            *req.body_mut() = Body::from(body);
        }
    }

    // 执行请求
    // 在移动 req 之前保存 request_id
    let request_id = req.extensions().get::<String>().cloned();
    let response = next.run(req).await;

    // 记录审计日志
    let status = response.status().as_u16();
    let _elapsed = start.elapsed();

    // 确定审计动作
    let action = determine_action(&method, &path);

    // 异步记录审计日志（不阻塞响应）
    if audit_config.log_all_requests || is_sensitive {
        let audit_service = AuditService::new(state.db.clone());

        let entry = AuditEntry {
            user_id,
            action,
            resource_type: Some(extract_resource_type(&path)),
            resource_id: extract_resource_id(&path),
            ip_address,
            user_agent,
            request_data: if is_sensitive { request_data } else { None },
            response_status: Some(status as i32),
        };

        // 异步写入
        tokio::spawn(async move {
            if let Err(e) = audit_service.log(entry).await {
                tracing::error!("Failed to write audit log: {}", e);
            }
        });
    }

    // 添加审计头
    let mut response = response;
    if let Some(rid) = request_id {
        response
            .headers_mut()
            .insert("x-audit-id", rid.parse().unwrap());
    }

    response
}

/// 从请求中提取 IP 地址
fn extract_ip(req: &Request<Body>) -> Option<String> {
    // 尝试从 X-Forwarded-For 获取
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            // 尝试从 X-Real-IP 获取
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            // 尝试从连接信息获取
            req.extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|addr| addr.ip().to_string())
        })
}

/// 从请求中提取 User-Agent
fn extract_user_agent(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// 根据请求确定审计动作
fn determine_action(method: &axum::http::Method, path: &str) -> String {
    // 登录
    if path.ends_with("/auth/login") {
        return AuditAction::UserLogin.as_str().to_string();
    }

    // 注册
    if path.ends_with("/auth/register") {
        return AuditAction::UserRegister.as_str().to_string();
    }

    // 登出
    if path.ends_with("/auth/logout") {
        return AuditAction::UserLogout.as_str().to_string();
    }

    // 密码修改
    if path.contains("password") || path.contains("change-password") {
        return AuditAction::PasswordChange.as_str().to_string();
    }

    // API Key 管理
    if path.contains("/apikeys") {
        return match method.as_str() {
            "POST" => AuditAction::ApiKeyCreate.as_str().to_string(),
            "DELETE" => AuditAction::ApiKeyDelete.as_str().to_string(),
            _ => AuditAction::ApiKeyUpdate.as_str().to_string(),
        };
    }

    // 账户管理
    if path.contains("/admin/accounts") {
        return match method.as_str() {
            "POST" => AuditAction::AccountCreate.as_str().to_string(),
            "DELETE" => AuditAction::AccountDelete.as_str().to_string(),
            _ => AuditAction::AccountUpdate.as_str().to_string(),
        };
    }

    // 管理员操作
    if path.contains("/admin/") {
        return AuditAction::AdminAction.as_str().to_string();
    }

    // 余额更新
    if path.contains("/balance") {
        return AuditAction::BalanceUpdate.as_str().to_string();
    }

    // 默认为 API 请求
    AuditAction::ApiRequest.as_str().to_string()
}

/// 从路径中提取资源类型
fn extract_resource_type(path: &str) -> String {
    if path.contains("/users") {
        "user".to_string()
    } else if path.contains("/apikeys") {
        "api_key".to_string()
    } else if path.contains("/accounts") {
        "account".to_string()
    } else if path.contains("/chat") || path.contains("/completions") {
        "api_request".to_string()
    } else {
        "unknown".to_string()
    }
}

/// 从路径中提取资源 ID
fn extract_resource_id(path: &str) -> Option<String> {
    // 尝试从路径中提取 UUID
    let parts: Vec<&str> = path.split('/').collect();
    for part in parts {
        if let Ok(uuid) = Uuid::parse_str(part) {
            return Some(uuid.to_string());
        }
    }
    None
}

/// 敏感操作审计中间件（用于特定的敏感端点）
pub async fn sensitive_audit(
    Extension(state): Extension<SharedState>,
    Extension(user_id): Extension<Uuid>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let ip_address = extract_ip(&req);
    let user_agent = extract_user_agent(&req);

    // 执行请求
    let response = next.run(req).await;
    let status = response.status().as_u16();

    // 记录敏感操作审计日志
    let audit_service = AuditService::new(state.db.clone());
    let action = determine_action(&method, &path);

    let entry = AuditEntry {
        user_id: Some(user_id),
        action,
        resource_type: Some(extract_resource_type(&path)),
        resource_id: extract_resource_id(&path),
        ip_address,
        user_agent,
        request_data: None,
        response_status: Some(status as i32),
    };

    // 同步写入敏感操作日志
    if let Err(e) = audit_service.log_sync(entry).await {
        tracing::error!("Failed to write sensitive audit log: {}", e);
    }

    response
}

/// 登录审计中间件
pub async fn login_audit(
    Extension(state): Extension<SharedState>,
    Extension(user_id): Extension<Uuid>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip_address = extract_ip(&req);
    let user_agent = extract_user_agent(&req);

    // 执行请求
    let response = next.run(req).await;
    let status = response.status();

    // 只在登录成功时记录
    if status.is_success() {
        let audit_service = AuditService::new(state.db.clone());
        let entry = AuditEntry::user_login(user_id, ip_address, user_agent);

        tokio::spawn(async move {
            if let Err(e) = audit_service.log_sync(entry).await {
                tracing::error!("Failed to write login audit log: {}", e);
            }
        });
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_action() {
        assert_eq!(
            determine_action(&axum::http::Method::POST, "/api/v1/auth/login"),
            AuditAction::UserLogin.as_str()
        );
        assert_eq!(
            determine_action(&axum::http::Method::POST, "/api/v1/auth/register"),
            AuditAction::UserRegister.as_str()
        );
        assert_eq!(
            determine_action(&axum::http::Method::POST, "/api/v1/user/apikeys"),
            AuditAction::ApiKeyCreate.as_str()
        );
        assert_eq!(
            determine_action(&axum::http::Method::DELETE, "/api/v1/user/apikeys/123"),
            AuditAction::ApiKeyDelete.as_str()
        );
    }

    #[test]
    fn test_extract_resource_type() {
        assert_eq!(extract_resource_type("/api/v1/users/123"), "user");
        assert_eq!(extract_resource_type("/api/v1/user/apikeys"), "api_key");
        assert_eq!(extract_resource_type("/v1/chat/completions"), "api_request");
    }

    #[test]
    fn test_audit_config_default() {
        let config = AuditConfig::default();
        assert!(config.log_all_requests);
        assert!(!config.log_request_body);
        assert!(config.excluded_paths.contains(&"/health".to_string()));
    }
}
