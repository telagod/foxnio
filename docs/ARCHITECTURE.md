# FoxNIO Architecture

## System Overview

```
                         ┌─────────────────────────────────────────────┐
                         │              Client (Browser / CLI)         │
                         └──────────────────┬──────────────────────────┘
                                            │
                                            ▼
                         ┌─────────────────────────────────────────────┐
                         │         Nginx (edge, optional)              │
                         │   TLS termination, rate limit, static       │
                         └──────┬───────────────────────┬──────────────┘
                                │                       │
                    ┌───────────▼──────────┐ ┌──────────▼──────────────┐
                    │  Backend (Rust/Axum) │ │ Frontend (SvelteKit)    │
                    │  :8080              │ │ adapter-node :3000      │
                    └──┬────┬────┬────────┘ └─────────────────────────┘
                       │    │    │
              ┌────────┘    │    └────────┐
              ▼             ▼             ▼
     ┌──────────────┐ ┌──────────┐ ┌───────────────────┐
     │ PostgreSQL   │ │  Redis   │ │ Upstream Providers │
     │ :5432        │ │  :6379   │ │ OpenAI / Anthropic │
     │              │ │          │ │ Gemini / DeepSeek  │
     │ users        │ │ sessions │ │ Mistral / Cohere   │
     │ accounts     │ │ rate lim │ └───────────────────┘
     │ usages       │ │ cache    │
     │ audit_logs   │ │ queues   │
     │ 28 tables    │ └──────────┘
     └──────────────┘
```

## Backend Layers

```
main.rs
  │
  ├── gateway/          ← Protocol adapters & upstream forwarding
  │   ├── routes.rs         Route registration (single source of truth)
  │   ├── chat/             OpenAI-compatible /v1/chat/completions
  │   ├── anthropic/        /v1/messages
  │   ├── gemini/           /v1beta/models/* (Native Gemini API)
  │   ├── websocket/        /v1/realtime (WebSocket proxy)
  │   ├── sora/             /v1/images, /v1/videos, /v1/prompts
  │   └── claude/           TLS fingerprint, headers, validation
  │
  ├── handler/          ← HTTP request handlers
  │   ├── auth/             Register, login, refresh, TOTP, password reset
  │   ├── user.rs           User profile, API keys
  │   ├── admin.rs          Admin CRUD (users, accounts, groups)
  │   ├── dashboard.rs      Admin dashboard aggregation endpoints
  │   ├── alerts.rs         Alert rules, channels, history
  │   └── ...               Quota, redeem, backup, audit, etc.
  │
  ├── service/          ← Business logic
  │   ├── billing.rs        Usage recording + balance ledger
  │   ├── usage_log.rs      Usage insert/query on `usages` table
  │   ├── dashboard_query_service.rs   Admin dashboard aggregation
  │   ├── api_key.rs        API key CRUD + Redis rate limiting
  │   ├── scheduler.rs      Account selection & model routing
  │   ├── redeem_code.rs    Redeem with idempotency + ledger
  │   ├── backup.rs         Export / import
  │   └── ...               50+ service files
  │
  ├── entity/           ← SeaORM entities (28 tables)
  ├── migration/        ← Database migrations (24 files)
  ├── config/           ← Config loading (yaml / env / runtime)
  ├── health/           ← Health check endpoints
  ├── metrics/          ← Prometheus metrics
  └── utils/            ← Crypto, JWT, hashing, validation
```

## Key Data Flow

### API Request (e.g. /v1/chat/completions)

```
Client request
  → API Key auth (middleware)
  → Redis RPM / concurrency check
  → Scheduler: select upstream account
  → Forward to provider (OpenAI / Anthropic / Gemini / ...)
  → Record usage to `usages` table
  → Deduct balance via `balance_ledger`
  → Write audit log
  → Return response to client
```

### User Dashboard

```
Browser → GET /api/v1/user/me          → user info
        → GET /api/v1/user/apikeys     → API key list
        → GET /api/v1/user/usage       → 30-day usage aggregation
```

### Admin Dashboard

```
Browser → GET /api/v1/admin/dashboard/stats              → summary counts
        → GET /api/v1/admin/dashboard/trend               → daily trend
        → GET /api/v1/admin/dashboard/line                → latency trend
        → GET /api/v1/admin/dashboard/pie                 → outcome distribution
        → GET /api/v1/admin/dashboard/model-distribution  → model breakdown
        → GET /api/v1/admin/dashboard/platform-distribution → platform breakdown
```

## Deployment Topology

```
docker-compose.yml profiles:

  core (default):   postgres + redis + backend
  ui:               frontend (SvelteKit/Node)
  edge:             nginx (TLS, reverse proxy)

Entry point:  ./deploy.sh build|start|build-ui|start-ui|build-edge|start-edge
```

## Key Endpoints

| Category | Path | Auth |
|----------|------|------|
| Health | `GET /health` | none |
| Models | `GET /v1/models` | API Key |
| Chat | `POST /v1/chat/completions` | API Key |
| Messages | `POST /v1/messages` | API Key |
| Gemini | `POST /v1beta/models/{model}` | API Key |
| Realtime | `GET /v1/realtime` | API Key (WS) |
| Auth | `POST /api/v1/auth/login` | none |
| User | `GET /api/v1/user/me` | JWT |
| Usage | `GET /api/v1/user/usage` | JWT |
| Admin | `GET /api/v1/admin/dashboard/stats` | JWT + admin |

Full route map: `docs/API_REFERENCE.md`

## Configuration

```
Priority: env vars > .env > config.yaml > defaults

Required:  JWT_SECRET, FOXNIO_MASTER_KEY, DATABASE_URL, REDIS_URL
Optional:  JWT_EXPIRE_HOURS, GATEWAY_API_KEY_PREFIX, FOXNIO_SERVER_HOST/PORT
Frontend:  VITE_API_URL (build-time)
```

Details: `docs/ENV_AND_ROLLBACK.md`
