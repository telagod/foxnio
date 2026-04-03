# FoxNIO API 路由索引

## 现状

本文件是人工维护的路由地图，不是 OpenAPI 契约。

- `backend/src/gateway/routes.rs` 是当前真实路由入口。
- Swagger UI 仍未启用，不能把旧示例文档当成权威接口说明。
- 本文件只记录“源码中真实挂载的路由”和“已经确认的占位口”。

默认源码运行地址参考：`http://localhost:8080`

### 认证口径

- 用户体系主要依赖 JWT。
- 网关调用主要依赖 API Key。
- 管理端接口通常依赖 JWT + permission middleware。

## 已完成

### 已验证闭环接口

- `GET /api/v1/user/usage?days=N`
  返回 `days`、`total_requests`、`total_input_tokens`、`total_output_tokens`、`total_tokens`、`total_cost`、`total_cost_yuan`、`daily_usage[]`。
- `GET /api/v1/admin/dashboard/stats`
  返回管理总览：`users`、`accounts`、`api_keys`、`usage`、`updated_at`。
- `GET /api/v1/admin/dashboard/trend`
  支持 `start_date`、`end_date`，返回趋势图 `ChartData`。
- `GET /api/v1/admin/dashboard/line`
  支持 `start_date`、`end_date`，返回折线图 `ChartData`。
- `GET /api/v1/admin/dashboard/pie`
  返回请求结果分布 `ChartData`。
- `GET /api/v1/admin/dashboard/model-distribution`
  返回模型分布 `{ labels, data, total }`。
- `GET /api/v1/admin/dashboard/platform-distribution`
  返回平台分布 `{ labels, data, total }`。
- `GET /api/v1/admin/stats`
  兼容接口，当前复用 dashboard 聚合服务，返回近 30 天 usage 汇总。
- `GET /api/v1/admin/dashboard`
  兼容接口，当前复用 dashboard 聚合服务，返回近 7 天兼容视图。

### 路由地图

#### 公开路由

- `GET /health`
- `GET /health/live`
- `GET /health/ready`
- `GET /health/detailed`
- `GET /health/resources`
- `GET /health/database`
- `GET /health/redis`
- `GET /health/info`
- `GET /metrics`
- `GET /v1/models`
- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `POST /api/v1/auth/send-verify-code`
- `POST /api/v1/auth/validate-promo-code`
- `POST /api/v1/auth/validate-invitation-code`
- `POST /api/v1/auth/totp/login`
- `POST /api/v1/auth/totp/backup-login`
- `POST /api/v1/auth/password/reset-request`
- `POST /api/v1/auth/password/verify-token`
- `POST /api/v1/auth/password/reset`

#### 用户 / 已认证路由

- `GET /api/v1/user/me`
- `GET /api/v1/user/usage`
- `PUT /api/v1/user`
- `PUT /api/v1/user/password`
- `GET /api/v1/users/me/audit-logs`
- `POST /api/v1/auth/totp/enable`
- `POST /api/v1/auth/totp/confirm`
- `POST /api/v1/auth/totp/disable`
- `POST /api/v1/auth/totp/verify`
- `GET /api/v1/auth/totp/status`
- `POST /api/v1/auth/totp/backup-codes/regenerate`
- `GET /api/v1/user/apikeys`
- `POST /api/v1/user/apikeys`
- `PUT /api/v1/user/apikeys/:id`
- `DELETE /api/v1/user/apikeys/:id`
- `GET /api/v1/groups/available`
- `GET /api/v1/groups/rates`
- `GET /api/v1/announcements`
- `GET /api/v1/subscriptions`
- `GET /api/v1/subscriptions/:id`
- `POST /api/v1/redeem`
- `GET /api/v1/redeem/history`
- `GET /api/v1/user/attributes`
- `POST /api/v1/user/attributes`

#### 网关 / Provider 路由

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/messages`
- `POST /v1/responses`
- `GET /v1/realtime`
- `GET /v1/responses`
- `GET /v1beta/models`
- `GET /v1beta/models/{model}`
- `POST /v1beta/models/{model}`
- `POST /v1beta/models/{model}/countTokens`
- `POST /v1beta/models/{model}/embedContent`
- `POST /v1/images/generations`
- `POST /v1/videos/generations`
- `GET /v1/videos/generations/:id`
- `POST /v1/prompts/enhance`
- `GET /v1/sora/models`
- `GET /v1/sora/families`

#### Webhook / 配额 / 运营侧公共路由

- `POST /api/v1/webhooks`
- `GET /api/v1/webhooks`
- `GET /api/v1/webhooks/:id`
- `PUT /api/v1/webhooks/:id`
- `DELETE /api/v1/webhooks/:id`
- `POST /api/v1/webhooks/:id/test`
- `GET /api/v1/webhooks/:id/deliveries`
- `GET /api/v1/quota`
- `POST /api/v1/quota`

#### 管理路由

- `GET /api/v1/admin/users`
- `POST /api/v1/admin/users`
- `GET /api/v1/admin/users/:id`
- `PUT /api/v1/admin/users/:id`
- `DELETE /api/v1/admin/users/:id`
- `POST /api/v1/admin/users/:id/balance`
- `GET /api/v1/admin/accounts`
- `POST /api/v1/admin/accounts`
- `POST /api/v1/admin/accounts/batch`
- `GET /api/v1/admin/accounts/:id`
- `PUT /api/v1/admin/accounts/:id`
- `DELETE /api/v1/admin/accounts/:id`
- `POST /api/v1/admin/accounts/test`
- `POST /api/v1/admin/accounts/:id/refresh`
- `POST /api/v1/admin/accounts/:id/recover-state`
- `POST /api/v1/admin/accounts/:id/set-privacy`
- `POST /api/v1/admin/accounts/:id/refresh-tier`
- `POST /api/v1/admin/accounts/:id/clear-error`
- `GET /api/v1/admin/accounts/:id/usage`
- `GET /api/v1/admin/accounts/:id/today-stats`
- `POST /api/v1/admin/accounts/today-stats/batch`
- `POST /api/v1/admin/accounts/:id/clear-rate-limit`
- `POST /api/v1/admin/accounts/:id/reset-quota`
- `GET /api/v1/admin/accounts/data`
- `POST /api/v1/admin/accounts/data`
- `POST /api/v1/admin/accounts/batch-update-credentials`
- `POST /api/v1/admin/accounts/batch-refresh-tier`
- `GET /api/v1/admin/apikeys`
- `GET /api/v1/admin/stats`
- `GET /api/v1/admin/dashboard`
- `GET /api/v1/admin/dashboard/stats`
- `GET /api/v1/admin/dashboard/trend`
- `GET /api/v1/admin/dashboard/line`
- `GET /api/v1/admin/dashboard/pie`
- `GET /api/v1/admin/dashboard/model-distribution`
- `GET /api/v1/admin/dashboard/platform-distribution`
- `GET /api/v1/admin/metrics`
- `GET /api/v1/admin/metrics/detail`
- `GET /api/v1/admin/metrics/health`
- `GET /api/v1/admin/metrics/realtime`
- `GET /api/v1/admin/metrics/cost`
- `GET /api/v1/admin/metrics/tokens`
- `GET /api/v1/admin/metrics/accounts`
- `GET /api/v1/admin/permissions/matrix`
- `GET /api/v1/admin/roles`
- `GET /api/v1/admin/audit-logs`
- `GET /api/v1/admin/audit-logs/stats`
- `GET /api/v1/admin/audit-logs/sensitive`
- `GET /api/v1/admin/audit-logs/users/:user_id`
- `POST /api/v1/admin/audit-logs/cleanup`
- `GET /api/v1/admin/alerts/rules`
- `POST /api/v1/admin/alerts/rules`
- `PUT /api/v1/admin/alerts/rules/:id`
- `DELETE /api/v1/admin/alerts/rules/:id`
- `GET /api/v1/admin/alerts/silences`
- `POST /api/v1/admin/alerts/silences`
- `DELETE /api/v1/admin/alerts/silences/:id`
- `GET /api/v1/admin/alerts/history`
- `GET /api/v1/admin/alerts/stats`
- `GET /api/v1/admin/alerts/channels`
- `POST /api/v1/admin/alerts/channels`
- `DELETE /api/v1/admin/alerts/channels/:id`
- `POST /api/v1/admin/alerts/test`
- `GET /api/v1/admin/models`
- `POST /api/v1/admin/models`
- `GET /api/v1/admin/models/:id`
- `PUT /api/v1/admin/models/:id`
- `DELETE /api/v1/admin/models/:id`
- `POST /api/v1/admin/models/reload`
- `POST /api/v1/admin/models/import-defaults`
- `GET /api/v1/admin/models/:name/route`
- `GET /api/v1/admin/proxies`
- `POST /api/v1/admin/proxies`
- `GET /api/v1/admin/proxies/:id`
- `PUT /api/v1/admin/proxies/:id`
- `DELETE /api/v1/admin/proxies/:id`
- `POST /api/v1/admin/proxies/:id/test`
- `POST /api/v1/admin/proxies/test-all`
- `GET /api/v1/admin/proxies/:id/quality`
- `POST /api/v1/admin/redeem/generate`
- `GET /api/v1/admin/redeem/stats`
- `POST /api/v1/admin/redeem/cancel`
- `GET /api/v1/admin/quota/:user_id/history`
- `POST /api/v1/admin/quota/:user_id/reset`
- `GET /api/v1/admin/quota/stats`
- `GET /api/v1/admin/announcements`
- `POST /api/v1/admin/announcements`
- `GET /api/v1/admin/announcements/:id`
- `PUT /api/v1/admin/announcements/:id`
- `DELETE /api/v1/admin/announcements/:id`
- `POST /api/v1/announcements/:id/read`
- `GET /api/v1/announcements/unread`
- `GET /api/v1/admin/promo-codes`
- `POST /api/v1/admin/promo-codes`
- `GET /api/v1/admin/promo-codes/:id`
- `PUT /api/v1/admin/promo-codes/:id`
- `DELETE /api/v1/admin/promo-codes/:id`
- `POST /api/v1/promo-codes/verify`
- `GET /api/v1/admin/attributes/definitions`
- `POST /api/v1/admin/attributes/definitions`
- `PUT /api/v1/admin/attributes/definitions/:id`
- `DELETE /api/v1/admin/attributes/definitions/:id`
- `GET /api/v1/admin/error-rules`
- `POST /api/v1/admin/error-rules`
- `PUT /api/v1/admin/error-rules/:id`
- `DELETE /api/v1/admin/error-rules/:id`
- `POST /api/v1/error-rules/apply`
- `GET /api/v1/admin/test-plans`
- `POST /api/v1/admin/test-plans`
- `PUT /api/v1/admin/test-plans/:id`
- `DELETE /api/v1/admin/test-plans/:id`
- `POST /api/v1/admin/test-plans/record`
- `GET /api/v1/admin/test-plans/:id/results`
- `POST /api/v1/admin/backup/export`
- `POST /api/v1/admin/backup/import`
- `GET /api/v1/admin/groups/usage-summary`
- `GET /api/v1/admin/groups/capacity-summary`
- `PUT /api/v1/admin/groups/sort-order`
- `GET /api/v1/admin/groups/:id/stats`
- `GET /api/v1/admin/groups/:id/rate-multipliers`
- `GET /api/v1/admin/groups/:id/api-keys`
- `GET /api/v1/admin/groups/all`
- `POST /api/v1/admin/groups`
- `PUT /api/v1/admin/groups/:id`
- `DELETE /api/v1/admin/groups/:id`
- `POST /api/v1/admin/api-keys/batch-create`
- `POST /api/v1/admin/accounts/batch-update`
- `POST /api/v1/admin/users/batch-import`
- `POST /api/v1/admin/api-keys/batch-delete`

## 未完成

- `POST /api/v1/auth/send-verify-code` 已接 Redis 缓存、60s 频控与 SMTP 邮件；但验证码消费校验还未并入注册/改邮箱提交链。
- `POST /api/v1/auth/validate-promo-code` 已接 `promo_codes` 实表校验，`POST /api/v1/auth/validate-invitation-code` 已接 `redeem_codes` 预检。
- `POST /v1/images/generations`、`POST /v1/videos/generations`、`GET /v1/videos/generations/:id`、`POST /v1/prompts/enhance` 当前仍返回 placeholder 语义。
- `GET /api/v1/admin/quota/stats` 已接活跃订阅配额池聚合，返回总用户数、活跃订阅用户数、总配额、已用量、剩余额度与利用率；但 quota 领域口径仍未统一。
- 备份当前只挂了 `export/import`；列表、下载、删除 handler 仍未完成，也未挂到路由。

## 下一步

1. 保持本文件只做“真实路由地图 + 占位说明”，不再混入过期示例响应。
2. 若后续重新启用 Swagger / OpenAPI，再把字段级契约迁回自动生成文档。
3. 每次新增或移除路由时，同步检查 `backend/src/gateway/routes.rs` 与本文件是否一致。
