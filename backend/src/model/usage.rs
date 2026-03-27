//! 用量记录模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub account_id: Uuid,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: i64, // 单位：分
    pub created_at: DateTime<Utc>,
}
