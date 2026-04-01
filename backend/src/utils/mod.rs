pub mod crypto;
pub mod encryption;
pub mod encryption_global;
pub mod id;
pub mod logger;
pub mod metrics;
pub mod test;
pub mod time;
pub mod uuid_conv;
pub mod validator;

// 重导出常用类型

// 请求 ID 生成
pub fn request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// 重导出加密相关
pub use encryption::{EncryptedString, EncryptionService};
pub use encryption_global::{get_encryption_service, init_encryption_service};
