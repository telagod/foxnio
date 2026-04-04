# FoxNIO 部署说明

**更新时间**: 2026-04-04
**口径**: 只写当前仓库已核对的部署路径，不把“计划中的生产方案”写成已完成

## 现状

FoxNIO 当前的部署主线已经从“脚本、Docker、compose、配置各说各话”收口到一套统一路径：

- backend 是主服务，默认运行在 `:8080`
- 数据层默认是 `postgres + redis`
- frontend 通过 `@sveltejs/adapter-node` 运行，不再走静态站点假设
- compose 已按 `core` / `ui` / `edge` 三层拆分
- `deploy.sh` 已成为当前仓库里的统一发布入口

当前权威部署结构：

- `core`: `postgres + redis + backend`
- `ui` profile: `frontend`
- `edge` profile: `nginx`

默认只启动 `core`，不再默认把 `frontend` 与 `nginx` 一起拉起。

## 已完成

### 配置口径统一

backend 当前已统一支持以下配置来源：

- `config.yaml`
- `FOXNIO_CONFIG`
- `.env`
- `DATABASE_URL`
- `REDIS_URL`
- `JWT_SECRET`
- `JWT_EXPIRE_HOURS`
- `GATEWAY_API_KEY_PREFIX`
- `FOXNIO_SERVER_HOST`
- `FOXNIO_SERVER_PORT`
- `FOXNIO_MASTER_KEY`

已确认：

- `backend/src/main.rs` 已走 `config.server.bind_addr()`
- backend 不再硬编码 `0.0.0.0:8080`

### Docker 与 compose 收口

- `frontend/Dockerfile` 已修正为 `adapter-node` 口径，运行方式是 `node build`
- `docker-compose.yml` 已按 profile 分层
- `nginx.conf` backend upstream 已改为 `backend:8080`
- `backend/.dockerignore` 已补 `**/target/`

### 部署脚本收口

`deploy.sh` 当前命令：

- `./deploy.sh build`
- `./deploy.sh start`
- `./deploy.sh build-ui`
- `./deploy.sh start-ui`
- `./deploy.sh build-edge`
- `./deploy.sh start-edge`

当前约定：

- `start` 只启动 `core`
- 控制台按需启动 `ui`
- 反代按需启动 `edge`

### 当前已核验证

- `docker compose config`
- backend `cargo check`
- frontend `npm run check`

## 未完成

### 发布链仍缺最终演练

还没有完成一轮“干净环境从构建到起服到 smoke”的完整演练：

- backend release image 最终构建结果还需固化
- `core` 起服后的 `/health` 回归还需写入正式记录
- `ui` 与 `edge` 组合启动还未形成权威回归结论

### 真实 provider smoke 仍缺输入条件

当前没有真实 OpenAI / Gemini 密钥与账号输入，所以还不能把部署文档写成“上线后 provider 已验证可用”。

### 边缘层仍不是默认生产口径

- `nginx` 目前只是可选边缘层
- TLS、证书、域名、反代策略还没形成权威生产方案
- 不应把 `edge` 层写成默认必须路径

### 工程口径仍有残缺

- frontend `eslint` 当前会扫到 `frontend/build/**` 产物
- 还缺最小 smoke 脚本与回滚说明
- 还缺生产环境变量最小集合与推荐值说明

## 下一步

### 最小上线顺序

1. `./deploy.sh build`
2. `./deploy.sh start`
3. 校验 `http://localhost:8080/health`
4. 若需要控制台：
5. `./deploy.sh build-ui`
6. `./deploy.sh start-ui`
7. 若需要反代：
8. 准备 `ssl/cert.pem` 与 `ssl/key.pem`
9. `./deploy.sh build-edge`
10. `./deploy.sh start-edge`

### 最小验证集

- `GET /health`
- `GET /v1/models`
- 一条认证接口
- `GET /api/v1/user/usage`
- `GET /api/v1/admin/dashboard/stats`

### 建议补齐项

1. 固化 backend release image 构建结果
2. 固化 `core` / `ui` / `edge` 的最小 smoke 记录
3. 输出最小回滚手册
4. 输出环境变量权威说明

## 说明

- `frontend/src/lib/api.ts` 读取的是 `VITE_API_URL`，这是 build-time 变量，不是 backend runtime 变量。
- `deploy.sh` 会在缺少 `.env` 时从 `.env.example` 生成最小配置。
- backend 运行时仍会自动执行 migration；发布前应确认数据库能接受该路径。
