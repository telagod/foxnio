//! 配置测试

#[cfg(test)]
mod tests {
    use crate::config::{Config, DatabaseConfig, GatewayConfig, RedisConfig};

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert!(config.server.http2.enabled);
    }

    #[test]
    fn test_database_url() {
        let config = Config {
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: "secret".to_string(),
                dbname: "foxnio".to_string(),
                max_connections: 10,
            },
            ..Default::default()
        };

        let url = config.database_url();
        assert_eq!(url, "postgres://postgres:secret@localhost:5432/foxnio");
    }

    #[test]
    fn test_redis_url() {
        let config = Config {
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password: String::new(),
                db: 0,
            },
            ..Default::default()
        };

        let url = config.redis_url();
        assert_eq!(url, "redis://localhost:6379/0");
    }

    #[test]
    fn test_redis_url_with_password() {
        let config = Config {
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password: "secret".to_string(),
                db: 1,
            },
            ..Default::default()
        };

        let url = config.redis_url();
        assert_eq!(url, "redis://:secret@localhost:6379/1");
    }

    #[test]
    fn test_gateway_config() {
        let config = GatewayConfig {
            user_concurrency: 5,
            user_balance: 1000,
            api_key_prefix: "fx-".to_string(),
            rate_multiplier: 1.5,
        };

        assert_eq!(config.user_concurrency, 5);
        assert_eq!(config.user_balance, 1000);
        assert_eq!(config.api_key_prefix, "fx-");
    }

    #[test]
    fn test_database_url_without_password() {
        let config = Config {
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: String::new(),
                dbname: "foxnio".to_string(),
                max_connections: 10,
            },
            ..Default::default()
        };

        let url = config.database_url();
        assert_eq!(url, "postgres://postgres@localhost:5432/foxnio");
    }
}
