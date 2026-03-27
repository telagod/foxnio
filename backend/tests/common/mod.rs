//! 测试通用模块

pub mod mock_redis;
pub mod fixtures;

pub use mock_redis::MockRedisPool;
pub use fixtures::*;
