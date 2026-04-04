#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! 集成测试

use foxnio::Config;

#[tokio::test]
async fn test_config_default() {
    let config = Config::default();

    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 8080);
    assert!(config.server.http2.enabled);
    assert_eq!(config.database.dbname, "foxnio");
    assert_eq!(config.gateway.api_key_prefix, "sk-");
}

#[tokio::test]
async fn test_database_url() {
    let config = Config {
        database: foxnio::config::DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            user: "test".to_string(),
            password: "pass".to_string(),
            dbname: "testdb".to_string(),
            max_connections: 5,
        },
        ..Default::default()
    };

    let url = config.database_url();
    assert_eq!(url, "postgres://test:pass@localhost:5432/testdb");
}

// 注意：需要真实数据库连接的测试应使用 testcontainers 或 mock
