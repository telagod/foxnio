# FoxNIO 发布检查清单

**更新时间**: 2026-04-04
**口径**: 本清单只用于当前仓库的发布前检查，不沿用旧版本号、旧完成度、旧发布结论

## 现状

FoxNIO 当前已经具备发布前检查的基础条件：

- backend `cargo check` 可过
- frontend `npm run check` 可过
- 用户 dashboard、`/usage`、管理 dashboard 已接真实后端聚合
- 部署路径已统一到 `deploy.sh + Docker + compose profile`

但还不能直接把当前状态称作“可无条件发布”，因为仍缺三类关键闭环：

- 干净环境部署演练
- 最小 smoke 回归
- 真实 provider smoke

## 已完成

### 代码与静态检查

- [x] backend `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
- [x] frontend `npm --prefix frontend run check`
- [x] `git diff --check`

### 主业务链

- [x] 用户 dashboard 主链
- [x] 用户 `/usage` 日维度 usage 主链
- [x] 管理 dashboard `/api/v1/admin/dashboard/*`
- [x] API Key 创建 / 删除 / 列表主链
- [x] 验证码发送与改邮箱消费主链
- [x] Realtime/WebSocket usage / audit 主链
- [x] Gemini Native usage / quota 主链
- [x] Sora / prompt enhance usage / quota 主链
- [x] redeem code 写库、幂等、ledger 审计主链

### 发布口径第一轮收口

- [x] runtime env / config 统一
- [x] frontend `adapter-node` Docker 运行方式
- [x] compose `core` / `ui` / `edge` 分层
- [x] `deploy.sh` 命令收口

## 未完成

### P0 - 发布前必须完成

- [ ] backend release image 构建成功并记录结果
- [ ] `./deploy.sh build` 在干净环境跑通
- [ ] `./deploy.sh start` 起 `core` 栈成功
- [ ] `GET /health` 成功
- [ ] `GET /v1/models` 成功
- [ ] 一条认证接口成功
- [ ] `GET /api/v1/user/usage` 成功
- [ ] `GET /api/v1/admin/dashboard/stats` 成功
- [ ] `ui` profile 起服成功
- [ ] `edge` profile 起服成功
- [ ] 最小回滚手册完成

### P0 - 当前已知阻断

- [ ] 真实 OpenAI / Gemini 密钥缺失
- [ ] 真实 provider account / OAuth token 缺失
- [ ] frontend lint 仍会扫到 `frontend/build/**`

### P1 - 建议在发布前补齐

- [ ] 最小 smoke 脚本化
- [ ] usage / audit 核验表
- [ ] 发布环境变量权威说明
- [ ] 发布后日志、健康检查、数据库迁移检查步骤

## 下一步

### 发布前执行顺序

1. 跑 backend / frontend 当前静态检查
2. 构建 backend image
3. 起 `core` 栈
4. 跑最小 smoke
5. 需要控制台时起 `ui`
6. 需要边缘层时起 `edge`
7. 记录结果并补回文档

### 通过标准

可认为“本轮可发布”至少需要同时满足：

- backend 与 frontend 当前静态检查通过
- `core` 栈成功起服
- `/health` 与最小业务 smoke 通过
- 没有新的高优先级已知回归

## 说明

- 本清单不再使用旧的 `v0.2.1` 固定发布结论。
- 本清单不再使用伪精确完成度口径。
- 真实 provider smoke 未完成前，不应在任何发布文档里写“所有 provider 已可直接上线”。
