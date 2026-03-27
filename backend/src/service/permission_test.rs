//! 权限系统测试
//!
//! 测试角色权限、权限检查和路由保护

use crate::service::permission::{Permission, PermissionService, Role, PermissionGroup};
use crate::service::user::Claims;

/// 创建测试用的 Claims
fn create_test_claims(role: &str) -> Claims {
    Claims {
        sub: "test-user-id".to_string(),
        email: "test@example.com".to_string(),
        role: role.to_string(),
        exp: 0,
        iat: 0,
    }
}

// ============ 角色测试 ============

#[test]
fn test_role_from_str() {
    // 测试标准角色
    assert_eq!(Role::from_str("admin"), Some(Role::Admin));
    assert_eq!(Role::from_str("ADMIN"), Some(Role::Admin));
    assert_eq!(Role::from_str("Admin"), Some(Role::Admin));
    
    assert_eq!(Role::from_str("manager"), Some(Role::Manager));
    assert_eq!(Role::from_str("MANAGER"), Some(Role::Manager));
    
    assert_eq!(Role::from_str("user"), Some(Role::User));
    assert_eq!(Role::from_str("USER"), Some(Role::User));
    
    assert_eq!(Role::from_str("guest"), Some(Role::Guest));
    assert_eq!(Role::from_str("GUEST"), Some(Role::Guest));
    
    // 测试无效角色
    assert_eq!(Role::from_str("invalid"), None);
    assert_eq!(Role::from_str(""), None);
}

#[test]
fn test_role_as_str() {
    assert_eq!(Role::Admin.as_str(), "admin");
    assert_eq!(Role::Manager.as_str(), "manager");
    assert_eq!(Role::User.as_str(), "user");
    assert_eq!(Role::Guest.as_str(), "guest");
}

#[test]
fn test_role_display_name() {
    assert_eq!(Role::Admin.display_name(), "管理员");
    assert_eq!(Role::Manager.display_name(), "经理");
    assert_eq!(Role::User.display_name(), "用户");
    assert_eq!(Role::Guest.display_name(), "访客");
}

#[test]
fn test_role_all() {
    let all_roles = Role::all();
    assert_eq!(all_roles.len(), 4);
    assert!(all_roles.contains(&Role::Admin));
    assert!(all_roles.contains(&Role::Manager));
    assert!(all_roles.contains(&Role::User));
    assert!(all_roles.contains(&Role::Guest));
}

#[test]
fn test_role_default() {
    assert_eq!(Role::default(), Role::User);
}

// ============ 权限测试 ============

#[test]
fn test_permission_from_str() {
    // 测试用户管理权限
    assert_eq!(Permission::from_str("user_read"), Some(Permission::UserRead));
    assert_eq!(Permission::from_str("UserRead"), Some(Permission::UserRead));
    assert_eq!(Permission::from_str("USER_READ"), Some(Permission::UserRead));
    
    assert_eq!(Permission::from_str("user_write"), Some(Permission::UserWrite));
    assert_eq!(Permission::from_str("user_delete"), Some(Permission::UserDelete));
    
    // 测试 API Key 权限
    assert_eq!(Permission::from_str("api_key_read"), Some(Permission::ApiKeyRead));
    assert_eq!(Permission::from_str("api_key_write"), Some(Permission::ApiKeyWrite));
    assert_eq!(Permission::from_str("api_key_delete"), Some(Permission::ApiKeyDelete));
    
    // 测试账号权限
    assert_eq!(Permission::from_str("account_read"), Some(Permission::AccountRead));
    assert_eq!(Permission::from_str("account_write"), Some(Permission::AccountWrite));
    
    // 测试系统权限
    assert_eq!(Permission::from_str("system_config"), Some(Permission::SystemConfig));
    assert_eq!(Permission::from_str("audit_log_read"), Some(Permission::AuditLogRead));
    
    // 测试无效权限
    assert_eq!(Permission::from_str("invalid"), None);
    assert_eq!(Permission::from_str(""), None);
}

#[test]
fn test_permission_as_str() {
    assert_eq!(Permission::UserRead.as_str(), "user_read");
    assert_eq!(Permission::UserWrite.as_str(), "user_write");
    assert_eq!(Permission::UserDelete.as_str(), "user_delete");
    assert_eq!(Permission::ApiKeyRead.as_str(), "api_key_read");
    assert_eq!(Permission::SystemConfig.as_str(), "system_config");
}

#[test]
fn test_permission_description() {
    assert_eq!(Permission::UserRead.description(), "查看用户信息");
    assert_eq!(Permission::UserWrite.description(), "创建/编辑用户");
    assert_eq!(Permission::UserDelete.description(), "删除用户");
    assert_eq!(Permission::SystemConfig.description(), "系统配置管理");
}

#[test]
fn test_permission_group() {
    assert_eq!(Permission::UserRead.group(), PermissionGroup::User);
    assert_eq!(Permission::UserWrite.group(), PermissionGroup::User);
    assert_eq!(Permission::UserDelete.group(), PermissionGroup::User);
    
    assert_eq!(Permission::ApiKeyRead.group(), PermissionGroup::ApiKey);
    assert_eq!(Permission::ApiKeyWrite.group(), PermissionGroup::ApiKey);
    
    assert_eq!(Permission::AccountRead.group(), PermissionGroup::Account);
    assert_eq!(Permission::SystemConfig.group(), PermissionGroup::System);
    assert_eq!(Permission::AuditLogRead.group(), PermissionGroup::Audit);
}

#[test]
fn test_permission_all() {
    let all_permissions = Permission::all();
    assert!(!all_permissions.is_empty());
    assert!(all_permissions.contains(&Permission::UserRead));
    assert!(all_permissions.contains(&Permission::SystemConfig));
}

// ============ 权限分组测试 ============

#[test]
fn test_permission_group_display_name() {
    assert_eq!(PermissionGroup::User.display_name(), "用户管理");
    assert_eq!(PermissionGroup::ApiKey.display_name(), "API Key 管理");
    assert_eq!(PermissionGroup::Account.display_name(), "账号管理");
    assert_eq!(PermissionGroup::System.display_name(), "系统管理");
    assert_eq!(PermissionGroup::Audit.display_name(), "审计日志");
}

// ============ 权限服务测试 ============

#[tokio::test]
async fn test_permission_service_admin_permissions() {
    let service = PermissionService::new();
    
    // Admin 应该拥有所有权限
    let permissions = service.get_role_permissions("admin").await;
    assert!(!permissions.is_empty());
    assert!(permissions.contains(&Permission::UserRead));
    assert!(permissions.contains(&Permission::UserWrite));
    assert!(permissions.contains(&Permission::UserDelete));
    assert!(permissions.contains(&Permission::ApiKeyRead));
    assert!(permissions.contains(&Permission::SystemConfig));
    assert!(permissions.contains(&Permission::AuditLogRead));
}

#[tokio::test]
async fn test_permission_service_manager_permissions() {
    let service = PermissionService::new();
    
    // Manager 应该有用户管理权限，但没有系统配置权限
    let permissions = service.get_role_permissions("manager").await;
    assert!(permissions.contains(&Permission::UserRead));
    assert!(permissions.contains(&Permission::UserWrite));
    assert!(!permissions.contains(&Permission::UserDelete));
    assert!(permissions.contains(&Permission::ApiKeyRead));
    assert!(!permissions.contains(&Permission::SystemConfig));
}

#[tokio::test]
async fn test_permission_service_user_permissions() {
    let service = PermissionService::new();
    
    // User 应该只有基本权限
    let permissions = service.get_role_permissions("user").await;
    assert!(permissions.contains(&Permission::ApiKeyRead));
    assert!(permissions.contains(&Permission::ApiKeyWrite));
    assert!(!permissions.contains(&Permission::UserRead));
    assert!(!permissions.contains(&Permission::UserDelete));
    assert!(!permissions.contains(&Permission::SystemConfig));
}

#[tokio::test]
async fn test_permission_service_guest_permissions() {
    let service = PermissionService::new();
    
    // Guest 应该只有只读权限
    let permissions = service.get_role_permissions("guest").await;
    assert!(permissions.contains(&Permission::ApiKeyRead));
    assert!(permissions.contains(&Permission::BillingRead));
    assert!(permissions.contains(&Permission::AnnouncementRead));
    assert!(!permissions.contains(&Permission::ApiKeyWrite));
    assert!(!permissions.contains(&Permission::UserRead));
}

#[tokio::test]
async fn test_has_permission() {
    let service = PermissionService::new();
    
    // Admin 权限测试
    assert!(service.has_permission("admin", Permission::UserRead).await);
    assert!(service.has_permission("admin", Permission::UserDelete).await);
    assert!(service.has_permission("admin", Permission::SystemConfig).await);
    
    // Manager 权限测试
    assert!(service.has_permission("manager", Permission::UserRead).await);
    assert!(service.has_permission("manager", Permission::UserWrite).await);
    assert!(!service.has_permission("manager", Permission::UserDelete).await);
    assert!(!service.has_permission("manager", Permission::SystemConfig).await);
    
    // User 权限测试
    assert!(service.has_permission("user", Permission::ApiKeyRead).await);
    assert!(service.has_permission("user", Permission::ApiKeyWrite).await);
    assert!(!service.has_permission("user", Permission::UserDelete).await);
    
    // Guest 权限测试
    assert!(service.has_permission("guest", Permission::ApiKeyRead).await);
    assert!(!service.has_permission("guest", Permission::ApiKeyWrite).await);
}

#[tokio::test]
async fn test_check_permissions() {
    let service = PermissionService::new();
    
    // Admin 检查
    let admin_claims = create_test_claims("admin");
    assert!(service.check_permissions(&admin_claims, &[Permission::UserRead]).await.is_ok());
    assert!(service.check_permissions(&admin_claims, &[Permission::UserDelete, Permission::SystemConfig]).await.is_ok());
    
    // Manager 检查
    let manager_claims = create_test_claims("manager");
    assert!(service.check_permissions(&manager_claims, &[Permission::UserRead]).await.is_ok());
    assert!(service.check_permissions(&manager_claims, &[Permission::UserDelete]).await.is_err());
    
    // User 检查
    let user_claims = create_test_claims("user");
    assert!(service.check_permissions(&user_claims, &[Permission::ApiKeyRead]).await.is_ok());
    assert!(service.check_permissions(&user_claims, &[Permission::UserRead]).await.is_err());
}

#[test]
fn test_check_role() {
    // Admin 角色检查
    let admin_claims = create_test_claims("admin");
    assert!(PermissionService::check_role(&admin_claims, Role::Admin).is_ok());
    assert!(PermissionService::check_role(&admin_claims, Role::Manager).is_ok());
    assert!(PermissionService::check_role(&admin_claims, Role::User).is_ok());
    assert!(PermissionService::check_role(&admin_claims, Role::Guest).is_ok());
    
    // Manager 角色检查
    let manager_claims = create_test_claims("manager");
    assert!(PermissionService::check_role(&manager_claims, Role::Admin).is_err());
    assert!(PermissionService::check_role(&manager_claims, Role::Manager).is_ok());
    assert!(PermissionService::check_role(&manager_claims, Role::User).is_ok());
    assert!(PermissionService::check_role(&manager_claims, Role::Guest).is_ok());
    
    // User 角色检查
    let user_claims = create_test_claims("user");
    assert!(PermissionService::check_role(&user_claims, Role::Admin).is_err());
    assert!(PermissionService::check_role(&user_claims, Role::Manager).is_err());
    assert!(PermissionService::check_role(&user_claims, Role::User).is_ok());
    assert!(PermissionService::check_role(&user_claims, Role::Guest).is_ok());
    
    // Guest 角色检查
    let guest_claims = create_test_claims("guest");
    assert!(PermissionService::check_role(&guest_claims, Role::Admin).is_err());
    assert!(PermissionService::check_role(&guest_claims, Role::Manager).is_err());
    assert!(PermissionService::check_role(&guest_claims, Role::User).is_err());
    assert!(PermissionService::check_role(&guest_claims, Role::Guest).is_ok());
}

#[test]
fn test_is_admin_or_higher() {
    let admin_claims = create_test_claims("admin");
    let manager_claims = create_test_claims("manager");
    let user_claims = create_test_claims("user");
    let guest_claims = create_test_claims("guest");
    
    assert!(PermissionService::is_admin_or_higher(&admin_claims));
    assert!(!PermissionService::is_admin_or_higher(&manager_claims));
    assert!(!PermissionService::is_admin_or_higher(&user_claims));
    assert!(!PermissionService::is_admin_or_higher(&guest_claims));
}

#[test]
fn test_is_manager_or_higher() {
    let admin_claims = create_test_claims("admin");
    let manager_claims = create_test_claims("manager");
    let user_claims = create_test_claims("user");
    let guest_claims = create_test_claims("guest");
    
    assert!(PermissionService::is_manager_or_higher(&admin_claims));
    assert!(PermissionService::is_manager_or_higher(&manager_claims));
    assert!(!PermissionService::is_manager_or_higher(&user_claims));
    assert!(!PermissionService::is_manager_or_higher(&guest_claims));
}

// ============ 动态权限测试 ============

#[tokio::test]
async fn test_dynamic_role_permissions() {
    let service = PermissionService::new();
    
    // 添加自定义角色
    service.update_role_permissions("support", vec![
        Permission::UserRead,
        Permission::BillingRead,
        Permission::AuditLogRead,
    ]).await;
    
    // 验证自定义角色权限
    assert!(service.has_permission("support", Permission::UserRead).await);
    assert!(service.has_permission("support", Permission::BillingRead).await);
    assert!(service.has_permission("support", Permission::AuditLogRead).await);
    assert!(!service.has_permission("support", Permission::UserDelete).await);
    assert!(!service.has_permission("support", Permission::SystemConfig).await);
}

#[tokio::test]
async fn test_add_remove_permission() {
    let service = PermissionService::new();
    
    // 添加权限
    service.add_role_permission("user", Permission::UserRead).await;
    assert!(service.has_permission("user", Permission::UserRead).await);
    
    // 移除权限
    service.remove_role_permission("user", Permission::UserRead).await;
    assert!(!service.has_permission("user", Permission::UserRead).await);
}

#[tokio::test]
async fn test_get_all_roles() {
    let service = PermissionService::new();
    
    let roles = service.get_all_roles().await;
    
    assert!(roles.contains_key("admin"));
    assert!(roles.contains_key("manager"));
    assert!(roles.contains_key("user"));
    assert!(roles.contains_key("guest"));
}

// ============ 权限矩阵测试 ============

#[tokio::test]
async fn test_permission_matrix() {
    let matrix = PermissionService::get_permission_matrix().await;
    
    assert!(!matrix.is_empty());
    
    // 检查分组
    let groups: Vec<&str> = matrix.iter().map(|r| r.group.as_str()).collect();
    assert!(groups.contains(&"用户管理"));
    assert!(groups.contains(&"API Key 管理"));
    assert!(groups.contains(&"账号管理"));
    assert!(groups.contains(&"系统管理"));
}

// ============ 边界情况测试 ============

#[tokio::test]
async fn test_unknown_role() {
    let service = PermissionService::new();
    
    // 未知角色应该没有任何权限
    let permissions = service.get_role_permissions("unknown_role").await;
    assert!(permissions.is_empty());
    
    assert!(!service.has_permission("unknown_role", Permission::UserRead).await);
}

#[tokio::test]
async fn test_case_insensitive_role() {
    let service = PermissionService::new();
    
    // 角色名应该不区分大小写
    assert!(service.has_permission("Admin", Permission::UserRead).await);
    assert!(service.has_permission("ADMIN", Permission::UserRead).await);
    assert!(service.has_permission("admin", Permission::UserRead).await);
}
