# FoxNIO 进化路线（2026-04）

## 核心方向

- **多账号管理**：精确计费、智能调度、管理后台、原生工具接入。
- **Router Policy**：多层 fallback、provider 级 resilience、观测、预算与 guardrail。
- **差异化**：把批量操作性能与大规模号池运营做成核心卖点。

## FoxNIO 当前应优先补齐

### 1. 核心能力
- 大规模账号池调度：provider 维度候选集、粘性会话、故障转移、限流恢复。
- 批量运营能力：批量导入、批量更新、批量凭证轮换、批量状态治理。
- 控制面解释力：账号 → 模型 → 配额 → 实际上游 的追踪闭环。

### 2. 差异化方向
- **高性能网关代理**：Rust 热路径 + Redis 队列/限流 + 轻控制面。
- **大号池运营**：把"账号管理系统"升级成"号池运营系统"。
- **批量操作友好**：导入、轮换、修复、封禁、恢复都要支持大批次执行。

## 本轮选定演化线路

### 主线：批量性能 + 大号池调度 + 品牌定位
1. **先打通高性能批量入口**
   - 修复 `fast-import` 路由编译阻塞。
   - 修正批量导入返回假 ID 的问题，保证控制面解释链不失真。
   - 路由挂载完成后，前端在大于 100 条时自动切换并回显跳过/失败详情。
2. **再压大规模号池热路径**
   - 继续使用 provider 维度候选缓存与 round-robin 快选。
   - 落 `provider` 级独立 round-robin index，避免跨 provider 热点争用。
   - 补可恢复 cooldown、可解释 sticky/session 指标、候选账号轻量引用缓存。
3. **最后收口品牌**
   - 统一对外叙事：FoxNIO 是面向大规模号池的高性能 AI gateway / control plane。
   - 核心卖点：**批量导入快、号池调度稳、代理链路清晰**。

## 落地记录

### 2026-04-13: 调度热路径 + 批量观测
- `select_fast()` 改为 provider 级独立 round-robin index
- `fast-import` 新增 providers 汇总（按 provider 返回 total/imported/skipped/failed）
- 新增 batch operation Prometheus 指标
- 新增可复现 benchmark 脚本

### 2026-04-13: Ops Snapshot
- admin dashboard 聚合结果新增 ops 区块
- admin Statistics 页面新增 Ops & Batch Performance 面板

### 2026-04-16: 性能优化 + Provider 对齐
- DB 查询从全表扫描改为 SQL 聚合
- Provider registry 跨层对齐
- 前端缓存 TTL 分级 + 组件拆分

### 2026-04-16: Session + 调度策略
- Session ID 从 metadata.user_id 提取（修复硬编码）
- 分组级调度策略：Sticky / LoadBalance / Scoring
- UA 归一化避免版本升级导致 session 漂移
- 流式 Failover 自动切换账号重试

### 2026-04-16: 计费体系重构
- QuotaGate 统一配额网关（原子结算）
- DB 驱动定价（PricingService + model_configs 表）
- 余额预检 + 分组配额强制执行
- Cache token 提取（Anthropic + OpenAI）
- 并发控制接入热路径

## 三条产品主轴

- **Gateway axis**：高性能热路径、provider fallback、sticky session、cooldown recovery
- **Ops axis**：批量导入/轮换/封禁/恢复/切组、失败原因聚合、健康分与恢复建议
- **Observability axis**：请求 trace、批量吞吐、调度命中率、provider 维度错误和冷却状态

## 下一轮落点

### P0：运营看板补到"可直接决策"
- dashboard 新增 scheduler / cooldown / sticky session 面板
- provider 维度：available / cooling_down / rate_limited / unhealthy
- 批量操作失败原因 TopN、最近大批次执行记录

### P1：Benchmark 变成固定资产
- 固化 benchmark methodology
- 三组规模：1k / 10k / 100k accounts
- 两类负载：single provider / mixed providers

### P1：号池运营能力继续产品化
- 批量封禁/恢复/清限流/切组补原因汇总
- 导入结果页补重复原因、失败分类、provider 偏斜提示
