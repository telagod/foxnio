# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-03-30

### Added

#### Core Features

**Webhook System**
- Complete webhook service with HMAC-SHA256 signature
- Exponential backoff retry mechanism (1s, 2s, 4s, 8s, 16s)
- 12 webhook event types (account, api_key, quota, payment, model events)
- 7 REST endpoints for webhook management
- Delivery tracking and retry queue
- HTTPS URL validation

**Batch Operations**
- Batch create API keys with transaction support
- Batch update accounts with error aggregation
- CSV user import with validation
- Batch delete operations
- Stop-on-error option for transactions
- Comprehensive error reporting

**Window Cost Cache**
- Dual-layer caching (Memory + Redis)
- Batch SQL query optimization
- 60s TTL automatic expiration
- Cache hit/miss metrics
- Prefetch optimization for multiple accounts

**API Key Permissions**
- Model access control with whitelist
- IP whitelist validation
- Daily quota management and tracking
- Expiration time enforcement
- Complete permission verification middleware

**Rate Limiting & Queue**
- Model-level RPM/TPM limits
- Waiting queue with sticky session priority
- Automatic expired request cleanup
- Redis-backed rate limiting
- Memory cache optimization

**OpenAPI Documentation**
- Swagger UI integration (/swagger-ui)
- OpenAPI spec endpoint (/api-docs/openapi.json)
- 15+ endpoints with full annotations
- Request/response schemas with ToSchema
- Authentication documentation

#### Monitoring & Metrics

**Prometheus Metrics (15+ new metrics)**
- Webhook: events_sent, delivery_success, delivery_failed, retry_count
- Batch: operations_total, items_processed, errors
- API Key: auth_checks, quota_exceeded, model_denied
- Window Cost: cache_hits, cache_misses, batch_queries, redis_hits/misses

**Integration**
- All new endpoints integrated into routing
- JWT authentication middleware
- Admin permission checks
- No conflicts with existing routes

### Technical Details

#### New Files Created
- backend/src/service/webhook.rs (20,309 bytes)
- backend/src/service/batch.rs (13,698 bytes)
- backend/src/service/window_cost_cache.rs (15,224 bytes)
- backend/src/service/wait_queue.rs (12,970 bytes)
- backend/src/service/model_rate_limit.rs (14,154 bytes)
- backend/src/handler/webhook.rs (12,895 bytes)
- backend/src/handler/batch.rs (7,881 bytes)
- backend/src/middleware/api_key_auth.rs (complete implementation)
- backend/src/entity/webhook_endpoints.rs
- backend/src/entity/webhook_deliveries.rs
- backend/migration/src/m20240330_000024_add_api_key_permissions.rs
- backend/migration/src/m20240330_000026_create_webhook_endpoints.rs
- backend/migration/src/m20240330_000027_create_webhook_deliveries.rs

#### Code Statistics
- New code lines: 7,000+
- Total commits: 16
- Parallel agents: 6 rounds, 24 agents
- Development time: 6.5 hours

#### API Endpoints Added
- Webhook endpoints (7): /api/v1/webhooks/*
- Batch operation endpoints (4): /api/v1/admin/*/batch-*
- Total new endpoints: 11

### Improved

#### Documentation
- Removed temporary design documents
- Cleaned up obsolete planning files
- Updated memory files with session summaries
- Maintained alignment documentation for reference

### Feature Alignment

**Feature Alignment Status**
| Feature | Status |
|---------|--------|
| Waiting Queue | ✅ 100% |
| Model-level RPM | ✅ 100% |
| API Key Permissions | ✅ 100% |
| Webhook System | ✅ 100% |
| Batch Operations | ✅ 100% |
| Window Cost Prefetch | ✅ 100% |
| OpenAPI Documentation | ✅ 100% |
| Sticky Sessions | ✅ 100% |
| Model Routing | ✅ 100% |
| Monitoring Metrics | ✅ 100% |

### Known Issues

- Cost optimization service (40% implemented, needs core logic)
- Model sync service (40% implemented, needs provider APIs)
- Integration tests (structure ready, needs test cases)

### Future Plans

**Short-term (this week)**
- Complete cost optimization recommendations
- Complete model auto-sync service
- Add comprehensive integration tests
- Performance benchmarking

**Mid-term (next week)**
- Production deployment
- Monitoring alert configuration
- User documentation updates
- Feedback collection

---

## [0.2.0] - 2026-03-29

### Added

#### Core Gateway Features
- OpenAI compatible API endpoints (`/v1/chat/completions`, `/v1/models`)
- Multi-provider support (OpenAI, Anthropic, Google, DeepSeek, Mistral, Cohere)
- Intelligent model routing with alias resolution
- Automatic failover with exponential backoff
- SSE streaming response support
- Request proxying and forwarding

#### Authentication & Authorization
- User registration and login
- JWT-based authentication
- Token refresh mechanism
- TOTP two-factor authentication
- Password reset via email
- OAuth integration (GitHub, Google, LinuxDo, Antigravity)
- Role-based access control (RBAC)

#### API Key Management
- API key creation and deletion
- Permission management
- Rate limiting per key
- Usage statistics tracking

#### Billing & Quota
- Subscription management system
- Quota control per user/group
- Usage history tracking
- Promo code system
- Redemption code system
- Automatic billing

#### Monitoring & Alerting
- Prometheus metrics collection
- Health checks (PostgreSQL, Redis, disk, memory)
- Alert rules engine
- Multi-channel alerting (Email, Slack, DingTalk, Feishu)
- Audit logging
- Real-time monitoring via WebSocket

#### Management Features
- User management (admin)
- Account management (AI provider accounts)
- Model configuration
- Group management
- Announcement system
- Backup and restore

#### Security Features
- AES-256-GCM data encryption
- TLS fingerprint recognition
- Distributed rate limiting
- Role permission control

#### System Features
- HTTP/2 support
- WebSocket real-time push
- Response compression (gzip, brotli)
- Connection pool optimization
- Redis caching

### Technical Details

#### Backend
- 158 Rust source files
- 36,438+ lines of code
- 328+ test cases
- 24 database migrations
- 31 entity definitions
- 54 service modules
- 29 HTTP handlers

#### Database
- 28 database tables
- PostgreSQL 16 support
- Sea-ORM integration
- Redis caching layer

#### API
- 49 API endpoints
- OpenAI compatible API
- RESTful admin API
- WebSocket API

#### Infrastructure
- Docker support
- Docker Compose orchestration
- Nginx reverse proxy
- Prometheus monitoring
- Grafana dashboard

### Documentation
- Project Overview
- API Reference
- Module Reference
- Database Schema
- Deployment Guide
- Development Guide

---

## [0.1.0] - 2026-03-20

### Added

#### Core Gateway Features
- Basic gateway functionality with OpenAI compatible API
- Request forwarding and response handling
- Docker support for easy deployment

#### Multi-Model Proxy Support
- OpenAI API integration
- Anthropic API integration
- Google AI API integration
- DeepSeek API integration
- Mistral API integration
- Cohere API integration
- Model routing and alias support

#### Account Scheduling System
- Multi-account management
- Intelligent account scheduling
- Automatic failover mechanism
- Load balancing across accounts

#### Usage Billing System
- Usage tracking and statistics
- Quota management per user
- Billing history and reports
- Cost calculation engine

#### OAuth Integration
- GitHub OAuth support
- Google OAuth support
- LinuxDo OAuth support
- Antigravity OAuth support
- Secure authentication flow

#### Management Dashboard
- User management interface
- Account configuration UI
- Model management panel
- Usage statistics dashboard
- Real-time monitoring interface

#### Security & Infrastructure
- JWT-based authentication
- API key management
- Basic monitoring setup
- PostgreSQL database integration
- Redis caching layer

---

[0.2.1]: https://github.com/telagod/foxnio/releases/tag/v0.2.1
[0.2.0]: https://github.com/telagod/foxnio/releases/tag/v0.2.0
[0.1.0]: https://github.com/telagod/foxnio/releases/tag/v0.1.0
