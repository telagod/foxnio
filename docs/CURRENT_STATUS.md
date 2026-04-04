# FoxNIO 当前状态

**更新时间**: 2026-04-04  
**口径**: 只写本次会话已核事实，不把计划写成完成

## 现状

FoxNIO 已经从“核心链路大量 placeholder”推进到“主服务可编译、主要网关链路接真、用户台与管理台关键报表接真、发布口径开始收束”的阶段。

当前可以确认的四个判断：

- backend 与 frontend 主工程都能通过当前关键静态校验。
- 用户 dashboard、`/usage`、管理 dashboard 已走真实后端聚合链。
- Realtime/WebSocket、Gemini Native、Sora / prompt enhance 已接真实转发与失败 usage/audit。
- 当前最大断点已从“功能主链没接上”转为“真实 provider smoke 缺密钥、控制面仍有骨架、部署权威路径刚完成第一轮收口”。

## 已完成

### 代码与业务主链

- `backend/src/service/usage_log.rs` 已接真实 `usages` 表聚合。
- `backend/src/service/dashboard_query_service.rs` + `backend/src/handler/dashboard.rs` + `frontend/src/routes/admin/+page.svelte` 已收成真实 `/api/v1/admin/dashboard/*`。
- `frontend/src/routes/dashboard/+page.svelte` 与 `frontend/src/routes/usage/+page.svelte` 已接真实用户与日维度 usage 数据。
- Realtime/WebSocket 已补 API Key 鉴权、上游转发、request/response 级 audit、成功/失败 usage 回写。
- Gemini Native 已补 OAuth/API Key 凭据解析、retry、typed error、`generateContent`/stream/`countTokens`/`embedContent` usage 回写。
- Sora image/video/status 与 prompt enhance 已接真实 OpenAI HTTP 转发、成功/失败 usage 与 quota 消耗。
- `redeem_code` 已具备事务内写库、幂等键、ledger 审计的主闭环。
- `backup` 已有 export/list/download/delete 的基础能力，`import` 仍未实现。
- 最小 smoke seed 已补：
  - `backend/src/bin/smoke_seed.rs`
  - `backend/src/service/smoke_seed.rs`
  - `scripts/real_provider_smoke.sh`
  - `backend/tests/real_provider_smoke_test.rs`

### 部署主链第一轮收口

- `backend/src/config/mod.rs` 已支持：
  - `config.yaml` / `FOXNIO_CONFIG`
  - `.env`
  - `DATABASE_URL`
  - `REDIS_URL`
  - `JWT_SECRET`
  - `JWT_EXPIRE_HOURS`
  - `GATEWAY_API_KEY_PREFIX`
  - `FOXNIO_SERVER_HOST`
  - `FOXNIO_SERVER_PORT`
- `backend/src/main.rs` 不再硬编码监听地址，已改走 `config.server.bind_addr()`。
- `frontend/Dockerfile` 已从错误的静态 `serve` 改为 `adapter-node` 正确口径：`node build`。
- `docker-compose.yml` 已改成三层结构：
  - 默认 core：`postgres + redis + backend`
  - `ui` profile：`frontend`
  - `edge` profile：`nginx`
- `deploy.sh` 已重写，默认只启动 core 栈，并补 `build-ui` / `start-ui` / `build-edge` / `start-edge`。
- `nginx.conf` 已修正 backend upstream 到 `backend:8080`。
- `backend/.dockerignore` 已补 `**/target/`，backend build context 已从近 1GB 收缩到约 22kB。

### 本次已核验证

- `cargo fmt --manifest-path backend/Cargo.toml --all`
- `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
- `cargo test --manifest-path backend/Cargo.toml config::test::tests:: -- --nocapture`
- `npm --prefix frontend run build`
- `npm --prefix frontend run check`
- `docker compose config`
- `docker build -t foxnio-frontend:test --build-arg VITE_API_URL=http://localhost:8080 ./frontend`

## 未完成

### 真实 provider smoke 仍未闭环

真实 OpenAI / Gemini smoke 本次明确跳过，阻断不是代码，而是输入条件为空：

- 当前无可用真实 `OPENAI_API_KEY`
- 当前无可用真实 `GEMINI_API_KEY` / `GOOGLE_API_KEY`
- 当前无可发布口径的真实 provider account 凭据

因此：

- smoke seed 代码已补
- 真实 smoke 自动化入口已补
- 真上游回归结果仍未产出

### 控制面与产品化仍有缺口

- `backup import` 未实现。
- redeem 仍缺更细粒度运营规则、额度来源解释与完整产品说明。
- `ops_*`、`batch*`、等待队列、调度恢复、快照恢复等模块仍偏骨架。
- Sora 任务持久化、媒体生命周期、回查体验还未产品化。
- 账务 / quota / subscription / redeem 的产品解释层仍未统一。

### 发布链仍待最终回归

- backend Dockerfile 已补 toolchain 与 build 依赖，但完整镜像起服结果仍需最终 `/health` 回归确认。
- 当前发布链已收口到同一套 env/config 语义，但还没做“干净环境从 build 到 up 到 smoke”的完整发布演练。
- 文档体系仍有历史专题文档残留旧完成度描述，需继续清理。

## 下一步

1. 完成 backend release image 构建与 core compose `/health` 回归。
2. 在缺真实密钥的前提下，先把上线链做成可直接接密钥即跑的状态：
   - `.env`
   - compose
   - deploy
   - health
3. 再回到产品主线：
   - backup 正式化
   - redeem 规则与说明
   - 账务 / quota 解释统一
4. 等真实密钥到位后，再跑 OpenAI / Gemini 真 smoke 与 failure integration。
