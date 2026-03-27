//! 配置管理

use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
mod test;

// ============================================================================
// Compression Configuration
// ============================================================================

/// 压缩级别
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionLevelConfig {
    /// 快速压缩
    Fast,
    /// 默认压缩
    Default,
    /// 最佳压缩
    Best,
}

impl Default for CompressionLevelConfig {
    fn default() -> Self {
        Self::Default
    }
}

impl From<CompressionLevelConfig> for crate::gateway::middleware::compression::CompressionLevel {
    fn from(level: CompressionLevelConfig) -> Self {
        match level {
            CompressionLevelConfig::Fast => Self::Fast,
            CompressionLevelConfig::Default => Self::Default,
            CompressionLevelConfig::Best => Self::Best,
        }
    }
}

/// 压缩配置
#[derive(Debug, Clone, Deserialize)]
pub struct CompressionConfig {
    /// 是否启用压缩
    #[serde(default = "default_compression_enabled")]
    pub enabled: bool,

    /// 是否启用 gzip
    #[serde(default = "default_gzip_enabled")]
    pub gzip: bool,

    /// 是否启用 brotli
    #[serde(default = "default_brotli_enabled")]
    pub brotli: bool,

    /// 最小压缩大小 (bytes)
    #[serde(default = "default_min_size")]
    pub min_size: usize,

    /// 压缩级别
    #[serde(default)]
    pub level: CompressionLevelConfig,
}

fn default_compression_enabled() -> bool {
    true
}
fn default_gzip_enabled() -> bool {
    true
}
fn default_brotli_enabled() -> bool {
    true
}
fn default_min_size() -> usize {
    1024
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: default_compression_enabled(),
            gzip: default_gzip_enabled(),
            brotli: default_brotli_enabled(),
            min_size: default_min_size(),
            level: CompressionLevelConfig::Default,
        }
    }
}

// ============================================================================
// HTTP/2 Configuration
// ============================================================================

/// TLS 版本
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TlsVersion {
    /// TLS 1.2
    V12,
    /// TLS 1.3 (推荐)
    V13,
}

impl Default for TlsVersion {
    fn default() -> Self {
        Self::V13
    }
}

/// 客户端认证模式
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClientAuthMode {
    /// 不验证客户端证书
    #[default]
    None,
    /// 可选验证
    Optional,
    /// 必须验证
    Required,
}

/// HTTP/2 服务器配置
#[derive(Debug, Clone, Deserialize)]
pub struct Http2Config {
    /// 是否启用 HTTP/2
    #[serde(default = "default_http2_enabled")]
    pub enabled: bool,

    /// 最大并发流数量
    #[serde(default = "default_max_concurrent_streams")]
    pub max_concurrent_streams: u32,

    /// 初始流窗口大小 (bytes)
    #[serde(default = "default_initial_stream_window_size")]
    pub initial_stream_window_size: u32,

    /// 初始连接窗口大小 (bytes)
    #[serde(default = "default_initial_connection_window_size")]
    pub initial_connection_window_size: u32,

    /// 是否启用自适应窗口
    #[serde(default = "default_adaptive_window")]
    pub adaptive_window: bool,

    /// Keep-alive 间隔 (秒)
    #[serde(default = "default_keep_alive_interval_secs")]
    pub keep_alive_interval_secs: u64,

    /// Keep-alive 超时 (秒)
    #[serde(default = "default_keep_alive_timeout_secs")]
    pub keep_alive_timeout_secs: u64,

    /// 最大帧大小 (bytes)
    #[serde(default = "default_max_frame_size")]
    pub max_frame_size: u32,

    /// 最大头部列表大小 (bytes)
    #[serde(default = "default_max_header_list_size")]
    pub max_header_list_size: u32,

    /// 启用连接重置流
    #[serde(default = "default_enable_connect_protocol")]
    pub enable_connect_protocol: bool,
}

fn default_http2_enabled() -> bool {
    true
}
fn default_max_concurrent_streams() -> u32 {
    200
}
fn default_initial_stream_window_size() -> u32 {
    65535
}
fn default_initial_connection_window_size() -> u32 {
    65535
}
fn default_adaptive_window() -> bool {
    true
}
fn default_keep_alive_interval_secs() -> u64 {
    30
}
fn default_keep_alive_timeout_secs() -> u64 {
    20
}
fn default_max_frame_size() -> u32 {
    16384
}
fn default_max_header_list_size() -> u32 {
    65536
}
fn default_enable_connect_protocol() -> bool {
    false
}

impl Http2Config {
    pub fn keep_alive_interval(&self) -> Duration {
        Duration::from_secs(self.keep_alive_interval_secs)
    }

    pub fn keep_alive_timeout(&self) -> Duration {
        Duration::from_secs(self.keep_alive_timeout_secs)
    }
}

impl Default for Http2Config {
    fn default() -> Self {
        Self {
            enabled: default_http2_enabled(),
            max_concurrent_streams: default_max_concurrent_streams(),
            initial_stream_window_size: default_initial_stream_window_size(),
            initial_connection_window_size: default_initial_connection_window_size(),
            adaptive_window: default_adaptive_window(),
            keep_alive_interval_secs: default_keep_alive_interval_secs(),
            keep_alive_timeout_secs: default_keep_alive_timeout_secs(),
            max_frame_size: default_max_frame_size(),
            max_header_list_size: default_max_header_list_size(),
            enable_connect_protocol: default_enable_connect_protocol(),
        }
    }
}

/// TLS 配置
#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// 是否启用 TLS
    #[serde(default)]
    pub enabled: bool,

    /// 证书文件路径
    #[serde(default)]
    pub cert_path: PathBuf,

    /// 私钥文件路径
    #[serde(default)]
    pub key_path: PathBuf,

    /// 最小 TLS 版本
    #[serde(default)]
    pub min_version: TlsVersion,

    /// 支持的密码套件
    #[serde(default = "default_cipher_suites")]
    pub cipher_suites: Vec<String>,

    /// 是否启用 OCSP Stapling
    #[serde(default)]
    pub ocsp_stapling: bool,

    /// 客户端证书验证
    #[serde(default)]
    pub client_auth: ClientAuthMode,
}

fn default_cipher_suites() -> Vec<String> {
    vec![
        "TLS_AES_256_GCM_SHA384".to_string(),
        "TLS_CHACHA20_POLY1305_SHA256".to_string(),
        "TLS_AES_128_GCM_SHA256".to_string(),
        "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384".to_string(),
        "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384".to_string(),
    ]
}

/// HTTP/2 客户端配置 (用于代理上游请求)
#[derive(Debug, Clone, Deserialize)]
pub struct Http2ClientConfig {
    /// 是否启用 HTTP/2
    #[serde(default = "default_client_http2_enabled")]
    pub enabled: bool,

    /// 连接池大小
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,

    /// 连接超时 (秒)
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,

    /// 请求超时 (秒)
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,

    /// 连接池 Keep-alive 超时 (秒)
    #[serde(default = "default_pool_keep_alive_secs")]
    pub pool_keep_alive_secs: u64,

    /// 最大空闲连接数
    #[serde(default = "default_max_idle_connections")]
    pub max_idle_connections: usize,

    /// TCP Keep-alive 间隔 (秒)
    #[serde(default = "default_tcp_keepalive_secs")]
    pub tcp_keepalive_secs: u64,

    /// 启用 TCP_NODELAY
    #[serde(default = "default_tcp_nodelay")]
    pub tcp_nodelay: bool,

    /// HTTP/2 初始流窗口大小
    #[serde(default = "default_client_initial_stream_window_size")]
    pub initial_stream_window_size: u32,

    /// HTTP/2 最大并发流
    #[serde(default = "default_client_max_concurrent_streams")]
    pub max_concurrent_streams: u32,

    /// 自动协商协议 (HTTP/2 或 HTTP/1.1)
    #[serde(default = "default_auto_negotiate")]
    pub auto_negotiate: bool,
}

fn default_client_http2_enabled() -> bool {
    true
}
fn default_pool_size() -> usize {
    16
}
fn default_connect_timeout_secs() -> u64 {
    10
}
fn default_request_timeout_secs() -> u64 {
    300
}
fn default_pool_keep_alive_secs() -> u64 {
    90
}
fn default_max_idle_connections() -> usize {
    32
}
fn default_tcp_keepalive_secs() -> u64 {
    60
}
fn default_tcp_nodelay() -> bool {
    true
}
fn default_client_initial_stream_window_size() -> u32 {
    65535
}
fn default_client_max_concurrent_streams() -> u32 {
    100
}
fn default_auto_negotiate() -> bool {
    true
}

impl Http2ClientConfig {
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.connect_timeout_secs)
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }

    pub fn pool_keep_alive(&self) -> Duration {
        Duration::from_secs(self.pool_keep_alive_secs)
    }

    pub fn tcp_keepalive(&self) -> Duration {
        Duration::from_secs(self.tcp_keepalive_secs)
    }
}

impl Default for Http2ClientConfig {
    fn default() -> Self {
        Self {
            enabled: default_client_http2_enabled(),
            pool_size: default_pool_size(),
            connect_timeout_secs: default_connect_timeout_secs(),
            request_timeout_secs: default_request_timeout_secs(),
            pool_keep_alive_secs: default_pool_keep_alive_secs(),
            max_idle_connections: default_max_idle_connections(),
            tcp_keepalive_secs: default_tcp_keepalive_secs(),
            tcp_nodelay: default_tcp_nodelay(),
            initial_stream_window_size: default_client_initial_stream_window_size(),
            max_concurrent_streams: default_client_max_concurrent_streams(),
            auto_negotiate: default_auto_negotiate(),
        }
    }
}

/// 服务器完整配置
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    /// HTTP/2 配置
    #[serde(default)]
    pub http2: Http2Config,

    /// TLS 配置
    #[serde(default)]
    pub tls: Option<TlsConfig>,

    /// HTTP/2 客户端配置
    #[serde(default)]
    pub http2_client: Http2ClientConfig,
}

// ============================================================================
// Main Configuration
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub http2: Http2Config,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    #[serde(default)]
    pub http2_client: Http2ClientConfig,
    #[serde(default)]
    pub compression: CompressionConfig,
    #[serde(default)]
    pub email: Option<crate::service::email::EmailConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfigLegacy {
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
        let config_path =
            std::env::var("FOXNIO_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());

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
            format!(
                "redis://{}:{}/{}",
                self.redis.host, self.redis.port, self.redis.db
            )
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
            server: ServerConfig::default(),
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
            http2: Http2Config::default(),
            tls: None,
            http2_client: Http2ClientConfig::default(),
            compression: CompressionConfig::default(),
            email: None,
        }
    }
}
