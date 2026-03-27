//! 中间件模块

pub mod auth;
pub mod compression;
pub mod permission;
pub mod telemetry;
pub mod audit;

pub use auth::*;
pub use compression::{
    CompressionLayer, CompressionLevel, ContentEncoding, CompressedResponse,
    CompressionStats, StatsSnapshot, StreamingCompressor,
    compression_middleware, decompression_middleware,
    get_content_encoding, get_accept_encoding, should_compress,
};
pub use permission::{
    get_permission_service,
    permission_denied, role_denied,
    require_permission_middleware,
    require_role_middleware,
    require_admin,
    require_manager,
    require_any_permission_middleware,
    require_all_permissions_middleware,
    with_permission, with_role,
    check_permission, check_any_permission, check_all_permissions,
};
pub use telemetry::*;
pub use audit::{audit_middleware, sensitive_audit, login_audit, AuditConfig};
