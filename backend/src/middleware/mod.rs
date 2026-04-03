//! 中间件模块

pub mod auth;

pub use auth::{can_access_user, is_admin, is_super_admin, Role};
