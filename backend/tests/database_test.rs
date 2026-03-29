#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 数据库集成测试
//!
//! 注意: 这些测试需要实际数据库连接,使用运行时查询而非编译时宏

use sqlx::postgres::PgPoolOptions;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要实际数据库连接
    async fn test_database_connection() {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/foxnio_test".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await;

        assert!(pool.is_ok());

        let pool = pool.unwrap();

        // 测试查询
        let result: Result<(i32,), sqlx::Error> = sqlx::query_as("SELECT 1").fetch_one(&pool).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, 1);
    }

    #[tokio::test]
    #[ignore]
    async fn test_user_operations() {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/foxnio_test".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // 创建测试用户
        let user_id = uuid::Uuid::new_v4();
        let email = format!("test_{}@example.com", user_id);

        let result: Result<(uuid::Uuid,), sqlx::Error> = sqlx::query_as(
            r#"
            INSERT INTO users (id, email, password_hash, balance, role, status)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind("hashed_password")
        .bind(0i64)
        .bind("user")
        .bind("active")
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok());

        // 查询用户
        let user: Result<(uuid::Uuid, String, i64, String, String), sqlx::Error> = sqlx::query_as(
            r#"
            SELECT id, email, balance, role, status
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await;

        assert!(user.is_ok());
        let user = user.unwrap();
        assert_eq!(user.1, email);

        // 删除测试用户
        let _: Result<_, sqlx::Error> = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_account_operations() {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/foxnio_test".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // 创建测试账号
        let account_id = uuid::Uuid::new_v4();

        let result: Result<(uuid::Uuid,), sqlx::Error> = sqlx::query_as(
            r#"
            INSERT INTO accounts (id, name, provider, api_key, status, priority, weight)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(account_id)
        .bind("Test Account")
        .bind("openai")
        .bind("sk-test-key")
        .bind("active")
        .bind(1i32)
        .bind(1i32)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok());

        // 清理
        let _: Result<_, sqlx::Error> = sqlx::query("DELETE FROM accounts WHERE id = $1")
            .bind(account_id)
            .execute(&pool)
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_usage_tracking() {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/foxnio_test".to_string()
        });

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to database");

        // 创建测试用户和账号
        let user_id = uuid::Uuid::new_v4();
        let account_id = uuid::Uuid::new_v4();
        let usage_id = uuid::Uuid::new_v4();

        // 插入使用记录
        let result: Result<(uuid::Uuid,), sqlx::Error> = sqlx::query_as(
            r#"
            INSERT INTO usages (id, user_id, account_id, model, input_tokens, output_tokens, cost)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(usage_id)
        .bind(user_id)
        .bind(account_id)
        .bind("gpt-4")
        .bind(100i32)
        .bind(50i32)
        .bind(150i64)
        .fetch_one(&pool)
        .await;

        assert!(result.is_ok());

        // 查询使用统计
        let stats: Result<(i64, Option<i64>, Option<i64>, Option<i64>), sqlx::Error> =
            sqlx::query_as(
                r#"
            SELECT 
                COUNT(*) as total_requests,
                SUM(input_tokens) as total_input_tokens,
                SUM(output_tokens) as total_output_tokens,
                SUM(cost) as total_cost
            FROM usages
            WHERE user_id = $1
            "#,
            )
            .bind(user_id)
            .fetch_one(&pool)
            .await;

        assert!(stats.is_ok());

        // 清理
        let _: Result<_, sqlx::Error> = sqlx::query("DELETE FROM usages WHERE id = $1")
            .bind(usage_id)
            .execute(&pool)
            .await;
    }
}
