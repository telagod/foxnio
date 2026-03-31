//! 权限中间件 - 路由保护
//!
//! 提供基于角色和权限的路由保护中间件。

use once_cell::sync::Lazy;
use std::sync::Arc;

use crate::service::permission::{Permission, PermissionService};
use crate::service::user::Claims;

/// 全局权限服务实例
static PERMISSION_SERVICE: Lazy<Arc<PermissionService>> =
    Lazy::new(|| Arc::new(PermissionService::new()));

/// 获取权限服务实例
pub fn get_permission_service() -> Arc<PermissionService> {
    PERMISSION_SERVICE.clone()
}

/// 检查用户是否有指定权限（用于处理器内部检查）
pub async fn check_permission(claims: &Claims, permission: Permission) -> Result<(), String> {
    let service = get_permission_service();

    if !service.has_permission(&claims.role, permission).await {
        return Err(format!("Permission '{permission}' is required"));
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
            jti: None,
            is_temp: false,
        }
    }

    #[tokio::test]
    async fn test_check_permission_admin() {
        let admin_claims = create_test_claims("admin");

        assert!(check_permission(&admin_claims, Permission::UserRead)
            .await
            .is_ok());
        assert!(check_permission(&admin_claims, Permission::UserDelete)
            .await
            .is_ok());
        assert!(check_permission(&admin_claims, Permission::SystemConfig)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_check_permission_user() {
        let user_claims = create_test_claims("user");

        assert!(check_permission(&user_claims, Permission::ApiKeyRead)
            .await
            .is_ok());
        assert!(check_permission(&user_claims, Permission::UserDelete)
            .await
            .is_err());
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
