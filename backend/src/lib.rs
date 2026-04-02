//! FoxNIO - AI API Gateway
//!
//! 订阅配额分发管理平台

#![allow(dead_code)]

pub mod alert;
pub mod cache;
pub mod config;
pub mod db;
pub mod entity;
pub mod gateway;
pub mod handler;
pub mod health;
pub mod metrics;
pub mod middleware;
pub mod model;
pub mod response;
pub mod server;
pub mod service;
pub mod state;
pub mod utils;

pub use config::Config;
pub use health::{HealthCheck, HealthChecker, HealthStatus};

// Re-export HTTP/2 configuration types
pub use config::{
    ClientAuthMode, Http2ClientConfig, Http2Config, ServerConfig, TlsConfig, TlsVersion,
};
pub use server::{build_tls_server_config, load_certs, load_private_key, TlsError};

// Re-export billing service as public API
pub use service::billing::{BillingService, RecordUsageParams, UsageRecord};
