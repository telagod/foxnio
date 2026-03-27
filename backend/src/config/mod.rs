//! 配置管理

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[cfg(test)]
mod test;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub gateway: GatewayConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub mode: String, // "debug" | "release"
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub db: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expire_hours: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    pub user_concurrency: u32,
    pub user_balance: i64,
    pub api_key_prefix: String,
    pub rate_multiplier: f64,
}

impl Config {
    /// 从文件加载配置
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("FOXNIO_CONFIG")
            .unwrap_or_else(|_| "config.yaml".to_string());
        
        Self::from_file(&config_path)
    }

    /// 从文件加载
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// 数据库连接 URL
    pub fn database_url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.database.user,
            self.database.password,
            self.database.host,
            self.database.port,
            self.database.dbname
        )
    }

    /// Redis 连接 URL
    pub fn redis_url(&self) -> String {
        if self.redis.password.is_empty() {
            format!("redis://{}:{}/{}", self.redis.host, self.redis.port, self.redis.db)
        } else {
            format!(
                "redis://:{}@{}:{}/{}",
                self.redis.password, self.redis.host, self.redis.port, self.redis.db
            )
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                mode: "debug".to_string(),
            },
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                user: "postgres".to_string(),
                password: String::new(),
                dbname: "foxnio".to_string(),
                max_connections: 10,
            },
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password: String::new(),
                db: 0,
            },
            jwt: JwtConfig {
                secret: "change-this-to-a-secure-random-string".to_string(),
                expire_hours: 24,
            },
            gateway: GatewayConfig {
                user_concurrency: 5,
                user_balance: 0,
                api_key_prefix: "sk-".to_string(),
                rate_multiplier: 1.0,
            },
        }
    }
}
