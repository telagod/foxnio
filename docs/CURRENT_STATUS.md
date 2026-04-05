# FoxNIO 当前状态

**更新时间**: 2026-04-05
**口径**: 只记录当前仓库已核对事实，不把计划写成完成

## 现状

FoxNIO 是一个 Rust/Axum + SvelteKit 的 AI gateway / control plane 项目，当前覆盖四层职责：

- gateway：OpenAI 兼容、Anthropic `messages`、Gemini Native、Realtime/WebSocket、Sora / prompt enhance
- user self-service：注册、登录、个人信息、API Key、dashboard、`/usage`
- admin console：用户、账号、API Key、dashboard、部分控制面查询
- control plane：验证码、兑换码、quota、审计、Webhook、告警、备份、调度

## 完成度总览

| 阶段 | 状态 | 说明 |
|------|------|------|
| P0 发布链 | 完成 | release image、compose 起服、smoke、回滚手册、环境变量说明、lint 范围修正 |
| P1 failure chain + 控制面 | 完成 | Realtime/Gemini/Sora 集成测试、usage/audit 核验表、backup/redeem 正式化、账务解释统一 |
| P2 运维与异步 | 部分完成 | batch、scheduler snapshot、token refresh、ops 模块仍偏骨架 |
| P2 前端产品化 | 部分完成 | loading/error/empty state 已统一、a11y 已清理、页面测试与权限回退仍待补齐 |
| P3 文档清理 | 进行中 | 历史专题文档已清理、架构图已输出、权威文档已收束 |
| 真实 provider smoke | 未完成 | 缺真实 OpenAI / Gemini 密钥与账号输入 |

## 已完成

### 发布链（P0）

- backend release image 构建、compose 起服、`/health` 回归均已通过
- `deploy.sh build/start/build-ui/start-ui/build-edge/start-edge` 全链路已验证
- 最小 smoke 脚本、回滚手册、环境变量权威说明已输出
- frontend lint 已排除 `frontend/build/**`

### Failure chain 与控制面（P1）

- Realtime/WebSocket、Gemini Native、Sora / prompt enhance 集成测试已通过
- usage / audit 权威核验表已输出（见 `USAGE_AUDIT_REFERENCE.md`）
- backup export/import 与恢复演练已完成
- redeem 限次、幂等、额度来源解释、ledger 查询已完成
- 账务 / quota / subscription / redeem 解释已统一

### 前端

- Tailwind v3 → v4（CSS-first）
- ESLint v8 → v10（flat config）
- 全页面去 emoji 换 SVG、a11y 修复、深浅色、响应式
- svelte-check 0 errors 0 warnings
- loading / error / empty state 已统一
- 401/403 API 错误已在 ApiClient 层统一处理

### 工程与部署

- backend `cargo check` / frontend `npm run check` 均可过
- runtime config、Docker、compose profile、deploy 脚本已收口
- `.env` / `config.yaml` / runtime env 关系已说明（见 `ENV_AND_ROLLBACK.md`）

## 未完成

### 真实 provider smoke

缺真实密钥与账号输入，代码链已接真但无真实回归报告。

### P2 运维与异步

- `batch.rs` / `batch_operations.rs` 真执行链
- `scheduler_snapshot_service.rs` 恢复能力
- `token_refresh_service.rs` 真刷新链
- `ops_*` 模块真实写入、聚合、清理、leader lock
- 等待队列 Redis 持久化、超时、取消、恢复

### P2 前端

- dashboard / usage / apikeys / admin 页面测试
- 权限不足时前端回退行为进一步细化

### P3 文档

- migration 权威路径说明
- `npm` / `pnpm` / `make` 权威命令统一

## 下一步

1. 用真实密钥完成 OpenAI / Gemini 真 smoke
2. 补 P2 运维异步能力（batch、scheduler、token refresh）
3. 补前端页面测试
4. 继续收束文档口径
