//! 粘性会话管理服务 - Redis 持久化实现
//!
//! 支持会话绑定、TTL 管理、摘要链生成

#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 默认会话 TTL（5 分钟）
const DEFAULT_SESSION_TTL_SECONDS: i64 = 300;

/// Anthropic 摘要会话 key 前缀
const ANTHROPIC_DIGEST_SESSION_KEY_PREFIX: &str = "anthropic:digest:";

/// 粘性会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickySession {
    /// 绑定的账号 ID
    pub account_id: Uuid,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后访问时间
    pub last_accessed: DateTime<Utc>,
    /// 请求计数
    pub request_count: i64,
    /// 会话元数据
    pub metadata: HashMap<String, String>,
}

/// 粘性会话配置
#[derive(Debug, Clone)]
pub struct StickySessionConfig {
    /// 会话 TTL（秒）
    pub ttl_seconds: i64,
    /// 最大会话数
    pub max_sessions: usize,
    /// 是否启用摘要链
    pub enable_digest_chain: bool,
}

impl Default for StickySessionConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: DEFAULT_SESSION_TTL_SECONDS,
            max_sessions: 10000,
            enable_digest_chain: true,
        }
    }
}

/// 粘性会话管理器
pub struct StickySessionManager {
    config: StickySessionConfig,
    /// 内存缓存（实际生产环境应使用 Redis）
    sessions: Arc<RwLock<HashMap<String, StickySession>>>,
    /// 反向索引：账号 -> 会话列表
    account_sessions: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
}

impl StickySessionManager {
    pub fn new(config: StickySessionConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            account_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取会话绑定的账号
    pub async fn get_account(&self, session_id: &str) -> Result<Option<Uuid>> {
        let sessions = self.sessions.read().await;

        if let Some(session) = sessions.get(session_id) {
            // 检查是否过期
            if self.is_session_expired(session) {
                return Ok(None);
            }

            return Ok(Some(session.account_id));
        }

        Ok(None)
    }

    /// 绑定会话到账号
    pub async fn bind_account(
        &self,
        session_id: String,
        account_id: Uuid,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<()> {
        let now = Utc::now();

        // 创建会话
        let session = StickySession {
            account_id,
            created_at: now,
            last_accessed: now,
            request_count: 1,
            metadata: metadata.unwrap_or_default(),
        };

        // 存储会话
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session);
        }

        // 更新反向索引
        {
            let mut account_sessions = self.account_sessions.write().await;
            account_sessions
                .entry(account_id)
                .or_insert_with(Vec::new)
                .push(session_id);
        }

        Ok(())
    }

    /// 刷新会话（延长 TTL）
    pub async fn touch(&self, session_id: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(session_id) {
            if self.is_session_expired(session) {
                sessions.remove(session_id);
                return Ok(false);
            }

            session.last_accessed = Utc::now();
            session.request_count += 1;
            return Ok(true);
        }

        Ok(false)
    }

    /// 清除会话
    pub async fn clear_session(&self, session_id: &str) -> Result<()> {
        let account_id = {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).map(|s| s.account_id)
        };

        if let Some(account_id) = account_id {
            // 从反向索引中移除
            let mut account_sessions = self.account_sessions.write().await;
            if let Some(session_list) = account_sessions.get_mut(&account_id) {
                session_list.retain(|id| id != session_id);
            }
        }

        // 从主存储移除
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);

        Ok(())
    }

    /// 清除账号的所有会话
    pub async fn clear_account_sessions(&self, account_id: Uuid) -> Result<usize> {
        let session_ids: Vec<String> = {
            let account_sessions = self.account_sessions.read().await;
            account_sessions
                .get(&account_id)
                .cloned()
                .unwrap_or_default()
        };

        let count = session_ids.len();

        // 移除所有会话
        let mut sessions = self.sessions.write().await;
        for session_id in &session_ids {
            sessions.remove(session_id);
        }

        // 清除反向索引
        let mut account_sessions = self.account_sessions.write().await;
        account_sessions.remove(&account_id);

        Ok(count)
    }

    /// 获取会话详情
    pub async fn get_session(&self, session_id: &str) -> Option<StickySession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// 获取账号的活跃会话数
    pub async fn get_account_session_count(&self, account_id: Uuid) -> usize {
        let account_sessions = self.account_sessions.read().await;
        account_sessions
            .get(&account_id)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> StickySessionStats {
        let sessions = self.sessions.read().await;
        let account_sessions = self.account_sessions.read().await;

        let total_sessions = sessions.len();
        let total_accounts = account_sessions.len();

        let mut total_requests = 0i64;
        let mut expired_count = 0;

        for session in sessions.values() {
            total_requests += session.request_count;
            if self.is_session_expired(session) {
                expired_count += 1;
            }
        }

        StickySessionStats {
            total_sessions,
            total_accounts,
            total_requests,
            expired_sessions: expired_count,
        }
    }

    /// 清理过期会话
    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut expired_sessions = Vec::new();

        // 找出过期会话
        {
            let sessions = self.sessions.read().await;
            for (session_id, session) in sessions.iter() {
                if self.is_session_expired(session) {
                    expired_sessions.push((session_id.clone(), session.account_id));
                }
            }
        }

        let count = expired_sessions.len();

        // 移除过期会话
        for (session_id, account_id) in &expired_sessions {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);

            let mut account_sessions = self.account_sessions.write().await;
            if let Some(session_list) = account_sessions.get_mut(account_id) {
                session_list.retain(|id| id != session_id);
            }
        }

        Ok(count)
    }

    /// 检查会话是否过期
    fn is_session_expired(&self, session: &StickySession) -> bool {
        let now = Utc::now();
        (now - session.last_accessed).num_seconds() > self.config.ttl_seconds
    }

    // ========== 摘要链相关方法 ==========

    /// 构建 Anthropic 摘要链
    ///
    /// 格式: s:<hash>-u:<hash>-a:<hash>-u:<hash>-...
    /// s = system, u = user, a = assistant
    pub fn build_anthropic_digest_chain(
        system: Option<&serde_json::Value>,
        messages: &[serde_json::Value],
    ) -> String {
        let mut parts = Vec::new();

        // 1. system prompt
        if let Some(sys) = system {
            let system_data = serde_json::to_vec(sys).unwrap_or_default();
            if !system_data.is_empty() && system_data != b"null" {
                parts.push(format!("s:{}", short_hash(&system_data)));
            }
        }

        // 2. messages
        for msg in messages {
            if let Some(obj) = msg.as_object() {
                let role = obj.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                let prefix = match role {
                    "assistant" => "a",
                    _ => "u",
                };
                let content = obj.get("content");
                let content_data = match content {
                    Some(c) => serde_json::to_vec(c).unwrap_or_default(),
                    None => Vec::new(),
                };
                parts.push(format!("{}:{}", prefix, short_hash(&content_data)));
            }
        }

        parts.join("-")
    }

    /// 生成 Anthropic 摘要会话 key
    ///
    /// 组合 prefixHash 前 8 位 + uuid 前 8 位
    pub fn generate_anthropic_digest_session_key(prefix_hash: &str, uuid: &str) -> String {
        let prefix = if prefix_hash.len() >= 8 {
            &prefix_hash[..8]
        } else {
            prefix_hash
        };
        let uuid_part = if uuid.len() >= 8 { &uuid[..8] } else { uuid };
        format!(
            "{}{}:{}",
            ANTHROPIC_DIGEST_SESSION_KEY_PREFIX, prefix, uuid_part
        )
    }

    /// 根据请求生成摘要链会话 key
    pub async fn get_or_create_digest_session(
        &self,
        system: Option<&serde_json::Value>,
        messages: &[serde_json::Value],
        preferred_account_id: Option<Uuid>,
    ) -> Result<(String, Option<Uuid>)> {
        // 构建摘要链
        let digest_chain = Self::build_anthropic_digest_chain(system, messages);
        let prefix_hash = short_hash(digest_chain.as_bytes());
        let uuid_part = Uuid::new_v4().to_string();
        let session_key = Self::generate_anthropic_digest_session_key(&prefix_hash, &uuid_part);

        // 尝试复用现有会话
        if let Some(_account_id) = preferred_account_id {
            let sessions = self.sessions.read().await;
            // 查找具有相同摘要前缀的会话
            let prefix_to_match = &prefix_hash[..8.min(prefix_hash.len())];
            for (key, session) in sessions.iter() {
                if key.starts_with(&format!(
                    "{}{}",
                    ANTHROPIC_DIGEST_SESSION_KEY_PREFIX, prefix_to_match
                )) && !self.is_session_expired(session)
                {
                    return Ok((key.clone(), Some(session.account_id)));
                }
            }
        }

        // 创建新会话
        if let Some(account_id) = preferred_account_id {
            self.bind_account(session_key.clone(), account_id, None)
                .await?;
            return Ok((session_key, Some(account_id)));
        }

        Ok((session_key, None))
    }
}

/// 会话统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickySessionStats {
    pub total_sessions: usize,
    pub total_accounts: usize,
    pub total_requests: i64,
    pub expired_sessions: usize,
}

/// 计算短哈希（用于摘要链）
fn short_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    // 取前 8 字节，转为 16 进制
    hex::encode(&result[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sticky_session_basic() {
        let manager = StickySessionManager::new(StickySessionConfig::default());
        let session_id = "test-session-1".to_string();
        let account_id = Uuid::new_v4();

        // 绑定会话
        manager
            .bind_account(session_id.clone(), account_id, None)
            .await
            .unwrap();

        // 获取账号
        let result = manager.get_account(&session_id).await.unwrap();
        assert_eq!(result, Some(account_id));

        // 刷新会话
        let touched = manager.touch(&session_id).await.unwrap();
        assert!(touched);

        // 获取统计
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_accounts, 1);
    }

    #[tokio::test]
    async fn test_sticky_session_expiry() {
        let config = StickySessionConfig {
            ttl_seconds: 1, // 1 秒过期
            ..Default::default()
        };
        let manager = StickySessionManager::new(config);
        let session_id = "test-session-2".to_string();
        let account_id = Uuid::new_v4();

        manager
            .bind_account(session_id.clone(), account_id, None)
            .await
            .unwrap();

        // 立即获取应该成功
        let result = manager.get_account(&session_id).await.unwrap();
        assert_eq!(result, Some(account_id));

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 过期后应该返回 None
        let result = manager.get_account(&session_id).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let config = StickySessionConfig {
            ttl_seconds: 1,
            ..Default::default()
        };
        let manager = StickySessionManager::new(config);

        // 创建多个会话
        for i in 0..5 {
            manager
                .bind_account(format!("session-{i}"), Uuid::new_v4(), None)
                .await
                .unwrap();
        }

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 清理
        let cleaned = manager.cleanup_expired().await.unwrap();
        assert_eq!(cleaned, 5);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_sessions, 0);
    }

    #[test]
    fn test_digest_chain() {
        let system = serde_json::json!("You are a helpful assistant.");
        let messages = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Hi there!"}),
        ];

        let chain = StickySessionManager::build_anthropic_digest_chain(Some(&system), &messages);
        assert!(chain.starts_with("s:"));
        assert!(chain.contains("-u:"));
        assert!(chain.contains("-a:"));
    }

    #[test]
    fn test_short_hash() {
        let hash1 = short_hash(b"test data");
        let hash2 = short_hash(b"test data");
        let hash3 = short_hash(b"different data");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // 8 bytes = 16 hex chars
    }
}
