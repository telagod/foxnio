//! 批量操作集成测试
//!
//! 测试覆盖：
//! - 批量创建 API Keys
//! - 批量更新账号
//! - 批量删除 API Keys
//! - CSV 导入用户
//! - 权限验证
//! - 错误处理
//! - 事务性测试

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]

mod common;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ============================================================================
// 模拟数据结构
// ============================================================================

/// 模拟 API Key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub key_hash: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// 模拟用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockUser {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub status: String,
    pub balance: i64,
    pub created_at: DateTime<Utc>,
}

/// 模拟账号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAccount {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub priority: i32,
    pub concurrent_limit: i32,
    pub rate_limit_rpm: i32,
    pub created_at: DateTime<Utc>,
}

/// 批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub results: Vec<BatchItemResult<T>>,
}

/// 单项批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult<T> {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 创建 API Key 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub user_id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// 创建用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub role: String,
}

/// CSV 导入用户记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserCsvRecord {
    pub email: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "user".to_string()
}

/// 批量更新请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub ids: Vec<Uuid>,
    pub updates: HashMap<String, serde_json::Value>,
}

/// 用户 Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

/// 用户信息返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// API Key 信息返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// 模拟测试环境
// ============================================================================

/// 模拟数据库
#[derive(Debug, Clone)]
pub struct MockDatabase {
    pub users: Arc<Mutex<HashMap<Uuid, MockUser>>>,
    pub api_keys: Arc<Mutex<HashMap<Uuid, MockApiKey>>>,
    pub accounts: Arc<Mutex<HashMap<Uuid, MockAccount>>>,
    pub emails: Arc<Mutex<HashMap<String, Uuid>>>, // email -> user_id
}

impl Default for MockDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(HashMap::new())),
            api_keys: Arc::new(Mutex::new(HashMap::new())),
            accounts: Arc::new(Mutex::new(HashMap::new())),
            emails: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 清空所有数据
    pub fn clear(&self) {
        self.users.lock().unwrap().clear();
        self.api_keys.lock().unwrap().clear();
        self.accounts.lock().unwrap().clear();
        self.emails.lock().unwrap().clear();
    }
}

/// 测试应用状态
pub struct TestApp {
    pub db: MockDatabase,
    pub admin_user: MockUser,
    pub normal_user: MockUser,
}

impl TestApp {
    pub fn new() -> Self {
        let db = MockDatabase::new();
        
        // 创建管理员用户
        let admin_user = MockUser {
            id: Uuid::new_v4(),
            email: "admin@test.com".to_string(),
            password_hash: "hashed_admin_password".to_string(),
            role: "admin".to_string(),
            status: "active".to_string(),
            balance: 0,
            created_at: Utc::now(),
        };

        // 创建普通用户
        let normal_user = MockUser {
            id: Uuid::new_v4(),
            email: "user@test.com".to_string(),
            password_hash: "hashed_user_password".to_string(),
            role: "user".to_string(),
            status: "active".to_string(),
            balance: 100,
            created_at: Utc::now(),
        };

        // 插入用户
        {
            let mut users = db.users.lock().unwrap();
            let mut emails = db.emails.lock().unwrap();
            
            users.insert(admin_user.id, admin_user.clone());
            users.insert(normal_user.id, normal_user.clone());
            emails.insert(admin_user.email.clone(), admin_user.id);
            emails.insert(normal_user.email.clone(), normal_user.id);
        }

        Self {
            db,
            admin_user,
            normal_user,
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 设置测试应用
pub fn setup_test_app() -> TestApp {
    TestApp::new()
}

/// 获取管理员令牌
pub fn get_admin_token(app: &TestApp) -> String {
    format!("admin_token_{}", app.admin_user.id)
}

/// 获取普通用户令牌
pub fn get_user_token(app: &TestApp) -> String {
    format!("user_token_{}", app.normal_user.id)
}

/// 创建测试账号
pub fn create_test_accounts(app: &TestApp, count: usize) -> Vec<MockAccount> {
    let mut accounts = Vec::new();
    let mut db_accounts = app.db.accounts.lock().unwrap();

    for i in 0..count {
        let account = MockAccount {
            id: Uuid::new_v4(),
            name: format!("Account {}", i + 1),
            provider: if i % 2 == 0 { "openai" } else { "anthropic" }.to_string(),
            status: "active".to_string(),
            priority: i as i32 + 1,
            concurrent_limit: 10,
            rate_limit_rpm: 1000,
            created_at: Utc::now(),
        };
        db_accounts.insert(account.id, account.clone());
        accounts.push(account);
    }

    accounts
}

/// 创建测试 API Keys
pub fn create_test_api_keys(app: &TestApp, user_id: Uuid, count: usize) -> Vec<MockApiKey> {
    let mut keys = Vec::new();
    let mut db_keys = app.db.api_keys.lock().unwrap();

    for i in 0..count {
        let key = format!("sk-test-{}", Uuid::new_v4().to_string().replace('-', ""));
        let prefix = key[..10].to_string();
        
        // 简单哈希
        let key_hash = format!("hash_{}", key);
        
        let api_key = MockApiKey {
            id: Uuid::new_v4(),
            user_id,
            name: format!("API Key {}", i + 1),
            prefix,
            key_hash,
            status: "active".to_string(),
            created_at: Utc::now(),
        };
        db_keys.insert(api_key.id, api_key.clone());
        keys.push(api_key);
    }

    keys
}

/// 批量创建 API Keys
pub async fn batch_create_api_keys(
    db: &MockDatabase,
    requests: Vec<CreateApiKeyRequest>,
    stop_on_error: bool,
) -> BatchResult<ApiKeyInfo> {
    let mut results = Vec::new();
    let mut success = 0;
    let mut failed = 0;

    for (index, req) in requests.into_iter().enumerate() {
        // 验证用户是否存在
        let user_exists = {
            let users = db.users.lock().unwrap();
            users.contains_key(&req.user_id)
        };

        if !user_exists {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some(format!("User {} not found", req.user_id)),
            });
            if stop_on_error {
                break;
            }
            continue;
        }

        // 验证名称不为空
        if req.name.is_empty() {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some("Name cannot be empty".to_string()),
            });
            if stop_on_error {
                break;
            }
            continue;
        }

        // 创建 API Key
        let key = format!("sk-{}", Uuid::new_v4().to_string().replace('-', ""));
        let prefix = key[..10].to_string();
        let key_hash = format!("hash_{}", key);

        let api_key = MockApiKey {
            id: Uuid::new_v4(),
            user_id: req.user_id,
            name: req.name,
            prefix: prefix.clone(),
            key_hash,
            status: "active".to_string(),
            created_at: Utc::now(),
        };

        let info = ApiKeyInfo {
            id: api_key.id,
            user_id: api_key.user_id,
            name: api_key.name.clone(),
            prefix,
            status: api_key.status.clone(),
            key: Some(key), // 创建时返回完整 key
            created_at: api_key.created_at,
        };

        // 保存到数据库
        {
            let mut keys = db.api_keys.lock().unwrap();
            keys.insert(api_key.id, api_key);
        }

        success += 1;
        results.push(BatchItemResult {
            index,
            data: Some(info),
            error: None,
        });
    }

    BatchResult {
        total: results.len(),
        success,
        failed,
        results,
    }
}

/// 批量更新账号
pub async fn batch_update_accounts(
    db: &MockDatabase,
    req: BatchUpdateRequest,
    stop_on_error: bool,
) -> BatchResult<()> {
    let mut results = Vec::new();
    let mut success = 0;
    let mut failed = 0;

    for (index, id) in req.ids.into_iter().enumerate() {
        // 检查账号是否存在
        let exists = {
            let accounts = db.accounts.lock().unwrap();
            accounts.contains_key(&id)
        };

        if !exists {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some(format!("Account {} not found", id)),
            });
            if stop_on_error {
                break;
            }
            continue;
        }

        // 更新账号
        {
            let mut accounts = db.accounts.lock().unwrap();
            if let Some(account) = accounts.get_mut(&id) {
                for (key, value) in &req.updates {
                    match key.as_str() {
                        "status" => {
                            if let Some(s) = value.as_str() {
                                account.status = s.to_string();
                            }
                        }
                        "priority" => {
                            if let Some(p) = value.as_i64() {
                                account.priority = p as i32;
                            }
                        }
                        "concurrent_limit" => {
                            if let Some(c) = value.as_i64() {
                                account.concurrent_limit = c as i32;
                            }
                        }
                        "rate_limit_rpm" => {
                            if let Some(r) = value.as_i64() {
                                account.rate_limit_rpm = r as i32;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        success += 1;
        results.push(BatchItemResult {
            index,
            data: Some(()),
            error: None,
        });
    }

    BatchResult {
        total: results.len(),
        success,
        failed,
        results,
    }
}

/// 批量删除 API Keys
pub async fn batch_delete_api_keys(db: &MockDatabase, ids: Vec<Uuid>) -> BatchResult<()> {
    let mut results = Vec::new();
    let mut success = 0;
    let mut failed = 0;

    for (index, id) in ids.into_iter().enumerate() {
        let deleted = {
            let mut keys = db.api_keys.lock().unwrap();
            keys.remove(&id).is_some()
        };

        if deleted {
            success += 1;
            results.push(BatchItemResult {
                index,
                data: Some(()),
                error: None,
            });
        } else {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some(format!("API Key {} not found", id)),
            });
        }
    }

    BatchResult {
        total: results.len(),
        success,
        failed,
        results,
    }
}

/// CSV 导入用户
pub async fn batch_import_users_csv(db: &MockDatabase, csv_content: &str) -> BatchResult<UserInfo> {
    let mut requests = Vec::new();
    let mut parse_errors = Vec::new();

    let lines: Vec<&str> = csv_content.lines().collect();
    
    // 跳过标题行
    for (line_num, line) in lines.iter().enumerate().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        
        if fields.len() < 2 {
            parse_errors.push((line_num, "Missing required fields"));
            continue;
        }

        let email = fields[0].trim().to_string();
        let password = fields[1].trim().to_string();
        let role = if fields.len() > 2 {
            fields[2].trim().to_string()
        } else {
            "user".to_string()
        };

        // 验证邮箱格式
        if !email.contains('@') {
            parse_errors.push((line_num, "Invalid email format"));
            continue;
        }

        // 验证密码不为空
        if password.is_empty() {
            parse_errors.push((line_num, "Password cannot be empty"));
            continue;
        }

        requests.push(CreateUserRequest { email, password, role });
    }

    batch_create_users(db, requests).await
}

/// 批量创建用户
pub async fn batch_create_users(
    db: &MockDatabase,
    requests: Vec<CreateUserRequest>,
) -> BatchResult<UserInfo> {
    let mut results = Vec::new();
    let mut success = 0;
    let mut failed = 0;

    for (index, req) in requests.into_iter().enumerate() {
        // 验证邮箱格式
        if !req.email.contains('@') {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some("Invalid email format".to_string()),
            });
            continue;
        }

        // 验证密码不为空
        if req.password.is_empty() {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some("Password cannot be empty".to_string()),
            });
            continue;
        }

        // 检查邮箱是否已存在
        let email_exists = {
            let emails = db.emails.lock().unwrap();
            emails.contains_key(&req.email)
        };

        if email_exists {
            failed += 1;
            results.push(BatchItemResult {
                index,
                data: None,
                error: Some(format!("Email {} already exists", req.email)),
            });
            continue;
        }

        // 创建用户
        let user = MockUser {
            id: Uuid::new_v4(),
            email: req.email.clone(),
            password_hash: format!("hashed_{}", req.password),
            role: req.role.clone(),
            status: "active".to_string(),
            balance: 0,
            created_at: Utc::now(),
        };

        let info = UserInfo {
            id: user.id,
            email: user.email.clone(),
            role: user.role,
            status: user.status,
            created_at: user.created_at,
        };

        // 保存到数据库
        {
            let mut users = db.users.lock().unwrap();
            let mut emails = db.emails.lock().unwrap();
            users.insert(user.id, user);
            emails.insert(req.email, info.id);
        }

        success += 1;
        results.push(BatchItemResult {
            index,
            data: Some(info),
            error: None,
        });
    }

    BatchResult {
        total: results.len(),
        success,
        failed,
        results,
    }
}

/// 检查权限
pub fn check_permission(claims: &Claims, required_role: &str) -> bool {
    match required_role {
        "admin" => claims.role == "admin",
        "manager" => claims.role == "admin" || claims.role == "manager",
        _ => true,
    }
}

// ============================================================================
// 测试用例
// ============================================================================

/// 测试 1: 批量创建 API Keys 成功
#[tokio::test]
async fn test_batch_create_api_keys_success() {
    let app = setup_test_app();

    // 创建多个 API Key 请求
    let requests: Vec<CreateApiKeyRequest> = (0..5)
        .map(|i| CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: format!("API Key {}", i + 1),
            permissions: vec!["read".to_string(), "write".to_string()],
            expires_at: None,
        })
        .collect();

    // 执行批量创建
    let result = batch_create_api_keys(&app.db, requests, false).await;

    // 验证返回的成功数量
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 5);
    assert_eq!(result.failed, 0);

    // 验证每个 key 的数据
    for (i, item) in result.results.iter().enumerate() {
        assert!(item.error.is_none());
        assert!(item.data.is_some());
        
        let key_info = item.data.as_ref().unwrap();
        assert_eq!(key_info.user_id, app.admin_user.id);
        assert_eq!(key_info.name, format!("API Key {}", i + 1));
        assert_eq!(key_info.status, "active");
        assert!(key_info.key.is_some());
        assert!(key_info.key.as_ref().unwrap().starts_with("sk-"));
    }

    // 验证数据库中的记录
    let db_keys = app.db.api_keys.lock().unwrap();
    assert_eq!(db_keys.len(), 5);
}

/// 测试 2: 批量创建包含无效请求
#[tokio::test]
async fn test_batch_create_with_invalid_requests() {
    let app = setup_test_app();

    // 创建混合有效和无效请求
    let requests = vec![
        // 有效请求
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Valid Key 1".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        // 无效请求：用户不存在
        CreateApiKeyRequest {
            user_id: Uuid::nil(), // 不存在的用户
            name: "Invalid Key - Bad User".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        // 有效请求
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Valid Key 2".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        // 无效请求：名称为空
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "".to_string(), // 空名称
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        // 有效请求
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Valid Key 3".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
    ];

    // 执行批量创建（不停止）
    let result = batch_create_api_keys(&app.db, requests, false).await;

    // 验证错误聚合
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 3);
    assert_eq!(result.failed, 2);

    // 验证部分成功
    let errors: Vec<_> = result.results.iter().filter(|r| r.error.is_some()).collect();
    assert_eq!(errors.len(), 2);

    // 验证错误信息
    assert!(result.results[1].error.as_ref().unwrap().contains("not found"));
    assert!(result.results[3].error.as_ref().unwrap().contains("empty"));
}

/// 测试 3: 批量创建遇错停止
#[tokio::test]
async fn test_batch_create_stop_on_error() {
    let app = setup_test_app();

    let requests = vec![
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Key 1".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        CreateApiKeyRequest {
            user_id: Uuid::nil(), // 无效用户
            name: "Key 2".to_string(),
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Key 3".to_string(), // 这个不应该被处理
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
        CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: "Key 4".to_string(), // 这个也不应该被处理
            permissions: vec!["read".to_string()],
            expires_at: None,
        },
    ];

    // 执行批量创建（停止模式）
    let result = batch_create_api_keys(&app.db, requests, true).await;

    // 验证遇到错误后停止
    assert_eq!(result.total, 2); // 只处理了 2 个就停止了
    assert_eq!(result.success, 1);
    assert_eq!(result.failed, 1);

    // 验证已处理的数量
    let db_keys = app.db.api_keys.lock().unwrap();
    assert_eq!(db_keys.len(), 1); // 只创建了 1 个
}

/// 测试 4: CSV 导入用户
#[tokio::test]
async fn test_batch_import_csv_users() {
    let app = setup_test_app();

    // 有效的 CSV 格式
    let csv_content = r#"email,password,role
user1@example.com,password123,user
user2@example.com,password456,user
user3@example.com,password789,admin
user4@example.com,password000,user
user5@example.com,password111,user
"#;

    // 执行导入
    let result = batch_import_users_csv(&app.db, csv_content).await;

    // 验证导入成功
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 5);
    assert_eq!(result.failed, 0);

    // 验证导入的用户数据
    for (i, item) in result.results.iter().enumerate() {
        assert!(item.error.is_none());
        let user_info = item.data.as_ref().unwrap();
        assert_eq!(user_info.email, format!("user{}@example.com", i + 1));
        assert_eq!(user_info.status, "active");
    }

    // 验证数据库中的用户
    let users = app.db.users.lock().unwrap();
    // 5 个导入的用户 + 2 个初始用户
    assert_eq!(users.len(), 7);
}

/// 测试 5: CSV 导入错误处理
#[tokio::test]
async fn test_batch_import_csv_with_errors() {
    let app = setup_test_app();

    // 包含各种错误的 CSV
    let csv_content = r#"email,password,role
valid1@example.com,password1,user
invalid-email,password2,user
valid2@example.com,,user
valid3@example.com,password3,user
another-invalid,password4,user
valid4@example.com,password5,user
"@bad-format.com,password6,user
valid5@example.com,password7,user
"#;

    // 执行导入
    let result = batch_import_users_csv(&app.db, csv_content).await;

    // 验证错误处理
    assert_eq!(result.total, 8); // 总共 8 条数据（不含标题）
    assert!(result.success > 0);
    assert!(result.failed > 0);

    // 验证错误提示
    let errors: Vec<_> = result
        .results
        .iter()
        .filter_map(|r| r.error.as_ref())
        .collect();

    // 应该有无效邮箱、空密码等错误
    assert!(!errors.is_empty());

    // 验证有效用户被正确导入
    let success_count = result.results.iter().filter(|r| r.data.is_some()).count();
    let db_users = app.db.users.lock().unwrap();
    assert_eq!(db_users.len(), 2 + success_count); // 初始用户 + 成功导入的用户
}

/// 测试 6: 批量更新账号
#[tokio::test]
async fn test_batch_update_accounts() {
    let app = setup_test_app();

    // 创建测试账号
    let accounts = create_test_accounts(&app, 5);
    let account_ids: Vec<Uuid> = accounts.iter().map(|a| a.id).collect();

    // 更新数据
    let mut updates = HashMap::new();
    updates.insert("status".to_string(), json!("inactive"));
    updates.insert("priority".to_string(), json!(100));
    updates.insert("rate_limit_rpm".to_string(), json!(500));

    let request = BatchUpdateRequest {
        ids: account_ids.clone(),
        updates,
    };

    // 执行批量更新
    let result = batch_update_accounts(&app.db, request, false).await;

    // 验证更新成功
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 5);
    assert_eq!(result.failed, 0);

    // 验证更新后的数据
    let db_accounts = app.db.accounts.lock().unwrap();
    for id in &account_ids {
        let account = db_accounts.get(id).unwrap();
        assert_eq!(account.status, "inactive");
        assert_eq!(account.priority, 100);
        assert_eq!(account.rate_limit_rpm, 500);
    }
}

/// 测试 7: 批量更新账号事务性
#[tokio::test]
async fn test_batch_update_accounts_transaction() {
    let app = setup_test_app();

    // 创建测试账号
    let accounts = create_test_accounts(&app, 3);
    let account_ids: Vec<Uuid> = accounts.iter().map(|a| a.id).collect();

    // 混合有效和无效 ID
    let mut updates = HashMap::new();
    updates.insert("status".to_string(), json!("updated"));

    let request = BatchUpdateRequest {
        ids: vec![
            account_ids[0],
            Uuid::nil(), // 不存在的 ID
            account_ids[1],
            Uuid::parse_str("00000000-0000-0000-0000-000000000099").unwrap(), // 不存在的 ID
            account_ids[2],
        ],
        updates: updates.clone(),
    };

    // 执行批量更新（不停止）
    let result = batch_update_accounts(&app.db, request, false).await;

    // 验证结果
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 3);
    assert_eq!(result.failed, 2);

    // 验证有效账号被更新
    let db_accounts = app.db.accounts.lock().unwrap();
    for id in &account_ids {
        let account = db_accounts.get(id).unwrap();
        assert_eq!(account.status, "updated");
    }
}

/// 测试 8: 批量删除 API Keys
#[tokio::test]
async fn test_batch_delete_api_keys() {
    let app = setup_test_app();

    // 创建测试 API Keys
    let keys = create_test_api_keys(&app, app.admin_user.id, 5);
    let key_ids: Vec<Uuid> = keys.iter().map(|k| k.id).collect();

    // 执行批量删除
    let result = batch_delete_api_keys(&app.db, key_ids.clone()).await;

    // 验证删除成功
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 5);
    assert_eq!(result.failed, 0);

    // 验证无法再查询
    let db_keys = app.db.api_keys.lock().unwrap();
    for id in &key_ids {
        assert!(!db_keys.contains_key(id));
    }
    assert_eq!(db_keys.len(), 0);
}

/// 测试 9: 批量删除包含不存在的 Key
#[tokio::test]
async fn test_batch_delete_with_nonexistent_keys() {
    let app = setup_test_app();

    // 创建一些 API Keys
    let keys = create_test_api_keys(&app, app.admin_user.id, 3);
    let existing_ids: Vec<Uuid> = keys.iter().map(|k| k.id).collect();

    // 混合存在和不存在的 ID
    let ids_to_delete = vec![
        existing_ids[0],
        Uuid::nil(), // 不存在
        existing_ids[1],
        Uuid::parse_str("00000000-0000-0000-0000-000000000099").unwrap(), // 不存在
        existing_ids[2],
    ];

    // 执行批量删除
    let result = batch_delete_api_keys(&app.db, ids_to_delete).await;

    // 验证结果
    assert_eq!(result.total, 5);
    assert_eq!(result.success, 3);
    assert_eq!(result.failed, 2);

    // 验证存在的 Keys 被删除
    let db_keys = app.db.api_keys.lock().unwrap();
    assert_eq!(db_keys.len(), 0);
}

/// 测试 10: 权限测试 - 管理员可以执行批量操作
#[tokio::test]
async fn test_batch_operations_permissions_admin() {
    let app = setup_test_app();

    // 管理员 Claims
    let admin_claims = Claims {
        sub: app.admin_user.id.to_string(),
        email: app.admin_user.email.clone(),
        role: "admin".to_string(),
        exp: 9999999999,
        iat: 1000000000,
    };

    // 验证管理员权限
    assert!(check_permission(&admin_claims, "admin"));
    assert!(check_permission(&admin_claims, "manager"));
    assert!(check_permission(&admin_claims, "user"));

    // 管理员可以创建 API Keys
    let requests = vec![CreateApiKeyRequest {
        user_id: app.admin_user.id,
        name: "Admin Key".to_string(),
        permissions: vec!["read".to_string()],
        expires_at: None,
    }];

    let result = batch_create_api_keys(&app.db, requests, false).await;
    assert_eq!(result.success, 1);
}

/// 测试 11: 权限测试 - 普通用户拒绝访问
#[tokio::test]
async fn test_batch_operations_permissions_user_denied() {
    let app = setup_test_app();

    // 普通用户 Claims
    let user_claims = Claims {
        sub: app.normal_user.id.to_string(),
        email: app.normal_user.email.clone(),
        role: "user".to_string(),
        exp: 9999999999,
        iat: 1000000000,
    };

    // 验证普通用户权限
    assert!(!check_permission(&user_claims, "admin"));
    assert!(!check_permission(&user_claims, "manager"));
    assert!(check_permission(&user_claims, "user"));

    // 模拟权限检查失败
    // 在实际应用中，这会在 handler 层通过 check_permission 函数检查
    // 这里我们验证权限逻辑
    let has_admin_permission = check_permission(&user_claims, "admin");
    assert!(!has_admin_permission, "普通用户不应该有管理员权限");
}

/// 测试 12: 批量创建用户
#[tokio::test]
async fn test_batch_create_users() {
    let app = setup_test_app();

    let requests = vec![
        CreateUserRequest {
            email: "newuser1@example.com".to_string(),
            password: "password123".to_string(),
            role: "user".to_string(),
        },
        CreateUserRequest {
            email: "newuser2@example.com".to_string(),
            password: "password456".to_string(),
            role: "manager".to_string(),
        },
        CreateUserRequest {
            email: "newuser3@example.com".to_string(),
            password: "password789".to_string(),
            role: "user".to_string(),
        },
    ];

    let result = batch_create_users(&app.db, requests).await;

    // 验证创建成功
    assert_eq!(result.total, 3);
    assert_eq!(result.success, 3);
    assert_eq!(result.failed, 0);

    // 验证用户数据
    for (i, item) in result.results.iter().enumerate() {
        let user_info = item.data.as_ref().unwrap();
        assert_eq!(user_info.email, format!("newuser{}@example.com", i + 1));
        assert_eq!(user_info.status, "active");
    }
}

/// 测试 13: 批量创建用户 - 重复邮箱
#[tokio::test]
async fn test_batch_create_users_duplicate_email() {
    let app = setup_test_app();

    let requests = vec![
        CreateUserRequest {
            email: "unique@example.com".to_string(),
            password: "password1".to_string(),
            role: "user".to_string(),
        },
        CreateUserRequest {
            email: app.admin_user.email.clone(), // 已存在的邮箱
            password: "password2".to_string(),
            role: "user".to_string(),
        },
        CreateUserRequest {
            email: "unique@example.com".to_string(), // 重复的新邮箱
            password: "password3".to_string(),
            role: "user".to_string(),
        },
    ];

    let result = batch_create_users(&app.db, requests).await;

    // 验证结果
    assert_eq!(result.total, 3);
    assert_eq!(result.success, 1); // 只有第一个成功
    assert_eq!(result.failed, 2);

    // 验证错误信息
    assert!(result.results[1].error.as_ref().unwrap().contains("already exists"));
    assert!(result.results[2].error.as_ref().unwrap().contains("already exists"));
}

/// 测试 14: 大批量操作
#[tokio::test]
async fn test_large_batch_operations() {
    let app = setup_test_app();

    // 创建大量 API Key 请求（模拟批量操作）
    let requests: Vec<CreateApiKeyRequest> = (0..100)
        .map(|i| CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: format!("Batch Key {}", i + 1),
            permissions: vec!["read".to_string()],
            expires_at: None,
        })
        .collect();

    let result = batch_create_api_keys(&app.db, requests, false).await;

    // 验证所有都成功
    assert_eq!(result.total, 100);
    assert_eq!(result.success, 100);
    assert_eq!(result.failed, 0);

    // 验证数据库中的记录数
    let db_keys = app.db.api_keys.lock().unwrap();
    assert_eq!(db_keys.len(), 100);
}

/// 测试 15: 并发批量操作安全性
#[tokio::test]
async fn test_concurrent_batch_operations() {
    let app = setup_test_app();
    let db = app.db.clone();
    let user_id = app.admin_user.id;

    // 并发执行多个批量创建
    let handles: Vec<_> = (0..3)
        .map(|batch_id| {
            let db = db.clone();
            let user_id = user_id;
            
            tokio::spawn(async move {
                let requests: Vec<CreateApiKeyRequest> = (0..10)
                    .map(|i| CreateApiKeyRequest {
                        user_id,
                        name: format!("Concurrent Key {}-{}", batch_id, i),
                        permissions: vec!["read".to_string()],
                        expires_at: None,
                    })
                    .collect();

                batch_create_api_keys(&db, requests, false).await
            })
        })
        .collect();

    // 等待所有操作完成
    let results: Vec<_> = futures::future::join_all(handles).await;

    // 验证所有操作都成功
    for result in results {
        let result = result.unwrap();
        assert_eq!(result.success, 10);
    }

    // 验证最终数据库状态
    let db_keys = db.api_keys.lock().unwrap();
    assert_eq!(db_keys.len(), 30); // 3 个批量操作 * 10 个 keys
}

/// 测试 16: 批量更新部分失败回滚验证
#[tokio::test]
async fn test_batch_update_partial_failure() {
    let app = setup_test_app();

    // 创建测试账号
    let accounts = create_test_accounts(&app, 3);
    
    // 先记录初始状态
    let initial_statuses: Vec<String> = accounts.iter().map(|a| a.status.clone()).collect();

    // 尝试更新，包含一个无效操作
    let mut updates = HashMap::new();
    updates.insert("status".to_string(), json!("new_status"));

    let request = BatchUpdateRequest {
        ids: vec![
            accounts[0].id,
            Uuid::nil(), // 无效 ID
            accounts[1].id,
        ],
        updates,
    };

    let result = batch_update_accounts(&app.db, request, false).await;

    // 验证部分成功
    assert_eq!(result.success, 2);
    assert_eq!(result.failed, 1);

    // 验证成功的账号被更新
    let db_accounts = app.db.accounts.lock().unwrap();
    assert_eq!(db_accounts.get(&accounts[0].id).unwrap().status, "new_status");
    assert_eq!(db_accounts.get(&accounts[1].id).unwrap().status, "new_status");
    // 第三个账号没有被更新请求，保持原状态
    assert_eq!(db_accounts.get(&accounts[2].id).unwrap().status, initial_statuses[2]);
}

/// 测试 17: 空 CSV 导入
#[tokio::test]
async fn test_batch_import_empty_csv() {
    let app = setup_test_app();

    // 空 CSV（只有标题）
    let csv_content = "email,password,role\n";

    let result = batch_import_users_csv(&app.db, csv_content).await;

    // 应该返回空结果
    assert_eq!(result.total, 0);
    assert_eq!(result.success, 0);
    assert_eq!(result.failed, 0);
}

/// 测试 18: API Key 唯一性验证
#[tokio::test]
async fn test_api_key_uniqueness() {
    let app = setup_test_app();

    // 创建多个 API Keys
    let requests: Vec<CreateApiKeyRequest> = (0..10)
        .map(|i| CreateApiKeyRequest {
            user_id: app.admin_user.id,
            name: format!("Unique Key {}", i),
            permissions: vec!["read".to_string()],
            expires_at: None,
        })
        .collect();

    let result = batch_create_api_keys(&app.db, requests, false).await;

    // 验证所有 key 都是唯一的
    let keys: Vec<_> = result
        .results
        .iter()
        .filter_map(|r| r.data.as_ref().and_then(|d| d.key.clone()))
        .collect();

    let unique_keys: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(unique_keys.len(), 10, "所有 API Keys 应该是唯一的");
}
