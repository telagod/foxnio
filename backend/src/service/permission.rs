//! 权限服务 - 角色权限管理
//!
//! 提供灵活的角色权限系统，支持动态配置和自定义角色。

use crate::service::user::Claims;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 系统角色定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 管理员 - 完全访问权限
    Admin,
    /// 经理 - 管理用户和 API Keys
    Manager,
    /// 普通用户 - 基本访问权限
    User,
    /// 访客 - 只读访问
    Guest,
}

impl Role {
    /// 从字符串解析角色
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "admin" => Some(Self::Admin),
            "manager" => Some(Self::Manager),
            "user" => Some(Self::User),
            "guest" => Some(Self::Guest),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::User => "user",
            Self::Guest => "guest",
        }
    }

    /// 获取角色显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Admin => "管理员",
            Self::Manager => "经理",
            Self::User => "用户",
            Self::Guest => "访客",
        }
    }

    /// 获取所有预定义角色
    pub fn all() -> Vec<Self> {
        vec![Self::Admin, Self::Manager, Self::User, Self::Guest]
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for Role {
    fn default() -> Self {
        Self::User
    }
}

/// 权限定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // ============ 用户管理权限 ============
    /// 查看用户信息
    UserRead,
    /// 创建/编辑用户
    UserWrite,
    /// 删除用户
    UserDelete,

    // ============ API Key 管理权限 ============
    /// 查看 API Key
    ApiKeyRead,
    /// 创建/编辑 API Key
    ApiKeyWrite,
    /// 删除 API Key
    ApiKeyDelete,

    // ============ 账号管理权限 ============
    /// 查看账号信息
    AccountRead,
    /// 创建/编辑账号
    AccountWrite,

    // ============ 系统管理权限 ============
    /// 系统配置
    SystemConfig,
    /// 查看审计日志
    AuditLogRead,
    /// 管理员读取权限
    AdminRead,
    /// 管理员写入权限
    AdminWrite,

    // ============ 订阅管理权限 ============
    /// 查看订阅
    SubscriptionRead,
    /// 管理订阅
    SubscriptionWrite,

    // ============ 计费管理权限 ============
    /// 查看计费信息
    BillingRead,
    /// 管理计费
    BillingWrite,

    // ============ 公告管理权限 ============
    /// 查看公告
    AnnouncementRead,
    /// 管理公告
    AnnouncementWrite,
}

impl Permission {
    /// 从字符串解析权限
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "user_read" | "userread" => Some(Self::UserRead),
            "user_write" | "userwrite" => Some(Self::UserWrite),
            "user_delete" | "userdelete" => Some(Self::UserDelete),
            "api_key_read" | "apikeyread" => Some(Self::ApiKeyRead),
            "api_key_write" | "apikeywrite" => Some(Self::ApiKeyWrite),
            "api_key_delete" | "apikeydelete" => Some(Self::ApiKeyDelete),
            "account_read" | "accountread" => Some(Self::AccountRead),
            "account_write" | "accountwrite" => Some(Self::AccountWrite),
            "system_config" | "systemconfig" => Some(Self::SystemConfig),
            "audit_log_read" | "auditlogread" => Some(Self::AuditLogRead),
            "subscription_read" | "subscriptionread" => Some(Self::SubscriptionRead),
            "subscription_write" | "subscriptionwrite" => Some(Self::SubscriptionWrite),
            "billing_read" | "billingread" => Some(Self::BillingRead),
            "billing_write" | "billingwrite" => Some(Self::BillingWrite),
            "announcement_read" | "announcementread" => Some(Self::AnnouncementRead),
            "announcement_write" | "announcementwrite" => Some(Self::AnnouncementWrite),
            "admin_read" | "adminread" => Some(Self::AdminRead),
            "admin_write" | "adminwrite" => Some(Self::AdminWrite),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserRead => "user_read",
            Self::UserWrite => "user_write",
            Self::UserDelete => "user_delete",
            Self::ApiKeyRead => "api_key_read",
            Self::ApiKeyWrite => "api_key_write",
            Self::ApiKeyDelete => "api_key_delete",
            Self::AccountRead => "account_read",
            Self::AccountWrite => "account_write",
            Self::SystemConfig => "system_config",
            Self::AuditLogRead => "audit_log_read",
            Self::SubscriptionRead => "subscription_read",
            Self::SubscriptionWrite => "subscription_write",
            Self::BillingRead => "billing_read",
            Self::BillingWrite => "billing_write",
            Self::AnnouncementRead => "announcement_read",
            Self::AnnouncementWrite => "announcement_write",
            Self::AdminRead => "admin_read",
            Self::AdminWrite => "admin_write",
        }
    }

    /// 获取权限描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::UserRead => "查看用户信息",
            Self::UserWrite => "创建/编辑用户",
            Self::UserDelete => "删除用户",
            Self::ApiKeyRead => "查看 API Key",
            Self::ApiKeyWrite => "创建/编辑 API Key",
            Self::ApiKeyDelete => "删除 API Key",
            Self::AccountRead => "查看账号信息",
            Self::AccountWrite => "创建/编辑账号",
            Self::SystemConfig => "系统配置管理",
            Self::AuditLogRead => "查看审计日志",
            Self::SubscriptionRead => "查看订阅信息",
            Self::SubscriptionWrite => "管理订阅",
            Self::BillingRead => "查看计费信息",
            Self::BillingWrite => "管理计费",
            Self::AnnouncementRead => "查看公告",
            Self::AnnouncementWrite => "管理公告",
            Self::AdminRead => "管理员读取",
            Self::AdminWrite => "管理员写入",
        }
    }

    /// 获取权限分组
    pub fn group(&self) -> PermissionGroup {
        match self {
            Self::UserRead | Self::UserWrite | Self::UserDelete => PermissionGroup::User,
            Self::ApiKeyRead | Self::ApiKeyWrite | Self::ApiKeyDelete => PermissionGroup::ApiKey,
            Self::AccountRead | Self::AccountWrite => PermissionGroup::Account,
            Self::SystemConfig => PermissionGroup::System,
            Self::AuditLogRead => PermissionGroup::Audit,
            Self::SubscriptionRead | Self::SubscriptionWrite => PermissionGroup::Subscription,
            Self::BillingRead | Self::BillingWrite => PermissionGroup::Billing,
            Self::AnnouncementRead | Self::AnnouncementWrite => PermissionGroup::Announcement,
            Self::AdminRead | Self::AdminWrite => PermissionGroup::Admin,
        }
    }

    /// 获取所有权限
    pub fn all() -> Vec<Self> {
        vec![
            Self::UserRead,
            Self::UserWrite,
            Self::UserDelete,
            Self::ApiKeyRead,
            Self::ApiKeyWrite,
            Self::ApiKeyDelete,
            Self::AccountRead,
            Self::AccountWrite,
            Self::SystemConfig,
            Self::AuditLogRead,
            Self::SubscriptionRead,
            Self::SubscriptionWrite,
            Self::BillingRead,
            Self::BillingWrite,
            Self::AnnouncementRead,
            Self::AnnouncementWrite,
            Self::AdminRead,
            Self::AdminWrite,
        ]
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 权限分组
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionGroup {
    User,
    ApiKey,
    Account,
    System,
    Audit,
    Subscription,
    Billing,
    Announcement,
    Admin,
}

impl PermissionGroup {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::User => "用户管理",
            Self::ApiKey => "API Key 管理",
            Self::Account => "账号管理",
            Self::System => "系统管理",
            Self::Audit => "审计日志",
            Self::Subscription => "订阅管理",
            Self::Billing => "计费管理",
            Self::Announcement => "公告管理",
            Self::Admin => "管理员权限",
        }
    }
}

/// 角色权限配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermissions {
    /// 角色名称
    pub role: String,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 描述
    #[serde(default)]
    pub description: Option<String>,
}

/// 权限配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionConfig {
    /// 角色权限映射
    pub roles: Vec<RolePermissions>,
    /// 自定义角色
    #[serde(default)]
    pub custom_roles: Vec<RolePermissions>,
}

/// 权限服务
pub struct PermissionService {
    /// 角色权限映射
    role_permissions: Arc<RwLock<HashMap<String, HashSet<Permission>>>>,
}

impl PermissionService {
    /// 创建新的权限服务
    pub fn new() -> Self {
        let service = Self {
            role_permissions: Arc::new(RwLock::new(HashMap::new())),
        };
        service.initialize_default_permissions();
        service
    }

    /// 从配置创建权限服务
    pub fn from_config(config: &PermissionConfig) -> Self {
        let service = Self::new();

        // 合并自定义角色配置
        if !config.roles.is_empty() || !config.custom_roles.is_empty() {
            tokio::spawn({
                let role_permissions = service.role_permissions.clone();
                let config = config.clone();
                async move {
                    let mut permissions = role_permissions.write().await;

                    // 应用配置中的角色权限
                    for role_config in config.roles.iter().chain(config.custom_roles.iter()) {
                        let perms: HashSet<Permission> = role_config
                            .permissions
                            .iter()
                            .filter_map(|p| Permission::from_str(p))
                            .collect();
                        permissions.insert(role_config.role.to_lowercase(), perms);
                    }
                }
            });
        }

        service
    }

    /// 初始化默认权限
    fn initialize_default_permissions(&self) {
        let mut permissions = HashMap::new();

        // Admin - 完全访问
        permissions.insert("admin".to_string(), Permission::all().into_iter().collect());

        // Manager - 管理用户和 API Keys
        permissions.insert(
            "manager".to_string(),
            [
                Permission::UserRead,
                Permission::UserWrite,
                Permission::ApiKeyRead,
                Permission::ApiKeyWrite,
                Permission::ApiKeyDelete,
                Permission::AccountRead,
                Permission::AccountWrite,
                Permission::BillingRead,
                Permission::AnnouncementRead,
                Permission::AnnouncementWrite,
            ]
            .into_iter()
            .collect(),
        );

        // User - 普通用户权限
        permissions.insert(
            "user".to_string(),
            [
                Permission::ApiKeyRead,
                Permission::ApiKeyWrite,
                Permission::BillingRead,
                Permission::SubscriptionRead,
                Permission::SubscriptionWrite,
            ]
            .into_iter()
            .collect(),
        );

        // Guest - 只读访问
        permissions.insert(
            "guest".to_string(),
            [
                Permission::ApiKeyRead,
                Permission::BillingRead,
                Permission::AnnouncementRead,
            ]
            .into_iter()
            .collect(),
        );

        // 使用 block_in_place 来初始化
        let role_permissions = self.role_permissions.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut perms = role_permissions.write().await;
                *perms = permissions;
            });
        });
    }

    /// 获取角色的所有权限
    pub async fn get_role_permissions(&self, role: &str) -> Vec<Permission> {
        let permissions = self.role_permissions.read().await;
        permissions
            .get(&role.to_lowercase())
            .map(|p| p.iter().copied().collect())
            .unwrap_or_default()
    }

    /// 检查角色是否拥有指定权限
    pub async fn has_permission(&self, role: &str, permission: Permission) -> bool {
        let permissions = self.role_permissions.read().await;
        permissions
            .get(&role.to_lowercase())
            .map(|p| p.contains(&permission))
            .unwrap_or(false)
    }

    /// 检查用户是否拥有所有指定权限
    pub async fn check_permissions(
        &self,
        claims: &Claims,
        permissions: &[Permission],
    ) -> Result<()> {
        for permission in permissions {
            if !self.has_permission(&claims.role, *permission).await {
                bail!(
                    "Permission denied: missing permission '{}' for role '{}'",
                    permission,
                    claims.role
                );
            }
        }
        Ok(())
    }

    /// 检查用户是否拥有任意一个指定权限
    pub async fn check_any_permission(
        &self,
        claims: &Claims,
        permissions: &[Permission],
    ) -> Result<()> {
        for permission in permissions {
            if self.has_permission(&claims.role, *permission).await {
                return Ok(());
            }
        }
        bail!(
            "Permission denied: missing any of required permissions for role '{}'",
            claims.role
        );
    }

    /// 检查用户角色
    pub fn check_role(claims: &Claims, required_role: Role) -> Result<()> {
        let user_role = Role::from_str(&claims.role)
            .ok_or_else(|| anyhow::anyhow!("Invalid role: {}", claims.role))?;

        match (user_role, required_role) {
            (Role::Admin, _) => Ok(()), // Admin 拥有所有角色权限
            (Role::Manager, Role::User | Role::Guest) => Ok(()),
            (Role::User, Role::Guest) => Ok(()),
            (Role::User, Role::User) => Ok(()),
            _ => bail!(
                "Role '{}' is not authorized for required role '{}'",
                claims.role,
                required_role
            ),
        }
    }

    /// 检查用户是否为管理员或更高
    pub fn is_admin_or_higher(claims: &Claims) -> bool {
        matches!(claims.role.to_lowercase().as_str(), "admin")
    }

    /// 检查用户是否为经理或更高
    pub fn is_manager_or_higher(claims: &Claims) -> bool {
        matches!(claims.role.to_lowercase().as_str(), "admin" | "manager")
    }

    /// 更新角色权限（动态配置）
    pub async fn update_role_permissions(&self, role: &str, permissions: Vec<Permission>) {
        let mut perms = self.role_permissions.write().await;
        perms.insert(role.to_lowercase(), permissions.into_iter().collect());
    }

    /// 添加角色权限
    pub async fn add_role_permission(&self, role: &str, permission: Permission) {
        let mut perms = self.role_permissions.write().await;
        perms
            .entry(role.to_lowercase())
            .or_default()
            .insert(permission);
    }

    /// 移除角色权限
    pub async fn remove_role_permission(&self, role: &str, permission: Permission) {
        let mut perms = self.role_permissions.write().await;
        if let Some(role_perms) = perms.get_mut(&role.to_lowercase()) {
            role_perms.remove(&permission);
        }
    }

    /// 获取所有角色及其权限
    pub async fn get_all_roles(&self) -> HashMap<String, Vec<Permission>> {
        let permissions = self.role_permissions.read().await;
        permissions
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().copied().collect()))
            .collect()
    }

    /// 获取权限矩阵（用于管理界面展示）
    pub async fn get_permission_matrix() -> Vec<PermissionMatrixRow> {
        let groups = [
            PermissionGroup::User,
            PermissionGroup::ApiKey,
            PermissionGroup::Account,
            PermissionGroup::System,
            PermissionGroup::Audit,
            PermissionGroup::Subscription,
            PermissionGroup::Billing,
            PermissionGroup::Announcement,
        ];

        let mut matrix = Vec::new();

        for group in groups {
            let permissions: Vec<Permission> = Permission::all()
                .into_iter()
                .filter(|p| p.group() == group)
                .collect();

            if !permissions.is_empty() {
                matrix.push(PermissionMatrixRow {
                    group: group.display_name().to_string(),
                    permissions: permissions
                        .into_iter()
                        .map(|p| PermissionInfo {
                            name: p.as_str().to_string(),
                            description: p.description().to_string(),
                        })
                        .collect(),
                });
            }
        }

        matrix
    }
}

impl Default for PermissionService {
    fn default() -> Self {
        Self::new()
    }
}

/// 权限矩阵行（用于管理界面）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionMatrixRow {
    pub group: String,
    pub permissions: Vec<PermissionInfo>,
}

/// 权限信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionInfo {
    pub name: String,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_role_permissions() {
        let service = PermissionService::new();

        // Admin 应该拥有所有权限
        let admin_perms = service.get_role_permissions("admin").await;
        assert!(admin_perms.contains(&Permission::UserRead));
        assert!(admin_perms.contains(&Permission::SystemConfig));

        // User 应该只有部分权限
        let user_perms = service.get_role_permissions("user").await;
        assert!(user_perms.contains(&Permission::ApiKeyRead));
        assert!(!user_perms.contains(&Permission::UserDelete));

        // Guest 应该只有只读权限
        let guest_perms = service.get_role_permissions("guest").await;
        assert!(guest_perms.contains(&Permission::ApiKeyRead));
        assert!(!guest_perms.contains(&Permission::ApiKeyWrite));
    }

    #[tokio::test]
    async fn test_has_permission() {
        let service = PermissionService::new();

        assert!(
            service
                .has_permission("admin", Permission::UserDelete)
                .await
        );
        assert!(
            service
                .has_permission("manager", Permission::UserWrite)
                .await
        );
        assert!(!service.has_permission("user", Permission::UserDelete).await);
        assert!(
            !service
                .has_permission("guest", Permission::ApiKeyWrite)
                .await
        );
    }

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::from_str("admin"), Some(Role::Admin));
        assert_eq!(Role::from_str("ADMIN"), Some(Role::Admin));
        assert_eq!(Role::from_str("Manager"), Some(Role::Manager));
        assert_eq!(Role::from_str("unknown"), None);
    }

    #[test]
    fn test_permission_from_str() {
        assert_eq!(
            Permission::from_str("user_read"),
            Some(Permission::UserRead)
        );
        assert_eq!(Permission::from_str("UserRead"), Some(Permission::UserRead));
        assert_eq!(Permission::from_str("unknown"), None);
    }

    #[test]
    fn test_check_role() {
        let admin_claims = Claims {
            sub: "1".to_string(),
            email: "admin@test.com".to_string(),
            role: "admin".to_string(),
            exp: 0,
            iat: 0,
        };

        let user_claims = Claims {
            sub: "2".to_string(),
            email: "user@test.com".to_string(),
            role: "user".to_string(),
            exp: 0,
            iat: 0,
        };

        assert!(PermissionService::check_role(&admin_claims, Role::User).is_ok());
        assert!(PermissionService::check_role(&user_claims, Role::Admin).is_err());
    }

    #[tokio::test]
    async fn test_dynamic_permission_update() {
        let service = PermissionService::new();

        // 添加新角色
        service
            .update_role_permissions(
                "custom",
                vec![Permission::ApiKeyRead, Permission::BillingRead],
            )
            .await;

        assert!(
            service
                .has_permission("custom", Permission::ApiKeyRead)
                .await
        );
        assert!(!service.has_permission("custom", Permission::UserRead).await);

        // 添加权限
        service
            .add_role_permission("custom", Permission::UserRead)
            .await;
        assert!(service.has_permission("custom", Permission::UserRead).await);

        // 移除权限
        service
            .remove_role_permission("custom", Permission::UserRead)
            .await;
        assert!(!service.has_permission("custom", Permission::UserRead).await);
    }
}
