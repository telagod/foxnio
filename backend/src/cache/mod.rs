//! 本地内存缓存模块
//!
//! 提供基于 LRU (Least Recently Used) 算法的线程安全缓存实现
//! 支持 TTL (Time To Live) 和缓存统计功能

use chrono::{DateTime, Utc};
use lru::LruCache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 缓存条目，包含值和过期时间
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    /// 缓存的值
    value: V,
    /// 过期时间（None 表示永不过期）
    expires_at: Option<DateTime<Utc>>,
}

impl<V> CacheEntry<V> {
    /// 创建新的缓存条目
    fn new(value: V, ttl: Option<Duration>) -> Self {
        let expires_at =
            ttl.map(|duration| Utc::now() + chrono::Duration::from_std(duration).unwrap());
        Self { value, expires_at }
    }

    /// 检查条目是否已过期
    fn is_expired(&self) -> bool {
        self.expires_at
            .map(|expires_at| Utc::now() > expires_at)
            .unwrap_or(false)
    }
}

/// 缓存统计信息
#[derive(Debug, Default)]
pub struct CacheStats {
    /// 命中次数
    hit_count: AtomicU64,
    /// 未命中次数
    miss_count: AtomicU64,
}

impl CacheStats {
    /// 创建新的统计实例
    fn new() -> Self {
        Self {
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
        }
    }

    /// 记录一次命中
    fn record_hit(&self) {
        self.hit_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一次未命中
    fn record_miss(&self) {
        self.miss_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取命中次数
    pub fn hit_count(&self) -> u64 {
        self.hit_count.load(Ordering::Relaxed)
    }

    /// 获取未命中次数
    pub fn miss_count(&self) -> u64 {
        self.miss_count.load(Ordering::Relaxed)
    }

    /// 计算命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count() + self.miss_count();
        if total == 0 {
            0.0
        } else {
            self.hit_count() as f64 / total as f64
        }
    }

    /// 重置统计信息
    fn reset(&self) {
        self.hit_count.store(0, Ordering::Relaxed);
        self.miss_count.store(0, Ordering::Relaxed);
    }
}

/// 线程安全的 LRU 缓存
pub struct Cache<K, V> {
    /// LRU 缓存实例
    inner: Arc<RwLock<LruCache<K, CacheEntry<V>>>>,
    /// 默认 TTL
    default_ttl: Option<Duration>,
    /// 缓存统计
    stats: Arc<CacheStats>,
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            default_ttl: self.default_ttl,
            stats: Arc::clone(&self.stats),
        }
    }
}

impl<K, V> Cache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建新的缓存实例
    ///
    /// # 参数
    /// - `size`: 缓存最大容量
    ///
    /// # 示例
    /// ```
    /// use foxnio::cache::Cache;
    /// let cache: Cache<String, String> = Cache::new(100);
    /// ```
    pub fn new(size: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(size).expect("Cache size must be non-zero"),
            ))),
            default_ttl: None,
            stats: Arc::new(CacheStats::new()),
        }
    }

    /// 创建带有默认 TTL 的缓存实例
    ///
    /// # 参数
    /// - `size`: 缓存最大容量
    /// - `ttl`: 默认的生存时间
    ///
    /// # 示例
    /// ```
    /// use foxnio::cache::Cache;
    /// use std::time::Duration;
    /// let cache: Cache<String, String> = Cache::with_ttl(100, Duration::from_secs(60));
    /// ```
    pub fn with_ttl(size: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(size).expect("Cache size must be non-zero"),
            ))),
            default_ttl: Some(ttl),
            stats: Arc::new(CacheStats::new()),
        }
    }

    /// 获取缓存中的值
    ///
    /// 如果键不存在或已过期，返回 None
    ///
    /// # 参数
    /// - `key`: 键的引用
    ///
    /// # 返回
    /// - `Some(V)`: 存在且未过期的值
    /// - `None`: 不存在或已过期
    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.inner.write().unwrap();

        if let Some(entry) = cache.get(key) {
            if entry.is_expired() {
                cache.pop(key);
                self.stats.record_miss();
                return None;
            }
            self.stats.record_hit();
            Some(entry.value.clone())
        } else {
            self.stats.record_miss();
            None
        }
    }

    /// 向缓存中插入键值对
    ///
    /// 使用缓存实例的默认 TTL
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 值
    pub fn put(&self, key: K, value: V) {
        let entry = CacheEntry::new(value, self.default_ttl);
        self.inner.write().unwrap().put(key, entry);
    }

    /// 向缓存中插入键值对（带自定义 TTL）
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 值
    /// - `ttl`: 生存时间
    pub fn put_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let entry = CacheEntry::new(value, Some(ttl));
        self.inner.write().unwrap().put(key, entry);
    }

    /// 从缓存中移除键值对
    ///
    /// # 参数
    /// - `key`: 键的引用
    ///
    /// # 返回
    /// - `Some(V)`: 被移除的值
    /// - `None`: 键不存在
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner
            .write()
            .unwrap()
            .pop(key)
            .map(|entry| entry.value)
    }

    /// 清空缓存
    pub fn clear(&self) {
        self.inner.write().unwrap().clear();
    }

    /// 获取缓存中的条目数量
    ///
    /// 注意：此数量包括已过期但尚未被访问的条目
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// 检查缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.read().unwrap().is_empty()
    }

    /// 清理所有已过期的条目
    ///
    /// # 返回
    /// 清理的条目数量
    pub fn cleanup_expired(&self) -> usize {
        let mut cache = self.inner.write().unwrap();
        let expired_keys: Vec<K> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            cache.pop(&key);
        }
        count
    }

    /// 获取缓存统计信息的快照
    pub fn stats(&self) -> CacheStatsSnapshot {
        CacheStatsSnapshot {
            hit_count: self.stats.hit_count(),
            miss_count: self.stats.miss_count(),
            hit_rate: self.stats.hit_rate(),
        }
    }

    /// 重置缓存统计信息
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// 检查键是否存在且未过期
    ///
    /// # 参数
    /// - `key`: 键的引用
    ///
    /// # 返回
    /// - `true`: 键存在且未过期
    /// - `false`: 键不存在或已过期
    pub fn contains_key(&self, key: &K) -> bool {
        let mut cache = self.inner.write().unwrap();
        if let Some(entry) = cache.peek(key) {
            if entry.is_expired() {
                cache.pop(key);
                return false;
            }
            true
        } else {
            false
        }
    }
}

/// 缓存统计信息的快照
#[derive(Debug, Clone, Copy)]
pub struct CacheStatsSnapshot {
    /// 命中次数
    pub hit_count: u64,
    /// 未命中次数
    pub miss_count: u64,
    /// 命中率
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_operations() {
        let cache: Cache<String, String> = Cache::new(3);

        // 测试插入和获取
        cache.put("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.len(), 1);

        // 测试移除
        let removed = cache.remove(&"key1".to_string());
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_lru_eviction() {
        let cache: Cache<String, String> = Cache::new(2);

        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());
        cache.put("key3".to_string(), "value3".to_string());

        // key1 应该被驱逐
        assert_eq!(cache.get(&"key1".to_string()), None);
        assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
        assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
    }

    #[test]
    fn test_ttl_expiration() {
        let cache: Cache<String, String> = Cache::with_ttl(10, Duration::from_millis(50));

        cache.put("key1".to_string(), "value1".to_string());

        // 立即获取应该成功
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        // 等待过期
        thread::sleep(Duration::from_millis(100));

        // 过期后应该返回 None
        assert_eq!(cache.get(&"key1".to_string()), None);
    }

    #[test]
    fn test_custom_ttl() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put_with_ttl(
            "key1".to_string(),
            "value1".to_string(),
            Duration::from_millis(50),
        );

        // 立即获取应该成功
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        // 等待过期
        thread::sleep(Duration::from_millis(100));

        // 过期后应该返回 None
        assert_eq!(cache.get(&"key1".to_string()), None);
    }

    #[test]
    fn test_cache_stats() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put("key1".to_string(), "value1".to_string());

        // 命中
        cache.get(&"key1".to_string());
        cache.get(&"key1".to_string());

        // 未命中
        cache.get(&"key2".to_string());

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate - 0.666_666_666_666_666_6).abs() < 0.0001);
    }

    #[test]
    fn test_clear() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put("key1".to_string(), "value1".to_string());
        cache.put("key2".to_string(), "value2".to_string());

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cleanup_expired() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put_with_ttl(
            "key1".to_string(),
            "value1".to_string(),
            Duration::from_millis(50),
        );
        cache.put("key2".to_string(), "value2".to_string());

        // 等待第一个键过期
        thread::sleep(Duration::from_millis(100));

        // 清理过期条目
        let cleaned = cache.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_contains_key() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put("key1".to_string(), "value1".to_string());

        assert!(cache.contains_key(&"key1".to_string()));
        assert!(!cache.contains_key(&"key2".to_string()));
    }

    #[test]
    fn test_thread_safety() {
        let cache: Cache<String, i32> = Cache::new(100);
        let cache_clone = cache.clone();
        let cache_clone2 = cache.clone();

        let handle1 = thread::spawn(move || {
            for i in 0..50 {
                cache_clone.put(format!("key{}", i), i);
            }
        });

        let handle2 = thread::spawn(move || {
            for i in 50..100 {
                cache_clone2.put(format!("key{}", i), i);
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Use a new clone to check since the original was moved
        assert_eq!(cache.len(), 100);
    }

    #[test]
    fn test_reset_stats() {
        let cache: Cache<String, String> = Cache::new(10);

        cache.put("key1".to_string(), "value1".to_string());
        cache.get(&"key1".to_string());
        cache.get(&"key2".to_string());

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);

        cache.reset_stats();

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 0);
    }

    #[test]
    fn test_concurrent_read_write() {
        let cache: Cache<String, i32> = Cache::new(100);
        let cache_clone1 = cache.clone();
        let cache_clone2 = cache.clone();

        // 写线程
        let write_handle = thread::spawn(move || {
            for i in 0..50 {
                cache_clone1.put(format!("key{}", i), i);
            }
        });

        // 读线程
        let read_handle = thread::spawn(move || {
            for i in 0..50 {
                let _ = cache_clone2.get(&format!("key{}", i));
            }
        });

        write_handle.join().unwrap();
        read_handle.join().unwrap();

        // 确保所有写入都成功
        assert_eq!(cache.len(), 50);
    }

    #[test]
    fn test_cache_clone() {
        let cache1: Cache<String, String> = Cache::new(10);
        cache1.put("key1".to_string(), "value1".to_string());

        let cache2 = cache1.clone();

        // 两个缓存应该共享相同的数据
        assert_eq!(cache2.get(&"key1".to_string()), Some("value1".to_string()));

        cache2.put("key2".to_string(), "value2".to_string());
        assert_eq!(cache1.get(&"key2".to_string()), Some("value2".to_string()));
    }
}
