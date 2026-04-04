# FoxNIO 开发文档

**更新时间**: 2026-04-04
**口径**: 本文档只保留当前仓库可用的开发路径与命令，不再沿用旧目录结构、旧 Make 目标、旧迁移方式

## 现状

FoxNIO 当前的开发方式分两条：

- 源码开发：本地启动 PostgreSQL / Redis，直接运行 backend 与 frontend
- 容器开发：通过 `deploy.sh` 或 `docker compose` 起 `core` / `ui` / `edge`

当前建议优先使用源码开发调试业务逻辑，使用容器开发验证部署路径。

## 已完成

### 当前可用开发环境

- Rust stable
- Node.js 20+
- PostgreSQL 16+
- Redis 7+
- Docker + Docker Compose plugin

### 当前有效目录认知

```text
foxnio/
├── backend/
│   ├── src/
│   │   ├── gateway/
│   │   ├── handler/
│   │   ├── service/
│   │   ├── entity/
│   │   ├── config/
│   │   ├── db/
│   │   ├── health/
│   │   ├── metrics/
│   │   └── utils/
│   ├── migration/
│   └── tests/
├── frontend/
│   ├── src/
│   └── package.json
├── docs/
├── docker-compose.yml
├── deploy.sh
└── Makefile
```

### 当前有效开发命令

backend：

```bash
cargo run --manifest-path backend/Cargo.toml
```

frontend：

```bash
npm --prefix frontend install
npm --prefix frontend run dev
```

静态检查：

```bash
cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short
npm --prefix frontend run check
```

可用的 Make 入口：

```bash
make dev
make run
make run-frontend
make build
make test
```

## 未完成

### 不再推荐或不再作为权威路径的做法

- 文档中旧的 `make run-backend`
- 文档中旧的 `make migrate`
- 文档中旧的不存在文件引用
- 旧的目录结构说明

### 当前开发链仍有现实缺口

- frontend `eslint` 仍会扫到 `frontend/build/**`
- 真实 provider smoke 仍缺密钥与账号输入
- control plane 仍有若干骨架模块，开发时要以当前状态文档为准，不要假设所有模块都已闭环

## 下一步

### 源码开发最小顺序

1. 准备 `.env`

```bash
cp .env.example .env
```

2. 启动依赖

```bash
docker compose up -d postgres redis
```

3. 启动 backend

```bash
cargo run --manifest-path backend/Cargo.toml
```

4. 启动 frontend

```bash
npm --prefix frontend install
npm --prefix frontend run dev
```

默认访问：

- backend: `http://localhost:8080`
- frontend dev server: `http://localhost:5173`

### 容器开发最小顺序

```bash
./deploy.sh build
./deploy.sh start
```

若需要 UI：

```bash
./deploy.sh build-ui
./deploy.sh start-ui
```

### 推荐验证命令

```bash
curl http://localhost:8080/health
cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short
npm --prefix frontend run check
```

## 说明

- backend 启动时会自动执行 migration，当前开发文档不再把手工 migration 命令写成权威主路径。
- `frontend/src/lib/api.ts` 使用的是 `VITE_API_URL`，这是 frontend build-time 变量。
- 当前项目状态请优先看 [`CURRENT_STATUS.md`](./CURRENT_STATUS.md) 与 [`BUSINESS_LOGIC.md`](./BUSINESS_LOGIC.md)。
