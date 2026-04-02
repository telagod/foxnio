//! 权限辅助函数模块
//! 
//! 提供权限检查辅助函数，与 gateway/middleware/permission.rs 配合使用

/// 用户角色
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Admin,
    SuperAdmin,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "user" => Some(Self::User),
            "admin" => Some(Self::Admin),
            "superadmin" | "super_admin" => Some(Self::SuperAdmin),
            _ => None,
        }
    }

    /// 检查是否至少拥有指定角色
    pub fn at_least(&self, required: Role) -> bool {
        match (self, required) {
            (Role::SuperAdmin, _) => true,
            (Role::Admin, Role::SuperAdmin) => false,
            (Role::Admin, _) => true,
            (Role::User, Role::User) => true,
            (Role::User, _) => false,
        }
    }
}

/// 检查是否为管理员
pub fn is_admin(role: &str) -> bool {
    Role::from_str(role).map(|r| r.at_least(Role::Admin)).unwrap_or(false)
}

/// 检查是否为超级管理员
pub fn is_super_admin(role: &str) -> bool {
    Role::from_str(role).map(|r| r == Role::SuperAdmin).unwrap_or(false)
}

/// 检查用户是否有权访问资源
pub fn can_access_user(requester_role: &str, requester_id: &str, target_user_id: &str) -> bool {
    // 管理员可以访问所有用户
    if is_admin(requester_role) {
        return true;
    }
    // 普通用户只能访问自己
    requester_id == target_user_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        let user = Role::User;
        let admin = Role::Admin;
        let super_admin = Role::SuperAdmin;

        assert!(super_admin.at_least(Role::Admin));
        assert!(super_admin.at_least(Role::User));
        assert!(admin.at_least(Role::User));
        assert!(!admin.at_least(Role::SuperAdmin));
        assert!(!user.at_least(Role::Admin));
    }

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::from_str("admin"), Some(Role::Admin));
        assert_eq!(Role::from_str("ADMIN"), Some(Role::Admin));
        assert_eq!(Role::from_str("superadmin"), Some(Role::SuperAdmin));
        assert_eq!(Role::from_str("user"), Some(Role::User));
        assert_eq!(Role::from_str("unknown"), None);
    }
}
