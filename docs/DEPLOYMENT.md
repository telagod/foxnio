# FoxNIO 部署说明

## 现状

本轮已把部署口径从“散装资产”推进到“同一套 runtime config + Docker + compose + deploy 脚本”。

当前权威发布结构：

- core：`postgres + redis + backend`
- `ui` profile：`frontend`
- `edge` profile：`nginx`

默认只启动 core，不再默认把 `frontend` 与 `nginx` 一起拉起。

## 已完成

### Runtime 配置统一

- backend 现在同时支持：
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
- `backend/src/main.rs` 已尊重配置，不再硬编码 `0.0.0.0:8080`。

### Docker 与 compose 收口

- `frontend/Dockerfile` 已修正为 `@sveltejs/adapter-node` 正确运行方式：`node build`。
- `docker-compose.yml` 已改为 profile 模式：
  - 默认 core 服务可直接 `docker compose up -d postgres redis backend`
  - `frontend` 通过 `--profile ui`
  - `nginx` 通过 `--profile edge`
- `nginx.conf` 已修正 backend upstream 到 `backend:8080`。
- `backend/.dockerignore` 已补 `**/target/`，避免把本地 build 垃圾带进镜像上下文。

### 脚本收口

`deploy.sh` 当前命令：

- `./deploy.sh build`
- `./deploy.sh start`
- `./deploy.sh build-ui`
- `./deploy.sh start-ui`
- `./deploy.sh build-edge`
- `./deploy.sh start-edge`

默认 `start` 只启动 core 栈。

## 未完成

- backend release image 的最终构建与起服回归还要跑完。
- 真实 provider smoke 仍因真实密钥缺失而跳过。
- `nginx` 仍只是可选边缘层，不应在未验证证书/域名/反代策略前当成默认生产入口。
- `backup import`、对象存储、恢复演练等仍未正式化。

## 下一步

### 最小上线顺序

1. `./deploy.sh build`
2. `./deploy.sh start`
3. 校验 `http://localhost:8080/health`
4. 若需要控制台：
   - `./deploy.sh build-ui`
   - `./deploy.sh start-ui`
5. 若需要反代：
   - 准备 `ssl/cert.pem` 与 `ssl/key.pem`
   - `./deploy.sh build-edge`
   - `./deploy.sh start-edge`

### 最小验证集

- `GET /health`
- `GET /v1/models`
- 一条认证接口
- `GET /api/v1/user/usage`
- `GET /api/v1/admin/dashboard/stats`

## 环境变量

常用项：

- `DATABASE_URL`
- `REDIS_URL`
- `JWT_SECRET`
- `JWT_EXPIRE_HOURS`
- `GATEWAY_API_KEY_PREFIX`
- `FOXNIO_MASTER_KEY`
- `FOXNIO_SERVER_HOST`
- `FOXNIO_SERVER_PORT`
- `VITE_API_URL`
- `RUST_LOG`

## 说明

- `frontend/src/lib/api.ts` 读取的是 `VITE_API_URL`，这是 build-time 变量，不是 runtime 变量。
- `deploy.sh` 会在缺少 `.env` 时从 `.env.example` 生成一份最小可启动配置。
- backend 运行时仍会自动执行 SeaORM migration；发布前应保证数据库可接受该路径。
