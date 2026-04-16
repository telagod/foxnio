# FoxNIO 进化路线（2026-04）

## 对齐对象

- **Sub2API**：多账号管理、精确计费、智能调度、管理后台、外部系统集成。
- **OmniRoute**：多层 fallback、provider 级 resilience、语义缓存、CLI/Agent 场景接入。
- **LiteLLM**：超广模型兼容面、router policy、observability、预算/guardrail 能力。

## 外部项目调研结论

### Sub2API
- 强项是 **多账号统一接入 + 平台计费 + 管理后台 + 原生工具可用**。
- 对 FoxNIO 的意义不是照抄 UI，而是确认“账号池运营平台”这条赛道成立。

### OmniRoute
- 强项是 **OpenAI-compatible 单入口 + smart routing + retries/fallbacks + policy/caching/observability**。
- 对 FoxNIO 的价值是提醒：代理层要继续补 provider 级容灾、幂等与请求去重，而不是只做 CRUD 控制台。

### LiteLLM
- 强项是 **超广 provider 兼容、cost tracking、guardrails、load balancing、logging**，并公开强调性能与稳定发布节奏。
- 对 FoxNIO 的价值是提醒：除了“能转发”，还要让策略、成本、观测三件事能被解释和治理。

### OngoingAI Gateway
- 强项是 **低开销 tracing / cost visibility / audit-ready logs**，把“观测”做成默认能力而不是外挂。
- 对 FoxNIO 的价值是提醒：品牌化不能只喊高性能，必须能回答“这次请求到底走了哪个号、花了多少钱、为何失败”。

### labring/aiproxy
- 强项是 **多协议入口 + 插件化缓存 + 监控告警 + 多租户**，强调生产可运营性。
- 对 FoxNIO 的价值是提醒：控制面和插件边界要清晰，缓存/搜索/审计类能力适合插件化，而不是侵入热路径。

## FoxNIO 当前应优先补齐

### 1. 核心能力对齐
- 大规模账号池调度：provider 维度候选集、粘性会话、故障转移、限流恢复。
- 批量运营能力：批量导入、批量更新、批量凭证轮换、批量状态治理。
- 控制面解释力：账号 → 模型 → 配额 → 实际上游 的追踪闭环。

### 2. 差异化方向
- **高性能网关代理**：Rust 热路径 + Redis 队列/限流 + 轻控制面。
- **大号池运营**：把“账号管理系统”升级成“号池运营系统”。
- **批量操作友好**：导入、轮换、修复、封禁、恢复都要支持大批次执行。

## 本轮选定演化线路

### 主线：批量性能 + 大号池调度 + 品牌定位
1. **先打通高性能批量入口**
   - 优先修复 `fast-import` 路由当前的 `Send trait` 编译阻塞。
   - 在保持可编译前提下，先修正批量导入返回假 ID 的问题，保证控制面解释链不失真。
   - 路由挂载完成后，再让前端在大于 100 条时自动切换并回显跳过/失败详情。
2. **再压大规模号池热路径**
   - 继续使用 provider 维度候选缓存与 round-robin 快选。
   - 本轮先落 `provider` 级独立 round-robin index，避免跨 provider 热点争用同一原子计数器。
   - 下一步补可恢复 cooldown、可解释 sticky/session 指标、候选账号轻量引用缓存。
3. **最后收口品牌**
   - 统一对外叙事：FoxNIO 不只是 relay，而是面向大规模号池的高性能 AI gateway / control plane。
   - 核心卖点固定为：**批量导入快、号池调度稳、代理链路清晰**。

## 本轮新增落地（2026-04-13）

- 调度热路径：`backend/src/gateway/scheduler/mod.rs`
  - `select_fast()` 改为 `provider` 级独立 round-robin index。
  - 目标是减少大号池混合流量下的跨 provider 计数器争用，让热门 provider 不再互相抢同一把原子锁。
- 批量导入控制面：`backend/src/service/batch_import.rs`、`backend/src/handler/admin_accounts.rs`
  - `fast-import` 实际导入结果新增 `providers` 汇总，按 provider 返回 `total/imported/skipped/failed`。
  - 目标不是改热路径吞吐，而是把大批次导入后的运营判断前置到服务端，避免前端再扫上万条输入做二次统计。
- 批量观测与基准：`backend/src/metrics/mod.rs`、`scripts/benchmark_fast_import.py`、`docs/BENCHMARKS.md`
  - 新增 batch operation Prometheus 指标：次数、耗时、最近吞吐、最近批次规模。
  - `fast-import` / `preview` 响应新增 `throughput_items_per_sec`，便于运营面直接判断批量入口是否退化。
  - 新增可复现 benchmark 脚本，开始把“高性能”从口号拉回到固定跑法。
- 已有批量链路继续保留：
  - `backend/src/service/batch_operations.rs` 已用 `update_many()` 合并批量更新/批量凭证轮换。
  - 当前批量主线从“能批量”推进到“批量时仍保持回显真实、事务边界清晰、控制面可解释”。

## 优秀项目再梳理（2026-04-13）

已补充一次公开对标扫描，筛选用于演化路线的项目：

- **Sub2API**  
  对齐信号：多账号管理、精确计费、原生分发、后台运维。  
  结论：继续作为账号池运营标杆。  
  链接：<https://github.com/Wei-Shaw/sub2api>
- **Plexus**  
  对齐信号：协议转换、Responses API、OAuth provider、provider cooldown。  
  结论：FoxNIO 下一步应补“可解释 provider cooldown + model alias/routing”而不是继续堆散点 endpoint。  
  链接：<https://github.com/mcowger/plexus>
- **Ferro Labs AI Gateway**  
  对齐信号：高性能叙事、公开 benchmark、低额外开销、插件/guardrail。  
  结论：FoxNIO 品牌化要从“多账号中转”升级成“面向大号池运营的高性能 AI gateway”，并补可复现 benchmark。Ferro 现在把 *13,925 RPS @ 1,000 VU*、*25µs 插件态开销* 直接写进 README，说明“性能口径公开化”本身就是产品卖点。  
  链接：<https://github.com/ferro-labs/ai-gateway>
- **LiteLLM**  
  对齐信号：统一路由、预算/成本治理、fallback、日志观测。  
  结论：复用其路由策略和预算治理思想，不复制其技术栈。其最近继续在 router hot path 上做 provider 解析去重，说明高频策略层的微优化依然值得做。  
  链接：<https://github.com/BerriAI/litellm>
- **OmniRoute**  
  对齐信号：多供应商 fallback、多策略路由、dashboard 与多 key 管理。  
  结论：重点吸收其多 key 续航与策略多样性思路。  
  链接：<https://omniroute.online/>
- **Bifrost**  
  对齐信号：高并发场景性能、低额外开销、可观测治理栈。  
  结论：作为 FoxNIO 热路径性能对标，补齐吞吐/排队/故障可恢复指标。Bifrost 对外强调 *11µs overhead @ 5k RPS* 与 OTel / Prometheus 双栈，这意味着 FoxNIO 也该把“号池调度 + 运营观测”的指标体系做成默认配置。  
  链接：<https://github.com/maximhq/bifrost>

## 本轮新增产品化动作（2026-04-13 / benchmark）

- `scripts/benchmark_fast_import.py`
  - 支持 `--providers openai,anthropic,gemini` 混合号池压测
  - 支持 `--repeat N` 多轮重复跑，自动汇总平均吞吐 / 平均耗时 / best / worst
  - 支持 `--format markdown|json|jsonl`，方便运营侧留档和后续接 dashboard
- `docs/BENCHMARKS.md`
  - 新增混合 provider 场景口径
  - 明确接口耗时、客户端 wall clock、Prometheus 三种观测口径

这一步不是单纯补脚本，而是把“批量操作性能”从开发者自测，往“运营可复跑、可留痕、可对比”的产品化方向推进了一格。

## 本轮新增产品化动作（2026-04-13 / ops snapshot）

- `backend/src/service/dashboard_query_service.rs`
  - admin dashboard 聚合结果新增 `ops` 区块。
  - 直接拉通数据库快照与 Prometheus 内存指标，把 `active_users_24h`、`error_rate_1h`、`avg_response_time_ms`、`cache_hit_rate`、批量总次数、批量错误数、最近 `fast_import/preview` 吞吐与批次规模收进同一份返回。
- `frontend/src/routes/(app)/admin/stats/+page.svelte`
  - admin Statistics 页面新增 **Ops & Batch Performance** 面板。
  - 运营侧可以直接看“最近批量吞吐”“最近批次规模”“1h 错误率”，不必再手动查 `/metrics` 或对 Prometheus label。

这一步的意义是把“批量性能观测”从纯 Prometheus 指标，推进到控制台可读、可运营、可快速判断退化。

## 最新外部对标快照（2026-04-13）

这轮不再泛泛搜“网关项目”，只保留对 FoxNIO 演化最有价值的公开样本：

- **Sub2API**（GitHub，`2026-04-12` 仍在更新）
  - 公开定位已经非常清晰：订阅配额分发、精确计费、智能调度、后台运营、支付与外部系统集成。
  - 最近公开变更继续集中在 scheduler cache、内容级会话 hash、账号隐私与可观测细节，说明这条赛道的竞争已经进入“热路径细节 + 运营可解释性”阶段。
  - 对 FoxNIO 的直接结论：必须继续补 **sticky/session 种子稳定性、调度缓存瘦身、批量账号运营**。
- **Plexus**（GitHub，`2026-04-11` 发布 `v0.19.10`）
  - 强项是 Responses / Chat / Anthropic / Gemini 多协议转换、OAuth provider、request tracing、usage stats。
  - 对 FoxNIO 的意义：继续强化 `OpenAI-compatible + 多协议兼容 + request trace`，但不要把主叙事稀释成“又一个多 provider 代理”。
- **Ferro Labs AI Gateway**（GitHub，`2026-04-09` 发布 `v1.0.4`）
  - 公开把 benchmark 打成卖点：`13,925 RPS @ 1,000 VU`，并给出固定环境与方法。
  - 对 FoxNIO 的意义：FoxNIO 也该公开自己的压测方法学，把 **批量吞吐** 和 **调度热路径** 的口径固定下来。
- **labring/aiproxy**（GitHub，`2026-03-27` 发布 `v0.4.9`）
  - 强项是多协议入口、监控告警、多租户、运维面板。
  - 对 FoxNIO 的意义：运维侧要从“能看统计”推进到“能按失败原因、provider、批次行为直接定位问题”。
- **ongoingai/gateway**
  - 强项是低开销 tracing、成本可见、audit-ready logs。
  - 对 FoxNIO 的意义：请求级链路解释要继续补到“账号 → provider → 模型 → cost → error”一眼看明白。

## 本轮新增决策：FoxNIO 的主打法

### 1. 不和 LiteLLM 拼 provider 面宽
- FoxNIO 不追求“谁都能接”的最大兼容面。
- FoxNIO 要优先赢在 **大规模账号池运营效率** 与 **运营控制面解释力**。

### 2. 不和 Sub2API 拼支付闭环优先级
- Sub2API 已经把支付与完整平台经营做得很深。
- FoxNIO 当前更适合先把 **高性能代理 + 大批量运营 + 观测产品化** 做成确定性优势。

### 3. 明确三条产品主轴
- **Gateway axis**：高性能热路径、provider fallback、sticky session、cooldown recovery
- **Ops axis**：批量导入/轮换/封禁/恢复/切组、失败原因聚合、健康分与恢复建议
- **Observability axis**：请求 trace、批量吞吐、调度命中率、provider 维度错误和冷却状态

## 下一轮落点（按优先级）

### P0：把运营看板补到“可直接决策”
- dashboard 新增 scheduler / cooldown / sticky session 面板
- 增加 provider 维度：`available / cooling_down / rate_limited / unhealthy`
- 增加批量操作失败原因 TopN、最近大批次执行记录、最近吞吐趋势

### P1：把 benchmark 变成固定资产
- 固化 `FoxNIO benchmark methodology`
- 至少保留三组规模：`1k / 10k / 100k accounts`
- 至少保留两类负载：`single provider` / `mixed providers`
- 每轮演化把 best / avg / worst 结果写回文档

### P1：把号池运营能力继续产品化
- 批量封禁 / 恢复 / 清限流 / 切组都补原因汇总
- 导入结果页补重复原因、失败分类、provider 偏斜提示
- admin stats 里加入“最近 1h/24h 批量操作对网关错误率的影响”

## 下一轮建议

1. **批量操作继续做成“运营级”**
    - 批量封禁 / 恢复 / 清限流 / 切换分组 / 轮换凭证改成单 SQL 批更新（状态/分组/清限流已完成）
   - 导入结果页继续补重复原因聚合、失败类型聚合；provider 维度聚合已在接口层补齐
2. **调度热路径继续瘦身**
   - provider 维度独立 round-robin index
   - cache 使用 `Arc<AccountInfo>` 降低 clone 成本
   - 把 cooldown 检查并回 `get_available_accounts`
3. **号池运营面增强**
   - 批量封禁 / 恢复 / 清限流 / 切换隐私模式
   - 账号健康分、失败原因聚合、恢复建议
4. **代理差异化**
   - model routing policy
   - request dedupe / idempotency
   - warm cache for account/model availability
5. **观测产品化**
   - 把 batch / scheduler / cooldown / sticky session 指标统一接入 admin metrics 与 Grafana
   - 固化每轮 benchmark 结果，形成可追踪基线
