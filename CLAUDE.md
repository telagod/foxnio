# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is FoxNIO

FoxNIO is a high-performance AI API Gateway / Account Pool Control Plane. Rust backend (Axum) handles the proxy hot path; SvelteKit frontend serves the ops control plane. Core differentiators: bulk account import, pool scheduling, quota governance, audit trail.

## Commands

### Build & Run (local source)

```bash
# Start infra
docker compose up -d postgres redis

# Backend
cargo run --manifest-path backend/Cargo.toml

# Frontend
npm --prefix frontend install
npm --prefix frontend run dev
```

Backend listens on `http://localhost:8080`, frontend dev server on `http://localhost:5173`.

### Build (release)

```bash
cd backend && cargo build --release
cd frontend && npm run build
```

### Tests

```bash
# All
make test

# Backend only
cd backend && cargo test

# Single backend test (by name filter)
cd backend && cargo test config::test::tests:: -- --nocapture

# Integration / E2E
cd backend && cargo test --test integration_test
cd backend && cargo test --test full_e2e_test

# Frontend only (single run, not watch)
cd frontend && npx vitest --run

# Frontend with coverage
cd frontend && npm run test:coverage
```

Backend tests that need DB require Postgres (`foxnio`/`foxnio` on 5432) and Redis (6379). `docker compose up -d postgres redis` provides both.

### Lint & Format

```bash
# Backend
cd backend && cargo fmt
cd backend && cargo clippy -- -D warnings

# Frontend
cd frontend && npm run lint
cd frontend && npm run check   # svelte-check + tsc
```

CI clippy flags: `-W clippy::correctness -W clippy::suspicious -W clippy::perf -A clippy::style -A clippy::pedantic -A clippy::nursery`

### Database Migrations

```bash
# Run migrations (SeaORM CLI)
cargo run --manifest-path backend/migration/Cargo.toml -- up

# Rollback
cargo run --manifest-path backend/migration/Cargo.toml -- down

# Reset
cargo run --manifest-path backend/migration/Cargo.toml -- reset
```

### Docker (full stack)

```bash
./deploy.sh build && ./deploy.sh start          # core: postgres + redis + backend
./deploy.sh build-ui && ./deploy.sh start-ui    # frontend
./deploy.sh build-edge && ./deploy.sh start-edge # nginx TLS edge
```

## Architecture

### Backend (`backend/`)

Rust, Axum, Tokio. Layered as:

- **gateway/** — proxy hot path. Submodules: `claude`, `gemini`, `sora`, `claude_shell`, `failover`, `scheduler`, `stream` (SSE), `websocket`, `waiting_queue`, `request_rectifier`, `responses`/`responses_handler`/`responses_converter`, `providers`, `models`, `middleware` (JWT auth, compression, CORS, permission, request logging), `proxy`. Entry: `routes::build_app()` constructs the full Axum router.
- **handler/** — HTTP handlers organized by domain: `admin`, `auth`, `batch`, `health`, `metrics`, `models`, `proxy`, `quota`, `subscription`, `user`, `webhook`, etc. Defines `ApiError`.
- **service/** — business logic: `AccountService`, `ApiKeyService`, `BillingService`, `ModelRouter`, `SchedulerService`, `UserService`, OAuth, idempotency, refresh policies, gateway forwarders, etc.
- **entity/** — SeaORM entities (users, accounts, api_keys, usages, groups, model_configs, subscriptions, promo_codes, alert_rules, etc.).
- **db/** — Postgres pool (SQLx + SeaORM `DatabaseConnection`) and Redis pool (with local LRU cache).
- **config/** — loads from `config.yaml` + `.env` + env vars. Supports both `FOXNIO_*` and legacy variable names.
- **state.rs** — `AppState` holds DB, Redis, Config, AlertManager. Shared as `Arc<AppState>`.
- **migration/** — separate crate (`foxnio-migration`), SeaORM migrations run via CLI.

### Frontend (`frontend/`)

SvelteKit, Svelte 5 (runes: `$state`, `$derived`, `$props`), Tailwind CSS v4, adapter-node.

- Route groups: `(public)/` landing, `(auth)/` login/register, `(app)/` authenticated shell (sidebar + dashboard + admin).
- API client: `src/lib/api.ts` — hits `VITE_API_URL` (default `http://localhost:8080`).
- Tests: Vitest + `@testing-library/svelte/svelte5`, files in `src/__tests__/`.

### Config

Backend config resolution order: `FOXNIO_CONFIG` path → `config.yaml` → `.env` → environment variables. See `.env.example` and `config.example.yaml` for all knobs.

### API Shape

OpenAI-compatible gateway at `/v1/` (e.g., `/v1/models`, `/v1/chat/completions`). Management API at `/api/v1/` (auth, admin, users, accounts, billing, webhooks). Health at `/health`. Metrics at `/metrics`.

### CI

GitHub Actions (`.github/workflows/ci.yml`): lint-backend → test-backend (with Postgres+Redis services) → build; lint-frontend → test-frontend → build. Plus security audit and quality gate jobs. Coverage uploaded to Codecov.
