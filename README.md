# FoxNIO

FoxNIO 是一个 Rust + SvelteKit 的 AI gateway / control plane 项目。它统一多家上游模型服务的入口，同时提供用户自助面、管理后台、配额、审计、Webhook、告警与部分运维能力。

## 现状

当前仓库已经不是空壳，但还没到 production-ready。

- 主服务链路：dashboard、`/usage`、admin dashboard、Realtime、Gemini Native、Sora / prompt enhance 已接真。
- 部署链路：runtime config、frontend Docker、compose profile、deploy 脚本已完成第一轮收口。
- 当前最大断点：真实 provider smoke 缺密钥，控制面还有骨架，完整上线回归还差最后一轮。

真实状态看：

- [docs/CURRENT_STATUS.md](docs/CURRENT_STATUS.md)
- [docs/BUSINESS_LOGIC.md](docs/BUSINESS_LOGIC.md)
- [TODO.md](TODO.md)

## 已验证

- `cargo fmt --manifest-path backend/Cargo.toml --all`
- `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
- `cargo test --manifest-path backend/Cargo.toml config::test::tests:: -- --nocapture`
- `npm --prefix frontend run build`
- `npm --prefix frontend run check`
- `docker compose config`
- `docker build -t foxnio-frontend:test --build-arg VITE_API_URL=http://localhost:8080 ./frontend`

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
- backup 仍缺 `import`、对象存储、恢复演练。
- redeem 仍缺更细粒度运营规则与产品说明。
- 异步、调度、等待队列、`ops_*` 等模块仍偏骨架。

## 文档入口

- [当前状态](docs/CURRENT_STATUS.md)
- [业务逻辑](docs/BUSINESS_LOGIC.md)
- [部署说明](docs/DEPLOYMENT.md)
- [开发文档](docs/DEVELOPMENT.md)
- [后端模块文档](backend/README.md)
- [长期 TODO](TODO.md)

## 许可证

MIT
