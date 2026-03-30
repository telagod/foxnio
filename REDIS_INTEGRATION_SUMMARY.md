# Redis 集成和批量 SQL 查询实现总结

## 实现概述

为 `backend/src/service/window_cost_cache.rs` 添加了完整的 Redis 集成和批量 SQL 查询功能，显著提升了窗口期费用缓存的性能和可扩展性。

## 核心功能

### 1. 多级缓存架构

- **内存缓存（L1）**: 使用 `Arc<RwLock<HashMap>>` 实现快速本地缓存
- **Redis 缓存（L2）**: 可选的分布式缓存层，支持跨实例共享
- **降级策略**: Redis 不可用时自动降级到内存缓存

### 2. 批量 SQL 查询

#### `prefetch_window_costs()`
- **单次查询**: 批量查询多个账户的窗口期费用
- **聚合计算**: 在数据库层面完成 SUM 和 COUNT 操作
- **自动缓存**: 查询结果自动写入 Redis
- **性能优化**: 减少数据库往返次数，从 N 次查询降到 1 次

**SQL 查询示例**:
```sql
SELECT 
    account_id,
    SUM(amount) as total_cost,
    SUM(tokens_in) as total_tokens_in,
    SUM(tokens_out) as total_tokens_out,
    COUNT(id) as total_requests
FROM quota_usage_history
WHERE account_id IN (1, 2, 3, ...)
  AND created_at > NOW() - INTERVAL '1 hour'
GROUP BY account_id
```

### 3. 智能缓存获取

#### `get_or_fetch_window_costs()`
- **缓存优先**: 先尝试从 Redis 获取
- **批量回填**: 未命中的账户批量查询数据库
- **自动预热**: 查询结果自动写入缓存

### 4. Prometheus 监控指标

新增 6 个监控指标：

| 指标名称 | 类型 | 说明 |
|---------|------|------|
| `foxnio_window_cache_hits_total` | Counter | 总缓存命中次数（内存+Redis） |
| `foxnio_window_cache_misses_total` | Counter | 缓存未命中次数 |
| `foxnio_window_batch_queries_total` | Counter | 批量 SQL 查询次数 |
| `foxnio_window_redis_hits_total` | Counter | Redis 缓存命中次数 |
| `foxnio_window_redis_misses_total` | Counter | Redis 缓存未命中次数 |
| `foxnio_window_prefetched_accounts` | Gauge | 当前预取的账户数量 |

## 数据结构

### WindowCostData
```rust
pub struct WindowCostData {
    pub account_id: i64,
    pub cost: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub requests: i64,
}
```

### Redis 缓存格式
```
key: window_cost:{account_id}
value: {cost}|{total_tokens}|{requests}
TTL: 60 seconds
```

## API 设计

### 主要方法

#### 1. 构造函数
```rust
// 基础版本（仅内存缓存）
let cache = WindowCostCache::new(Duration::minutes(5));

// 带 Redis 支持的版本
let cache = WindowCostCache::with_redis(
    Duration::minutes(5),
    redis_pool
);
```

#### 2. 单账户查询
```rust
// 获取窗口费用
if let Some((cost, tokens, requests)) = cache.get_window_cost("key").await {
    // 缓存命中
}

// 设置窗口费用
cache.set_window_cost("key".to_string(), 10.5, 1000, 5).await;
```

#### 3. 批量操作
```rust
// 批量预取（从数据库）
let costs = cache.prefetch_window_costs(&db, &[1, 2, 3]).await?;

// 智能获取（优先缓存，未命中则查询数据库）
let costs = cache.get_or_fetch_window_costs(&db, &[1, 2, 3]).await?;

// 从 Redis 获取缓存
if let Some(data) = cache.get_cached(&redis, account_id).await? {
    // 缓存命中
}
```

#### 4. 维护操作
```rust
// 清理过期缓存
cache.cleanup_expired().await;

// 清空缓存
cache.clear().await;

// 获取统计信息
let stats = cache.stats().await;
```

## 性能优化

### 1. 数据库优化
- **批量查询**: 从 N 次查询优化到 1 次
- **索引利用**: `account_id` 和 `created_at` 字段应建立复合索引
- **聚合下推**: 在数据库层面完成聚合计算

### 2. 缓存优化
- **多级缓存**: L1 内存 + L2 Redis
- **TTL 控制**: 60 秒过期，平衡实时性和性能
- **自动回填**: 缓存未命中时自动填充

### 3. 并发优化
- **读写锁**: 使用 `RwLock` 支持高并发读取
- **异步 IO**: 全异步实现，不阻塞线程
- **连接池**: Redis 使用连接管理器

## 使用示例

```rust
use std::sync::Arc;
use crate::db::redis::{RedisPool, RedisConfig};
use crate::service::window_cost_cache::WindowCostCache;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 Redis
    let redis_config = RedisConfig::default();
    let redis_pool = Arc::new(RedisPool::new(&redis_config)?);
    
    // 创建缓存
    let cache = WindowCostCache::with_redis(
        Duration::minutes(5),
        redis_pool.clone()
    );
    
    // 场景 1: 批量预取（适合批量处理）
    let account_ids = vec![1, 2, 3, 4, 5];
    let costs = cache.prefetch_window_costs(&db, &account_ids).await?;
    
    // 场景 2: 智能获取（适合单个请求）
    let costs = cache.get_or_fetch_window_costs(&db, &[1, 2, 3]).await?;
    
    // 场景 3: 单账户查询
    let key = "window_cost:123";
    if let Some((cost, tokens, requests)) = cache.get_window_cost(key).await {
        println!("Cost: {}, Tokens: {}, Requests: {}", cost, tokens, requests);
    }
    
    Ok(())
}
```

## 监控和可观测性

### Grafana 仪表板指标

```promql
# 缓存命中率
rate(foxnio_window_cache_hits_total[5m]) 
  / 
(rate(foxnio_window_cache_hits_total[5m]) + rate(foxnio_window_cache_misses_total[5m]))

# Redis 缓存命中率
rate(foxnio_window_redis_hits_total[5m]) 
  / 
(rate(foxnio_window_redis_hits_total[5m]) + rate(foxnio_window_redis_misses_total[5m]))

# 批量查询速率
rate(foxnio_window_batch_queries_total[5m])

# 当前预取账户数
foxnio_window_prefetched_accounts
```

### 告警规则建议

```yaml
# 缓存命中率过低
- alert: WindowCacheHitRateLow
  expr: |
    rate(foxnio_window_cache_hits_total[5m]) 
      / 
    (rate(foxnio_window_cache_hits_total[5m]) + rate(foxnio_window_cache_misses_total[5m])) 
      < 0.5
  for: 5m
  annotations:
    summary: "窗口期费用缓存命中率低于 50%"

# 批量查询频率过高
- alert: WindowBatchQueriesHigh
  expr: rate(foxnio_window_batch_queries_total[1m]) > 10
  for: 2m
  annotations:
    summary: "窗口期批量查询频率过高，可能需要调整缓存策略"
```

## 测试覆盖

已添加单元测试：
- ✅ 缓存创建和配置
- ✅ 设置和获取操作
- ✅ 过期清理
- ✅ 统计信息
- ✅ 数据解析

## 后续优化建议

1. **缓存预热**: 应用启动时预加载热点账户数据
2. **自适应 TTL**: 根据访问频率动态调整过期时间
3. **批量大小控制**: 限制单次批量查询的账户数量
4. **错误重试**: 添加数据库查询失败时的重试机制
5. **缓存穿透保护**: 对不存在的账户进行空值缓存
6. **分布式锁**: 批量查询时加锁防止缓存击穿

## 兼容性

- ✅ 向后兼容：无 Redis 时自动降级到内存缓存
- ✅ 渐进式迁移：可逐步启用 Redis 功能
- ✅ 配置灵活：通过构造函数控制是否启用 Redis

## 依赖项

所有依赖已在 `Cargo.toml` 中存在：
- `redis` v0.25 (features: tokio-comp, connection-manager)
- `sea-orm` v0.12
- `prometheus` v0.13
- `lazy_static` v1.4
- `tokio` v1.36
- `chrono` v0.4

## 文件变更

- **修改**: `backend/src/service/window_cost_cache.rs`
  - 新增 Redis 集成
  - 新增批量 SQL 查询
  - 新增 6 个 Prometheus 指标
  - 新增 3 个主要方法
  - 新增单元测试
