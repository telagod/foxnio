# FoxNIO

FoxNIO 是一个 Rust + SvelteKit 的 AI gateway / control plane 项目。它统一多家上游模型服务的入口，同时提供用户自助面、管理后台、配额、审计、Webhook、告警与部分运维能力。

## 现状

P0 发布链与 P1 failure chain / 控制面已完成。P2 运维异步能力部分完成。前端已完成 Tailwind v4、ESLint v10、全页面重设计与 401/403 统一处理。

- 主服务链路：dashboard、`/usage`、admin dashboard、Realtime、Gemini Native、Sora / prompt enhance 已接真。
- 部署链路：release image、compose 起服、smoke、回滚手册、环境变量说明均已完成。
- 当前最大断点：真实 provider smoke 缺密钥，P2 运维异步模块仍偏骨架。

真实状态看：

- [docs/CURRENT_STATUS.md](docs/CURRENT_STATUS.md)
- [docs/BUSINESS_LOGIC.md](docs/BUSINESS_LOGIC.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [TODO.md](TODO.md)

## 已验证

- `cargo fmt --manifest-path backend/Cargo.toml --all`
- `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
- `cargo test --manifest-path backend/Cargo.toml config::test::tests:: -- --nocapture`
- `npm --prefix frontend run build`
- `npm --prefix frontend run check`
- `docker compose config`
- `docker build -t foxnio-frontend:test --build-arg VITE_API_URL=http://localhost:8080 ./frontend`
- `./deploy.sh build && ./deploy.sh start` — core 栈起服、`/health` 通过
- `./deploy.sh build-ui && ./deploy.sh start-ui` — frontend Node server 通过
- `./deploy.sh build-edge && ./deploy.sh start-edge` — nginx profile 通过

## 快速开始

### 依赖

- Rust stable
- Node.js 20
- PostgreSQL 16
- Redis 7
- Docker + Docker Compose plugin

### 本地源码运行

```bash
docker compose up -d postgres redis
cp .env.example .env

cargo run --manifest-path backend/Cargo.toml
npm --prefix frontend install
npm --prefix frontend run dev
```

默认源码运行口径：

- backend: `http://localhost:8080`
- frontend dev server: `http://localhost:5173`

### 容器运行

```bash
./deploy.sh build
./deploy.sh start
```

可选层：

```bash
./deploy.sh build-ui
./deploy.sh start-ui

./deploy.sh build-edge
./deploy.sh start-edge
```

## 部署结构

- core：`postgres + redis + backend`
- `ui` profile：`frontend`
- `edge` profile：`nginx`

默认不再把 `frontend` 与 `nginx` 一起强拉起。

## 当前未闭环重点

- 真实 OpenAI / Gemini smoke 仍缺真实密钥与账号输入。
- P2 运维异步能力（batch、scheduler snapshot、token refresh、ops）仍偏骨架。
- 前端页面测试尚未补齐。

## 文档入口

- [当前状态](docs/CURRENT_STATUS.md)
- [架构总览](docs/ARCHITECTURE.md)
- [业务逻辑](docs/BUSINESS_LOGIC.md)
- [部署说明](docs/DEPLOYMENT.md)
- [开发文档](docs/DEVELOPMENT.md)
- [环境变量与回滚](docs/ENV_AND_ROLLBACK.md)
- [后端模块文档](backend/README.md)
- [长期 TODO](TODO.md)

## 许可证

MIT
