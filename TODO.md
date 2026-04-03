# FoxNIO TODO 清单

**更新时间**: 2026-04-03
**口径**: 只记录仍未闭环的工作，不再把已完成链路继续挂在 TODO 上
**整理原则**: 按“现状 / 已完成 / 未完成 / 下一步”维护，并尽量落到文件与链路

## 现状

- 用户 dashboard、用户 usage 报表、管理 dashboard、usage log、Gemini OAuth 持久化、前后端静态检查，当前都已闭环。
- 项目当前的主矛盾已经从“核心统计链路断裂”转成“控制面局部空壳 + 业务分支未闭环 + 部署与运维口径分裂 + 产品化细节不足”。
- 如果目标是把形态推向“完整可用产品”，接下来不能只补单点功能，必须同时推进控制面、部署、测试、文档、发布口径。

## 已完成

- [x] 用户 dashboard 改为真实调用 `getMe` / `listApiKeys` / `getUserUsage`
- [x] `/usage` 页面改为真实 `daily_usage` 报表
- [x] 管理 dashboard 改为真实聚合接口
- [x] `/api/v1/admin/dashboard/*` 路由接线完成
- [x] `/api/v1/admin/stats` 与 `/api/v1/admin/dashboard` 兼容口径收口到统一聚合服务
- [x] `backend/src/service/usage_log.rs` 接入真实 `usages` 表
- [x] `backend/src/service/gemini_oauth_service.rs` 接入 `oauth_tokens`
- [x] `cargo fmt --all` 通过
- [x] `cargo check --all-targets --message-format short` 通过
- [x] `pnpm run check` 达到 `0 error / 0 warning`
- [x] `docs/CURRENT_STATUS.md`、`docs/BUSINESS_LOGIC.md`、`docs/PROJECT_OVERVIEW.md`、`docs/API_REFERENCE.md` 已按真实状态重写
- [x] `README.md`、`backend/README.md`、`frontend/README.md`、`docs/DEVELOPMENT.md`、`docs/DEPLOYMENT.md` 已开始按当前口径回正
- [x] 管理员 API Key 列表已接真实数据与分页
- [x] 管理员账号详情 / 更新 / 测试兼容接口已从占位改为真实读取/更新逻辑
- [x] 用户 API Key 更新已接真实数据库写入
- [x] backend Docker 二进制名、端口、健康检查、Compose 缺失 `init.sql` 挂载已完成基础修正
- [x] `backend/src/handler/verify.rs` 已接 Redis 验证码缓存、60s 频控、SMTP 邮件、promo code 数据库校验、redeem code 预检
- [x] `backend/src/handler/quota.rs:get_quota_stats` 已接活跃订阅配额池聚合

## 未完成

### P0 - 控制面空壳与主链路阻断

- [ ] 统一 frontend 发布方式
  - 文件：`frontend/svelte.config.js`、`frontend/Dockerfile`
  - 现状：使用 `adapter-node`，Docker 却把 `build/` 当静态站点用 `serve`
  - 预期：要么按 Node server 运行，要么改成真正静态发布链

- [ ] 统一数据库迁移权威路径
  - 文件：`backend/src/db/pool.rs`、`Makefile`、部署脚本
  - 现状：运行时迁移走 `SeaORM`，脚本仍写 `sqlx migrate`
  - 预期：只保留一条权威迁移策略并写进文档

### P1 - 认证、增长、配额、备份业务链

- [ ] 把验证码消费校验并入注册 / 改邮箱提交流程
  - 文件：`backend/src/handler/verify.rs`、`backend/src/handler/auth/mod.rs`
  - 现状：发送链已闭环，但还没有“提交验证码并核销”的最终入口
  - 预期：注册 / 改邮箱提交前强制核验 Redis 中的验证码并完成一次性消费

- [ ] 统一 quota handler 与 service 语义
  - 文件：`backend/src/handler/quota.rs`
  - 现状：`get_quota_stats` 已接活跃订阅聚合，但 `get_quota_history` / `get_user_quota` 仍沿用旧 API key 语义
  - 预期：统一 quota handler 的“用户 / API key / subscription”主键口径，补齐历史与详情字段语义

- [ ] 完成备份列表
  - 文件：`backend/src/handler/backup.rs`
  - 现状：未实现
  - 预期：读取文件系统或对象存储元数据

- [ ] 完成备份下载
  - 文件：`backend/src/handler/backup.rs`
  - 现状：未实现
  - 预期：支持受控下载与权限审计

- [ ] 完成备份删除
  - 文件：`backend/src/handler/backup.rs`
  - 现状：未实现
  - 预期：支持安全删除与审计记录

- [ ] 完成兑换码 subscription 分支
  - 文件：`backend/src/service/redeem_code.rs`
  - 现状：未闭环
  - 预期：接真实订阅变更与有效期更新

- [ ] 完成兑换码 quota 分支
  - 文件：`backend/src/service/redeem_code.rs`
  - 现状：未闭环
  - 预期：接真实额度发放与账务记录

### P1 - Provider 与媒体能力闭环

- [ ] 完成 Gemini Native 路由剩余转发细节
  - 文件：`backend/src/gateway/gemini/mod.rs`
  - 现状：完整账户选择与请求转发仍有 TODO
  - 预期：与主网关保持一致的账户选择、失败处理、计费回写

- [ ] 实现 Sora 图片生成真实转发
  - 文件：`backend/src/gateway/routes.rs`
  - 现状：placeholder response
  - 预期：接真实上游请求、任务持久化、状态查询

- [ ] 实现 Sora 视频生成真实转发
  - 文件：`backend/src/gateway/routes.rs`
  - 现状：placeholder response
  - 预期：接真实上游请求、任务持久化、状态查询

- [ ] 实现 Sora 生成状态查询
  - 文件：`backend/src/gateway/routes.rs`
  - 现状：placeholder response
  - 预期：返回真实 generation 状态与失败原因

- [ ] 实现 prompt enhance 真实转发
  - 文件：`backend/src/gateway/routes.rs`
  - 现状：placeholder response
  - 预期：接真实上游增强逻辑与结果结构

### P1 - 账户、分组、用量、调度链

- [ ] 完成 `account_usage_service` 的真实查询/更新
  - 文件：`backend/src/service/account_usage_service.rs`

- [ ] 完成 `account_expiry_service` 的查询、状态更新与通知
  - 文件：`backend/src/service/account_expiry_service.rs`

- [ ] 完成 `account_group` 的持久化与自动分组规则
  - 文件：`backend/src/service/account_group.rs`

- [ ] 完成 `user_group` 的持久化、权限与限制获取
  - 文件：`backend/src/service/user_group.rs`

- [ ] 完成 `group.rs` 的真实使用量统计与 API Key 查询
  - 文件：`backend/src/service/group.rs`

- [ ] 完成 `account.rs` 的 token 刷新、tier 刷新、统计与 quota reset
  - 文件：`backend/src/service/account.rs`

### P2 - 批量操作、异步任务、恢复能力

- [ ] 完成 `batch.rs` 的 SeaORM 版本批量操作
  - 文件：`backend/src/service/batch.rs`

- [ ] 收口 `batch_operations.rs` 的真实批量刷新 tier / 今日统计 / 账号测试
  - 文件：`backend/src/service/batch_operations.rs`

- [ ] 完成 scheduled test plan 的真实执行
  - 文件：`backend/src/handler/scheduled_test_plan.rs`

- [ ] 完成 timing wheel 的真实任务执行
  - 文件：`backend/src/service/timing_wheel_service.rs`

- [ ] 完成 scheduler snapshot 恢复
  - 文件：`backend/src/service/scheduler_snapshot_service.rs`

- [ ] 完成 token refresh service 的等待机制与真实刷新逻辑
  - 文件：`backend/src/service/token_refresh_service.rs`

### P2 - Redis、限流、并发、等待队列

- [ ] 实现 Redis 版模型滑动窗口限流
  - 文件：`backend/src/service/model_rate_limit.rs`

- [ ] 实现网关 auth middleware 的 Redis 速率限制与并发限制
  - 文件：`backend/src/gateway/middleware/auth.rs`

- [ ] 实现等待队列 Redis 持久化
  - 文件：`backend/src/service/wait_queue.rs`

- [ ] 补齐等待队列超时、取消、恢复后的状态一致性
  - 文件：`backend/src/service/wait_queue.rs` 及相关调用链

### P2 - 运维聚合与可观测性

- [ ] 完成 `ops_service.rs` 的数据库插入、查询、清理、统计
- [ ] 完成 `ops_metrics_collector.rs` 的真实指标查询与持久化
- [ ] 完成 `ops_aggregation_service.rs` / `ops_alert_evaluator_service.rs` 的 leader lock 与真实聚合
- [ ] 完成 `ops_cleanup_service.rs`、`ops_trends.rs`、`ops_health_score.rs`、`ops_realtime_traffic.rs` 的真实数据来源
- [ ] 完成 `usage_cleanup.rs`、`usage_cleanup_service.rs`、`idempotency_cleanup_service.rs` 的数据库级清理

### P2 - 前端产品化

- [ ] 为 `dashboard` 页面补测试
  - 文件：`frontend/src/routes/dashboard/+page.svelte`
  - 预期：覆盖加载成功、鉴权跳转、API Key 创建/删除、错误态

- [ ] 为 `usage` 页面补测试
  - 文件：`frontend/src/routes/usage/+page.svelte`
  - 预期：覆盖周期切换、空态、错误态、表格渲染

- [ ] 为 `apikeys` 页面补测试
  - 文件：`frontend/src/routes/apikeys/+page.svelte`
  - 预期：覆盖列表、搜索、复制、创建、删除

- [ ] 为 `admin` 页面补测试
  - 文件：`frontend/src/routes/admin/+page.svelte`
  - 预期：覆盖 dashboard 拉取、刷新、错误态、数据呈现

- [ ] 统一 loading / error / empty state 表现
  - 范围：`frontend/src/routes/*`
  - 预期：减少页面之间体验割裂

- [ ] 统一中英文混杂的 UI 文案
  - 范围：`frontend/src/routes/*`
  - 预期：形成一致的产品语言风格

- [ ] 明确管理员权限不足时的前端回退行为
  - 范围：`frontend/src/routes/admin/*`、`src/lib/api.ts`
  - 预期：避免 403 时直接报错白屏

### P3 - API 与架构一致性债

- [ ] 统一 handler 响应格式
- [ ] 统一权限检查落点
- [ ] 统一路径参数风格（`{id}` / `:id`）
- [ ] 恢复 Swagger UI 集成或明确永久弃用
- [ ] 拆分超长模块
  - 重点：`backend/src/gateway/routes.rs`、`backend/src/gateway/scheduler/mod.rs`
- [ ] 收口前端 API client 的历史兼容命名与类型风格
  - 文件：`frontend/src/lib/api.ts`

### P3 - 测试与质量门禁

- [ ] 建立 backend 关键链路回归清单
  - 重点：auth、usage、dashboard、admin、gateway

- [ ] 建立 frontend 页面级回归清单
  - 重点：login、dashboard、usage、apikeys、admin

- [ ] 为控制面 placeholder 修复同步补集成测试
- [ ] 明确 CI 需要跑的最小集合
  - 建议：`cargo fmt --check`、`cargo check --all-targets`、关键 backend tests、`pnpm run check`、前端页面测试

- [ ] 统一前端包管理器
  - 现状：`Makefile` 用 `npm`，验证用 `pnpm`，仓库有 `package-lock.json`
  - 预期：只保留一种权威口径

### P3 - 文档与发布口径

- [ ] 持续清理历史专题文档中的过期完成度表述
  - 重点：`docs/` 下专题报告类文件

- [ ] 输出“最小可发布清单”
  - 范围：功能、测试、部署、回滚、监控

- [ ] 输出“最小 smoke test 手册”
  - 范围：健康检查、认证、用户 usage、管理 dashboard、聊天调用

- [ ] 输出“环境变量与配置权威说明”
  - 范围：`.env.example`、`config.yaml`、`FOXNIO_CONFIG`

### P4 - 走向完整可用产品

- [ ] 做一轮控制台 IA 梳理
  - 目标：让用户与管理员更容易找到核心入口，而不是靠现有零散页面拼接

- [ ] 做一轮权限矩阵梳理
  - 目标：明确用户 / 管理员 / 更高权限角色的页面与接口可见性

- [ ] 做一轮账务与配额体验梳理
  - 目标：让余额、配额、订阅、兑换码之间的关系对最终用户可解释

- [ ] 做一轮运营闭环梳理
  - 目标：告警、审计、备份、恢复、测试计划、批量操作形成最小闭环

- [ ] 做一轮发布演练
  - 目标：从干净环境构建、迁移、启动、smoke test、回滚全部走通

## 下一步

1. 先补 verify / quota / backup / redeem / Sora / Gemini Native 等业务支线。
2. 并行统一 frontend 发布方式、迁移口径与 smoke test。
3. 然后补前端测试、统一交互体验、统一包管理器与 CI 门禁。
4. 最后推进发布演练与产品化收口，让仓库从“能开发”走向“可发布、可维护、可回归”。
