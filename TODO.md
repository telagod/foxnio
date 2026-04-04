# FoxNIO TODO

**更新时间**: 2026-04-04  
**目标**: 把项目从“主链已接真”推进到“可构建、可部署、可运营、可回归的完整可用产品”

## 现状

- 主链现状：用户 dashboard、`/usage`、管理 dashboard、Realtime、Gemini Native、Sora / prompt enhance 都已接真。
- 发布现状：runtime config、frontend Docker、compose profile、deploy 脚本已完成第一轮收口。
- 当前主矛盾：不是“系统完全起不来”，而是“真实 smoke 缺密钥、控制面仍有骨架、发布链还差最后一轮端到端回归”。

## 已闭环主链

- [x] 用户 dashboard 真实化
- [x] `/usage` 日维度 usage 聚合
- [x] 管理 dashboard `/api/v1/admin/dashboard/*`
- [x] `usage_log` 接真实 `usages`
- [x] API Key 级 RPM / concurrency Redis 风控
- [x] 用户 subscription / quota 主语义统一
- [x] 验证码发送 + 注册/改邮箱消费主链
- [x] redeem code 写库、幂等键、审计 ledger
- [x] Realtime/WebSocket 鉴权、真实上游转发、usage/audit
- [x] Gemini Native retry、typed error、usage/audit、quota
- [x] Sora / prompt enhance 真实 OpenAI 转发、usage、quota
- [x] smoke seed CLI 与真 smoke 骨架脚本
- [x] runtime env/config 统一加载
- [x] frontend `adapter-node` Docker 运行口径
- [x] compose core/ui/edge 分层
- [x] deploy 脚本重写

## P0 - 上线阻断

- [ ] backend release image 完整构建成功
- [ ] `docker compose up -d postgres redis backend` 后 `/health` 回归通过
- [ ] `deploy.sh start` 在干净环境跑通
- [ ] `deploy.sh start-ui` 跑通 frontend Node server
- [ ] `deploy.sh start-edge` 跑通 nginx profile
- [ ] 补最小发布 smoke：
  - `GET /health`
  - `GET /v1/models`
  - 一条用户认证接口
  - `GET /api/v1/user/usage`
  - `GET /api/v1/admin/dashboard/stats`
- [ ] 输出环境变量权威说明
- [ ] 输出最小发布 / 回滚手册

## P0 - 真实 provider smoke

- [ ] 准备最小真实输入物：
  - 1 个用户
  - 1 个 FoxNIO API Key
  - 1 个 OpenAI account
  - 1 个 Gemini account / OAuth token
- [ ] 跑真实 OpenAI smoke：
  - `chat/completions`
  - `responses`
  - `realtime`
  - `images/videos`
  - `prompt enhance`
- [ ] 跑真实 Gemini smoke：
  - `generateContent`
  - `streamGenerateContent`
  - `countTokens`
  - `embedContent`
- [ ] 把真 smoke 结果写回文档

## P1 - failure-chain 与审计回归

- [ ] Realtime integration test：
  - `upstream_prepare_failed`
  - `upstream_connect_failed`
  - `upstream_stream_error`
  - `response.failed`
- [ ] Gemini integration test 继续补：
  - stream read error
  - quota consume metadata
  - failed usage metadata
- [ ] Sora / prompt enhance integration test：
  - 成功 usage
  - 失败 usage
  - 状态查询失败
  - quota consume
- [ ] 建 usage / audit 权威核验表：
  - 每条成功写什么
  - 每条失败写什么
  - metadata 长什么样

## P1 - 控制面补齐

- [ ] backup 正式化：
  - `import`
  - 对象存储
  - 生命周期
  - 恢复演练
  - 审计
- [ ] redeem 正式化：
  - 活动规则
  - 限次与幂等
  - 额度来源解释
  - ledger 查询接口
- [ ] 账务 / quota / subscription / redeem 解释统一
- [ ] account/group/control-plane 查询链继续接真

## P2 - 运维与异步

- [ ] `batch.rs` / `batch_operations.rs` 真执行链
- [ ] scheduled test plan 真执行
- [ ] `timing_wheel_service.rs` 真任务执行
- [ ] `scheduler_snapshot_service.rs` 恢复能力
- [ ] `token_refresh_service.rs` 真刷新链
- [ ] 模型级 / 账号级 / 排队层风控
- [ ] 等待队列 Redis 持久化、超时、取消、恢复
- [ ] `ops_*` 模块真实写入、聚合、清理、leader lock

## P2 - 前端产品化

- [ ] `dashboard` 页面测试
- [ ] `usage` 页面测试
- [ ] `apikeys` 页面测试
- [ ] `admin` 页面测试
- [ ] loading / error / empty state 统一
- [ ] 中英文混杂 UI 文案统一
- [ ] 管理员权限不足时前端回退行为明确化

## P3 - 文档清理

- [ ] 清理 `docs/` 历史专题文档中的旧完成度
- [ ] 统一 migration 权威路径说明
- [ ] 统一 frontend 构建 / 运行 / Docker 命令口径
- [ ] 统一 `npm` / `pnpm` / `make` 的权威命令说明
- [ ] 统一 `.env` / `config.yaml` / runtime env 关系说明

## 下一刀

1. 先把 backend image 与 core compose `/health` 跑通。
2. 再补发布文档与上线 smoke 手册。
3. 然后回头收 backup / redeem / 账务解释这三块最影响“完整可用产品”的断点。
