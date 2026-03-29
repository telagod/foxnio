//! 模拟 Redis 连接池用于测试

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 模拟 Redis 存储
#[derive(Debug, Default)]
struct RedisStore {
    data: HashMap<String, (String, Option<i64>)>, // key -> (value, expiration_timestamp_ms)
}

impl RedisStore {
    fn cleanup_expired(&mut self) {
        let now = chrono::Utc::now().timestamp_millis();
        self.data
            .retain(|_, (_, exp)| exp.map(|e| e > now).unwrap_or(true));
    }
}

/// 模拟 Redis 连接池
#[derive(Debug, Clone)]
pub struct MockRedisPool {
    store: Arc<Mutex<RedisStore>>,
}

impl Default for MockRedisPool {
    fn default() -> Self {
        Self {
            store: Arc::new(Mutex::new(RedisStore::default())),
        }
    }
}

impl MockRedisPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置键值（带 TTL）
    pub async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> anyhow::Result<()> {
        let mut store = self.store.lock().unwrap();
        let exp = ttl.map(|d| chrono::Utc::now().timestamp_millis() + d.as_millis() as i64);
        store.data.insert(key.to_string(), (value.to_string(), exp));
        Ok(())
    }

    /// 获取键值
    pub async fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut store = self.store.lock().unwrap();
        store.cleanup_expired();
        Ok(store.data.get(key).map(|(v, _)| v.clone()))
    }

    /// 删除键
    pub async fn del(&self, key: &str) -> anyhow::Result<bool> {
        let mut store = self.store.lock().unwrap();
        Ok(store.data.remove(key).is_some())
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let mut store = self.store.lock().unwrap();
        store.cleanup_expired();
        Ok(store.data.contains_key(key))
    }

    /// 设置过期时间
    pub async fn expire(&self, key: &str, ttl: Duration) -> anyhow::Result<bool> {
        let mut store = self.store.lock().unwrap();
        if let Some((_, ref mut exp)) = store.data.get_mut(key) {
            *exp = Some(chrono::Utc::now().timestamp_millis() + ttl.as_millis() as i64);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取剩余过期时间
    pub async fn ttl(&self, key: &str) -> anyhow::Result<i64> {
        let store = self.store.lock().unwrap();
        if let Some((_, exp)) = store.data.get(key) {
            if let Some(e) = exp {
                let remaining_ms = e - chrono::Utc::now().timestamp_millis();
                // 返回秒数（向上取整）
                let remaining_secs = (remaining_ms + 999) / 1000;
                Ok(remaining_secs.max(0))
            } else {
                Ok(-1) // 无过期时间
            }
        } else {
            Ok(-2) // 键不存在
        }
    }

    /// 清除所有数据
    pub async fn clear(&self) {
        let mut store = self.store.lock().unwrap();
        store.data.clear();
    }

    /// 获取存储的键数量
    pub async fn len(&self) -> usize {
        let store = self.store.lock().unwrap();
        store.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let redis = MockRedisPool::new();

        redis.set("key1", "value1", None).await.unwrap();
        let value = redis.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_nonexistent_key() {
        let redis = MockRedisPool::new();

        let value = redis.get("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let redis = MockRedisPool::new();

        redis.set("key1", "value1", None).await.unwrap();
        let deleted = redis.del("key1").await.unwrap();
        assert!(deleted);

        let value = redis.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_exists() {
        let redis = MockRedisPool::new();

        assert!(!redis.exists("key1").await.unwrap());
        redis.set("key1", "value1", None).await.unwrap();
        assert!(redis.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_expiration() {
        let redis = MockRedisPool::new();

        // 设置 1 秒过期
        redis
            .set("key1", "value1", Some(Duration::from_millis(100)))
            .await
            .unwrap();
        assert!(redis.exists("key1").await.unwrap());

        // 等待过期
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!redis.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_expire() {
        let redis = MockRedisPool::new();

        redis.set("key1", "value1", None).await.unwrap();
        redis
            .expire("key1", Duration::from_millis(100))
            .await
            .unwrap();

        assert!(redis.exists("key1").await.unwrap());
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!redis.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_ttl() {
        let redis = MockRedisPool::new();

        // 不存在的键
        let ttl = redis.ttl("nonexistent").await.unwrap();
        assert_eq!(ttl, -2);

        // 无过期时间的键
        redis.set("key1", "value1", None).await.unwrap();
        let ttl = redis.ttl("key1").await.unwrap();
        assert_eq!(ttl, -1);

        // 有过期时间的键
        redis
            .set("key2", "value2", Some(Duration::from_secs(100)))
            .await
            .unwrap();
        let ttl = redis.ttl("key2").await.unwrap();
        assert!(ttl > 90 && ttl <= 100);
    }
}
