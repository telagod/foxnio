# FoxNIO

FoxNIO 是一个面向大规模号池运营的 **高性能 AI API Gateway / Control Plane**。它用 Rust 承载代理热路径，用 SvelteKit 承载运营控制面，目标不是只做“多模型转发”，而是把 **批量导入、号池调度、配额治理、审计追踪** 压成一套可运营产品。

## 现状

P0 发布链与 P1 failure chain / 控制面已完成。P2 运维异步能力部分完成。前端已完成 Tailwind v4、ESLint v10、全页面重设计与 401/403 统一处理。

当前北极星：

- **对齐 Sub2API**：补齐多账号管理、计费、调度、控制台与原生工具接入。
- **吸收 LiteLLM**：补 router policy、观测、预算与 fallback 经验。
- **走 FoxNIO 自己的差异化**：把批量操作性能与大规模号池运营做成核心卖点，而不是附属功能。

当前对外品牌口径：

- **FoxNIO = 高性能 AI API Gateway / Account Pool Control Plane**
- **不是** 只做多模型 relay，而是把「大规模号池调度 + 批量运营 + 可解释观测」收成同一产品
- 对外优先讲三件事：**批量导入快、号池调度稳、运营解释清**

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
- 大批量账号导入、批量凭证轮换、批量状态治理仍需继续压性能和回显质量。
- 批量基准与 Prometheus 观测已开始收口，执行口径见 `docs/BENCHMARKS.md`。
- 对标项目已补一轮扫描，当前主参考为 Sub2API / Plexus / Ferro Labs AI Gateway / labring aiproxy。

## 文档入口

- [当前状态](docs/CURRENT_STATUS.md)
- [架构总览](docs/ARCHITECTURE.md)
- [业务逻辑](docs/BUSINESS_LOGIC.md)
- [进化路线](docs/EVOLUTION_TRACK_2026-04.md)
- [性能基准](docs/BENCHMARKS.md)
- [部署说明](docs/DEPLOYMENT.md)
- [开发文档](docs/DEVELOPMENT.md)
- [环境变量与回滚](docs/ENV_AND_ROLLBACK.md)
- [后端模块文档](backend/README.md)
- [长期 TODO](TODO.md)

## 许可证

MIT
