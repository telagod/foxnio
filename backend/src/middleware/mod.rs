//! 中间件模块

pub mod auth;

pub use auth::{is_admin, is_super_admin, can_access_user, Role};
