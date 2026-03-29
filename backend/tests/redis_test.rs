//! Redis 集成测试

use redis::AsyncCommands;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要实际 Redis 连接
    async fn test_redis_connection() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        // 测试 PING
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .expect("Failed to ping");

        assert_eq!(result, "PONG");
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_string_operations() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:string:key";
        let value = "test_value";

        // SET
        let result: String = conn.set(key, value).await.expect("Failed to set");
        assert_eq!(result, "OK");

        // GET
        let result: Option<String> = conn.get(key).await.expect("Failed to get");
        assert_eq!(result, Some(value.to_string()));

        // DEL
        let result: i32 = conn.del(key).await.expect("Failed to del");
        assert_eq!(result, 1);

        // GET after DEL
        let result: Option<String> = conn.get(key).await.expect("Failed to get");
        assert_eq!(result, None);
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_ttl_operations() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:ttl:key";

        // SET with EX
        let result: String = conn
            .set_ex(key, "value", 60)
            .await
            .expect("Failed to setex");
        assert_eq!(result, "OK");

        // TTL
        let ttl: i64 = conn.ttl(key).await.expect("Failed to get ttl");
        assert!(ttl > 0 && ttl <= 60);

        // 清理
        let _: () = conn.del(key).await.unwrap_or(());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_counter_operations() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:counter:key";

        // INCR
        let result: i64 = conn.incr(key, 1).await.expect("Failed to incr");
        assert_eq!(result, 1);

        // INCR again
        let result: i64 = conn.incr(key, 1).await.expect("Failed to incr");
        assert_eq!(result, 2);

        // DECR
        let result: i64 = conn.decr(key, 1).await.expect("Failed to decr");
        assert_eq!(result, 1);

        // 清理
        let _: () = conn.del(key).await.unwrap_or(());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_hash_operations() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:hash:key";

        // HSET
        let result: bool = conn
            .hset(key, "field1", "value1")
            .await
            .expect("Failed to hset");
        assert!(result);

        // HGET
        let result: Option<String> = conn.hget(key, "field1").await.expect("Failed to hget");
        assert_eq!(result, Some("value1".to_string()));

        // HSET multiple
        let result: i32 = conn
            .hset_multiple(key, &[("field2", "value2"), ("field3", "value3")])
            .await
            .expect("Failed to hset multiple");
        assert_eq!(result, 2);

        // HGETALL
        let result: std::collections::HashMap<String, String> =
            conn.hgetall(key).await.expect("Failed to hgetall");
        assert_eq!(result.len(), 3);

        // 清理
        let _: () = conn.del(key).await.unwrap_or(());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_set_operations() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:set:key";

        // SADD
        let result: i32 = conn.sadd(key, "member1").await.expect("Failed to sadd");
        assert_eq!(result, 1);

        // SISMEMBER
        let result: bool = conn
            .sismember(key, "member1")
            .await
            .expect("Failed to sismember");
        assert!(result);

        // SISMEMBER (not exists)
        let result: bool = conn
            .sismember(key, "member2")
            .await
            .expect("Failed to sismember");
        assert!(!result);

        // 清理
        let _: () = conn.del(key).await.unwrap_or(());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_rate_limit_pattern() {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        let client = redis::Client::open(redis_url.as_str()).expect("Failed to create client");
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .expect("Failed to connect");

        let key = "test:ratelimit:user:123";
        let limit = 5;
        let window = 60;

        // 模拟速率限制
        for i in 0..limit + 1 {
            let count: i64 = conn.incr(key, 1).await.expect("Failed to incr");

            if i == 0 {
                // 第一次设置过期时间
                let _: () = conn.expire(key, window).await.expect("Failed to expire");
            }

            if i < limit {
                assert!(count <= limit as i64);
            } else {
                assert!(count > limit as i64);
            }
        }

        // 清理
        let _: () = conn.del(key).await.unwrap_or(());
    }
}
