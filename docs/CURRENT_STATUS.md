# FoxNIO 当前状态

**更新时间**: 2026-04-04  
**口径**: 只记录当前仓库已核对事实，不把计划写成完成

## 现状

FoxNIO 当前已经不是“只有代理入口”的项目，而是一个单体 Axum 服务承载的混合系统：

- gateway：OpenAI 兼容、Anthropic `messages`、Gemini Native、Realtime/WebSocket、Sora / prompt enhance
- user self-service：注册、登录、个人信息、API Key、dashboard、`/usage`
- admin console：用户、账号、API Key、dashboard、部分控制面查询
- control plane：验证码、兑换码、quota、审计、Webhook、告警、备份、调度与部分运维能力

截至本轮核对，可以确认四个现实判断：

- backend `cargo check` 可过，frontend `npm run check` 可过。
- 用户 dashboard、`/usage`、管理 dashboard 已接真实后端聚合，不再是 placeholder 数据。
- Realtime/WebSocket、Gemini Native、Sora / prompt enhance 主转发链已接真实上游，并有 usage / audit / quota 回写。
- 项目已进入“主链大体接真，但发布回归、真实 provider smoke、控制面补齐仍待完成”的阶段。

补充说明：

- frontend `eslint` 当前仍会扫到 `frontend/build/**` 产物，属于工程配置问题，不是本轮业务主链回归失败。
- 真实 OpenAI / Gemini smoke 仍缺密钥与账号输入，因此没有形成真上游回归报告。

## 已完成

### 用户侧闭环

- `GET /api/v1/user/me` 已返回真实用户信息。
- 用户 dashboard 已接真实 API Key 列表与 usage 汇总。
- `GET /api/v1/user/usage` 已支持按天聚合 usage，并支持 `days` 查询参数。
- 用户改邮箱已要求并消费验证码，不再只是前端表单更新。

### 管理侧闭环

- `/api/v1/admin/dashboard/stats`
- `/api/v1/admin/dashboard/trend`
- `/api/v1/admin/dashboard/line`
- `/api/v1/admin/dashboard/pie`
- `/api/v1/admin/dashboard/model-distribution`
- `/api/v1/admin/dashboard/platform-distribution`

以上接口已走真实数据库聚合，管理端 dashboard 页面已接入这些接口。

- 管理员 API Key 列表已支持分页查询。
- dashboard 查询逻辑已从 handler 下沉到 `backend/src/service/dashboard_query_service.rs`，避免控制器堆积查询细节。

### 网关、计费与审计闭环

- `backend/src/service/usage_log.rs` 已以 `usages` 真表为准提供写入、查询与聚合。
- API Key 鉴权、RPM / concurrency Redis 风控、quota 消耗、usage 回写已接入主链。
- Realtime/WebSocket 已补握手鉴权、上游转发、request/response 级 audit、成功与失败 usage 回写。
- Gemini Native 已补 OAuth / API Key 凭据解析、retry、typed error、`generateContent` / stream / `countTokens` / `embedContent` usage 回写。
- Sora image / video / status 与 prompt enhance 已补真实 OpenAI HTTP 转发、成功 / 失败 usage 与 quota 口径。

### 认证与兑换闭环

- 注册、登录、刷新、验证码发送、密码重置、TOTP 主链已接线。
- redeem code 已具备事务写库、幂等键与 ledger 审计的主闭环。

### 发布口径第一轮收口

- backend 已统一支持 `config.yaml`、`FOXNIO_CONFIG`、`.env` 与关键 runtime env。
- `backend/src/main.rs` 已走 `config.server.bind_addr()`，不再硬编码监听地址。
- frontend Docker 已切回 `adapter-node` 正确运行方式：`node build`。
- `docker-compose.yml` 已按 `core` / `ui` / `edge` 分层。
- `deploy.sh` 已收口为 core 默认启动，`ui` 与 `edge` 按需启动。
- `nginx.conf` backend upstream 已改为 `backend:8080`。

### 本轮已核验证

- `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
- `npm --prefix frontend run check`
- `git diff --check`

## 未完成

### 真实 provider 回归未完成

阻断不是代码空壳，而是输入条件缺失：

- 缺真实 `OPENAI_API_KEY`
- 缺真实 `GEMINI_API_KEY` / `GOOGLE_API_KEY`
- 缺可用 provider account / OAuth token

当前状态是：

- smoke seed 已有骨架
- 真 smoke 脚本与测试入口已存在
- 真上游成功 / 失败回归结果仍未产出

### 控制面仍未达到完整产品状态

- `backup import` 未实现。
- backup 的对象存储、恢复演练、正式审计仍未完成。
- redeem 的活动规则、额度来源说明、查询与运营语义仍不完整。
- quota / subscription / balance / redeem 的对外产品解释仍未统一。
- `ops_*`、`batch*`、等待队列、调度恢复、快照恢复等模块仍偏骨架。

### 发布链仍缺最终演练

- 还缺“干净环境从 build 到 up 到 smoke”的完整上线演练。
- 还缺发布后的最小 smoke 套件与回滚说明。
- 还缺 frontend lint 范围清理，避免把构建产物误算进 CI 结果。

### 文档体系仍有历史债务

- `docs/` 下仍有多份历史专题文档带着旧阶段描述。
- 部分文档的“完成度”口径已落后于当前代码状态。
- 发布、运行、配置、命令口径仍需继续统一。

## 下一步

1. 先补发布闭环：backend image、core compose、`/health`、最小 smoke、回滚手册。
2. 再补真实 provider smoke：OpenAI、Gemini、Realtime、Sora / prompt enhance。
3. 再收控制面缺口：backup、redeem、quota / subscription 解释统一。
4. 最后继续清理 `docs/` 历史专题文档，把“现状 / 已完成 / 未完成 / 下一步”作为统一写法。
