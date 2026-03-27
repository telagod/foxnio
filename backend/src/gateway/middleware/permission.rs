//! 权限中间件 - 路由保护
//!
//! 提供基于角色和权限的路由保护中间件。

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde_json::json;
use std::sync::Arc;
use once_cell::sync::Lazy;

use crate::service::user::Claims;
use crate::service::permission::{Permission, PermissionService, Role};

/// 全局权限服务实例
static PERMISSION_SERVICE: Lazy<Arc<PermissionService>> = Lazy::new(|| {
    Arc::new(PermissionService::new())
});

/// 获取权限服务实例
pub fn get_permission_service() -> Arc<PermissionService> {
    PERMISSION_SERVICE.clone()
}

/// 权限错误响应
pub fn permission_denied(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "permission_denied",
            "message": message
        })),
    )
        .into_response()
}

/// 角色错误响应
pub fn role_denied(required: &str, actual: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "insufficient_role",
            "message": format!("Role '{}' is required, but you have role '{}'", required, actual)
        })),
    )
        .into_response()
}

/// 需要指定权限的中间件
/// 
/// # Example
/// ```rust
/// // 在路由中使用
/// Router::new()
///     .route("/admin/users", get(list_users))
///     .layer(axum::middleware::from_fn(|req, next| {
///         require_permission(req, next, Permission::UserRead)
///     }));
/// ```
pub async fn require_permission_middleware(
    Extension(claims): Extension<Claims>,
    permission: Permission,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let service = get_permission_service();
    
    if !service.has_permission(&claims.role, permission).await {
        return Err(permission_denied(&format!(
            "Permission '{}' is required",
            permission
        )));
    }
    
    Ok(next.run(req).await)
}

/// 需要指定角色的中间件
pub async fn require_role_middleware(
    Extension(claims): Extension<Claims>,
    role: Role,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    match PermissionService::check_role(&claims, role) {
        Ok(()) => Ok(next.run(req).await),
        Err(e) => Err(role_denied(role.as_str(), &claims.role)),
    }
}

/// 需要管理员权限的中间件
pub async fn require_admin(
    Extension(claims): Extension<Claims>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    if !PermissionService::is_admin_or_higher(&claims) {
        return Err(role_denied("admin", &claims.role));
    }
    
    Ok(next.run(req).await)
}

/// 需要经理或更高权限的中间件
pub async fn require_manager(
    Extension(claims): Extension<Claims>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    if !PermissionService::is_manager_or_higher(&claims) {
        return Err(role_denied("manager", &claims.role));
    }
    
    Ok(next.run(req).await)
}

/// 需要任意一个权限的中间件
pub async fn require_any_permission_middleware(
    Extension(claims): Extension<Claims>,
    permissions: Vec<Permission>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let service = get_permission_service();
    
    for permission in &permissions {
        if service.has_permission(&claims.role, *permission).await {
            return Ok(next.run(req).await);
        }
    }
    
    Err(permission_denied(&format!(
        "Any of permissions {:?} is required",
        permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>()
    )))
}

/// 需要所有权限的中间件
pub async fn require_all_permissions_middleware(
    Extension(claims): Extension<Claims>,
    permissions: Vec<Permission>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let service = get_permission_service();
    
    for permission in &permissions {
        if !service.has_permission(&claims.role, *permission).await {
            return Err(permission_denied(&format!(
                "Permission '{}' is required",
                permission
            )));
        }
    }
    
    Ok(next.run(req).await)
}

// ============ 便捷中间件创建器 ============

/// 创建需要指定权限的中间件
pub fn with_permission(permission: Permission) -> impl Fn(Extension<Claims>, Request<Body>, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Response>> + Send>> + Clone + Send + Sync + 'static {
    move |Extension(claims): Extension<Claims>, req: Request<Body>, next: Next| {
        let permission = permission;
        let service = get_permission_service();
        
        Box::pin(async move {
            if !service.has_permission(&claims.role, permission).await {
                return Err(permission_denied(&format!(
                    "Permission '{}' is required",
                    permission
                )));
            }
            
            Ok(next.run(req).await)
        })
    }
}

/// 创建需要指定角色的中间件
pub fn with_role(role: Role) -> impl Fn(Extension<Claims>, Request<Body>, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Response>> + Send>> + Clone + Send + Sync + 'static {
    move |Extension(claims): Extension<Claims>, req: Request<Body>, next: Next| {
        let role = role;
        
        Box::pin(async move {
            match PermissionService::check_role(&claims, role) {
                Ok(()) => Ok(next.run(req).await),
                Err(_) => Err(role_denied(role.as_str(), &claims.role)),
            }
        })
    }
}

// ============ 权限检查辅助函数 ============

/// 检查用户是否有指定权限（用于处理器内部检查）
pub async fn check_permission(claims: &Claims, permission: Permission) -> Result<(), String> {
    let service = get_permission_service();
    
    if !service.has_permission(&claims.role, permission).await {
        return Err(format!("Permission '{}' is required", permission));
    }
    
    Ok(())
}

/// 检查用户是否有任意一个权限
pub async fn check_any_permission(claims: &Claims, permissions: &[Permission]) -> Result<(), String> {
    let service = get_permission_service();
    
    for permission in permissions {
        if service.has_permission(&claims.role, *permission).await {
            return Ok(());
        }
    }
    
    Err(format!(
        "Any of permissions {:?} is required",
        permissions.iter().map(|p| p.as_str()).collect::<Vec<_>>()
    ))
}

/// 检查用户是否有所有权限
pub async fn check_all_permissions(claims: &Claims, permissions: &[Permission]) -> Result<(), String> {
    let service = get_permission_service();
    
    for permission in permissions {
        if !service.has_permission(&claims.role, *permission).await {
            return Err(format!("Permission '{}' is required", permission));
        }
    }
    
    Ok(())
}

// ============ 测试 ============

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::permission::Permission;

    fn create_test_claims(role: &str) -> Claims {
        Claims {
            sub: "test-user-id".to_string(),
            email: "test@example.com".to_string(),
            role: role.to_string(),
            exp: 0,
            iat: 0,
        }
    }

    #[tokio::test]
    async fn test_check_permission_admin() {
        let admin_claims = create_test_claims("admin");
        
        assert!(check_permission(&admin_claims, Permission::UserRead).await.is_ok());
        assert!(check_permission(&admin_claims, Permission::UserDelete).await.is_ok());
        assert!(check_permission(&admin_claims, Permission::SystemConfig).await.is_ok());
    }

    #[tokio::test]
    async fn test_check_permission_user() {
        let user_claims = create_test_claims("user");
        
        assert!(check_permission(&user_claims, Permission::ApiKeyRead).await.is_ok());
        assert!(check_permission(&user_claims, Permission::UserDelete).await.is_err());
    }

    #[tokio::test]
    async fn test_check_any_permission() {
        let manager_claims = create_test_claims("manager");
        
        assert!(check_any_permission(&manager_claims, &[Permission::UserRead, Permission::UserDelete]).await.is_ok());
        
        let user_claims = create_test_claims("user");
        assert!(check_any_permission(&user_claims, &[Permission::UserRead, Permission::UserDelete]).await.is_err());
    }

    #[tokio::test]
    async fn test_is_admin_or_higher() {
        let admin_claims = create_test_claims("admin");
        let user_claims = create_test_claims("user");
        
        assert!(PermissionService::is_admin_or_higher(&admin_claims));
        assert!(!PermissionService::is_admin_or_higher(&user_claims));
    }

    #[tokio::test]
    async fn test_is_manager_or_higher() {
        let admin_claims = create_test_claims("admin");
        let manager_claims = create_test_claims("manager");
        let user_claims = create_test_claims("user");
        
        assert!(PermissionService::is_manager_or_higher(&admin_claims));
        assert!(PermissionService::is_manager_or_higher(&manager_claims));
        assert!(!PermissionService::is_manager_or_higher(&user_claims));
    }
}
