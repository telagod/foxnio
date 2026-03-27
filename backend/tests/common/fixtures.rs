//! 测试数据夹具

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 创建测试用户信息
pub fn create_test_user() -> TestUser {
    TestUser {
        id: Uuid::new_v4(),
        email: format!("test-{}@example.com", Uuid::new_v4()),
        role: "user".to_string(),
        status: "active".to_string(),
        balance: 1000,
        created_at: Utc::now(),
    }
}

/// 创建测试管理员
pub fn create_test_admin() -> TestUser {
    TestUser {
        id: Uuid::new_v4(),
        email: format!("admin-{}@example.com", Uuid::new_v4()),
        role: "admin".to_string(),
        status: "active".to_string(),
        balance: 0,
        created_at: Utc::now(),
    }
}

/// 测试用户结构
#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub created_at: DateTime<Utc>,
}

/// 测试 JWT secret
pub fn test_jwt_secret() -> String {
    "test-jwt-secret-key-for-testing-only".to_string()
}

/// 测试 JWT 过期时间（小时）
pub fn test_jwt_expire_hours() -> u64 {
    24
}

/// 测试 refresh token 过期时间（天）
pub fn test_refresh_token_expire_days() -> u64 {
    7
}
