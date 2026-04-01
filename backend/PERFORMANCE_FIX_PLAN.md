# FoxNIO 性能优化方案 - 大量号池场景

## 问题总结

### 1. 后端账号列表 API（高优先级）

**当前问题**:
- `list_all()` 无分页，全量返回
- 当账号数 > 1000 时，响应延迟 > 500ms
- 前端一次性渲染大量数据导致卡顿

**优化方案**:

```rust
// service/account.rs - 添加分页支持
pub async fn list_paged(
    &self,
    page: u64,
    per_page: u64,
    status: Option<&str>,
    provider: Option<&str>,
) -> Result<(Vec<AccountInfo>, u64)> {
    let mut query = accounts::Entity::find();
    
    if let Some(s) = status {
        query = query.filter(accounts::Column::Status.eq(s));
    }
    if let Some(p) = provider {
        query = query.filter(accounts::Column::Provider.eq(p));
    }
    
    // 先获取总数
    let total = query.clone().count(&self.db).await?;
    
    // 分页查询
    let accounts = query
        .order_by_desc(accounts::Column::Priority)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&self.db)
        .await?;
    
    Ok((accounts.into_iter().map(...).collect(), total))
}
```

**Handler 更新**:

```rust
// handler/admin.rs
pub async fn list_accounts(
    Extension(state): Extension<SharedState>,
    Query(params): Query<ListAccountsQuery>,
) -> Result<Json<Value>, ApiError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50).min(200); // 限制最大 200
    
    let (accounts, total) = account_service
        .list_paged(page, per_page, params.status.as_deref(), params.provider.as_deref())
        .await?;
    
    Ok(Json(json!({
        "object": "list",
        "data": accounts,
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total": total,
            "total_pages": (total + per_page - 1) / per_page,
        }
    })))
}
```

### 2. 调度器优化（高优先级）

**当前问题**:
- 每次请求遍历所有账号 O(n)
- 每个账号多次异步调用获取指标
- 高并发下调度器成为瓶颈

**优化方案 A - 账号池预热 + 缓存**:

```rust
// gateway/scheduler/mod.rs
pub struct Scheduler {
    // 新增：预计算的候选账号列表
    candidate_cache: RwLock<Vec<AccountInfo>>,
    // 新增：上次更新时间
    cache_updated_at: RwLock<Instant>,
    // 新增：按 Provider 分组的账号
    accounts_by_provider: RwLock<HashMap<String, Vec<AccountInfo>>>,
}

impl Scheduler {
    /// 后台任务：每 5 秒更新候选账号缓存
    pub async fn refresh_candidate_cache(&self) {
        let accounts = self.accounts.read().await;
        
        // 按 provider 分组
        let mut by_provider: HashMap<String, Vec<AccountInfo>> = HashMap::new();
        for account in accounts.iter() {
            by_provider
                .entry(account.provider.clone())
                .or_default()
                .push(account.clone());
        }
        
        *self.accounts_by_provider.write().await = by_provider;
        *self.candidate_cache.write().await = accounts.clone();
        *self.cache_updated_at.write().await = Instant::now();
    }
    
    /// 快速选择 - 只从缓存的活跃账号中选择
    pub async fn select_fast(&self, provider: &str) -> Option<AccountInfo> {
        let by_provider = self.accounts_by_provider.read().await;
        let candidates = by_provider.get(provider)?;
        
        // 使用轮询或随机，O(1) 复杂度
        let index = self.round_robin_index.fetch_add(1, Ordering::SeqCst);
        Some(candidates[index % candidates.len()].clone())
    }
}
```

**优化方案 B - 使用堆结构维护最优账号**:

```rust
use std::collections::BinaryHeap;

pub struct PriorityAccountHeap {
    // 按综合分数排序的账号堆
    heap: RwLock<BinaryHeap<ScoredAccount>>,
}

#[derive(Clone, Eq, PartialEq)]
struct ScoredAccount {
    score: i64, // 整数分数，便于比较
    account: AccountInfo,
}

impl Ord for ScoredAccount {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl PriorityAccountHeap {
    /// 获取最优账号 - O(log n)
    pub async fn peek_best(&self) -> Option<AccountInfo> {
        self.heap.read().await.peek().map(|s| s.account.clone())
    }
    
    /// 后台更新分数
    pub async fn update_scores(&self, metrics: &SchedulerMetrics) {
        let mut heap = self.heap.write().await;
        heap.clear();
        
        // 重新计算所有账号分数并插入堆
        for account in self.accounts.read().await.iter() {
            let score = self.calculate_score(account, metrics).await;
            heap.push(ScoredAccount { score, account: account.clone() });
        }
    }
}
```

### 3. 数据库索引优化

**添加 Priority 索引**:

```rust
// migration/src/m20240401_xxx_add_priority_index.rs
manager
    .create_index(
        Index::create()
            .name("idx_accounts_priority")
            .table(Accounts::Table)
            .col(Accounts::Priority)
            .to_owned(),
    )
    .await?;

// 复合索引：status + priority（常用查询）
manager
    .create_index(
        Index::create()
            .name("idx_accounts_status_priority")
            .table(Accounts::Table)
            .col(Accounts::Status)
            .col(Accounts::Priority)
            .to_owned(),
    )
    .await?;
```

### 4. 前端虚拟列表

**当前问题**:
- 一次性渲染所有账号
- DOM 节点过多导致卡顿

**优化方案**:

```svelte
<!-- routes/admin/accounts/+page.svelte -->
<script lang="ts">
  import { VirtualList } from 'svelte-virtual-list';
  
  let accounts = [];
  let page = 1;
  let hasMore = true;
  
  async function loadMore() {
    if (!hasMore) return;
    const res = await fetch(`/api/v1/admin/accounts?page=${page}&per_page=50`);
    const data = await res.json();
    accounts = [...accounts, ...data.data];
    hasMore = data.pagination.page < data.pagination.total_pages;
    page++;
  }
</script>

<VirtualList items={accounts} let:item>
  <AccountRow {item} />
</VirtualList>

{#if hasMore}
  <button on:click={loadMore}>Load More</button>
{/if}
```

### 5. 批量操作优化

**当前问题**:
- 批量导入使用循环插入
- 每次插入都是独立的数据库操作

**优化方案**:

```rust
// service/batch_import.rs
pub async fn fast_import(&self, accounts: Vec<ImportAccountItem>) -> Result<ImportResult> {
    const BATCH_SIZE: usize = 1000;
    
    // 批量插入，使用单条 SQL
    for chunk in accounts.chunks(BATCH_SIZE) {
        let values: Vec<accounts::ActiveModel> = chunk.iter()
            .map(|item| accounts::ActiveModel {
                id: Set(Uuid::new_v4()),
                name: Set(item.name.clone()),
                // ...
                ..Default::default()
            })
            .collect();
        
        // 单条 INSERT 语句插入 1000 条
        accounts::Entity::insert_many(values)
            .exec(&self.db)
            .await?;
    }
    
    Ok(ImportResult { ... })
}
```

## 实施优先级

| 优先级 | 优化项 | 预期效果 | 工作量 |
|--------|--------|----------|--------|
| P0 | 账号列表分页 | 响应时间从 >500ms 降到 <50ms | 2h |
| P0 | 调度器缓存 | 调度延迟从 O(n) 降到 O(1) | 4h |
| P1 | 数据库索引 | 查询性能提升 10x | 0.5h |
| P1 | 前端虚拟列表 | 渲染 10000+ 账号流畅 | 3h |
| P2 | 批量导入优化 | 导入 10000 账号 < 10s | 2h |

## 监控指标

添加 Prometheus 指标监控：

```rust
// 新增指标
lazy_static! {
    pub static ref SCHEDULER_SELECT_DURATION: Histogram = 
        register_histogram!("scheduler_select_duration_seconds", "调度选择耗时").unwrap();
    
    pub static ref ACCOUNT_LIST_QUERY_DURATION: Histogram =
        register_histogram!("account_list_query_duration_seconds", "账号列表查询耗时").unwrap();
    
    pub static ref ACTIVE_ACCOUNTS_COUNT: Gauge =
        register_gauge!("active_accounts_count", "活跃账号数量").unwrap();
}
```

## 测试场景

1. **压力测试**: 10000 账号，1000 并发请求
2. **调度延迟**: P99 < 10ms
3. **列表响应**: 分页查询 < 50ms
4. **内存占用**: 调度器缓存 < 100MB

---

生成时间: 2026-04-01
