# Usage / Audit / Balance Ledger — Authoritative Verification Table

> Single source of truth for what gets recorded, when, and with what values.
> Every field and value below is derived from the actual codebase.

---

## 1. `usages` Table Schema

Source: `backend/src/entity/usages.rs`

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `id` | `Uuid` (PK) | no | Auto-generated, not auto-increment |
| `user_id` | `Uuid` (FK → users) | no | Requesting user; `Uuid::nil()` for anonymous/system |
| `api_key_id` | `Uuid` (FK → api_keys) | no | API key used; `Uuid::nil()` when unavailable |
| `account_id` | `Option<Uuid>` (FK → accounts) | yes | Upstream account selected by scheduler |
| `model` | `String` | no | Model identifier (e.g. `gpt-4o`, `sora2-landscape-10s`, `websocket-realtime`) |
| `input_tokens` | `i64` | no | Prompt / input tokens; `0` for non-token channels (sora, ws failure) |
| `output_tokens` | `i64` | no | Completion / output tokens; `0` for non-token channels |
| `cost` | `i64` | no | Cost in cents (分); `0` on failure or free models |
| `request_id` | `Option<String>` | yes | Upstream `x-request-id` header or generated UUID |
| `success` | `bool` | no | `true` = upstream returned 2xx; `false` = error |
| `error_message` | `Option<String>` | yes | `None` on success; error description on failure |
| `metadata` | `Option<JsonValue>` | yes | Channel-specific JSON blob (see section 5) |
| `created_at` | `DateTime<Utc>` | no | Row creation timestamp |

Helper methods on `Model`: `total_tokens()` → `input_tokens + output_tokens`, `cost_yuan()` → `cost / 100.0`.

---

## 2. Per-Channel Recording Matrix

### 2a. Chat/Completions (`/v1/chat/completions`, `/v1/completions`, `/v1/messages`)

Writer: `ChatCompletionsForwarder::record_usage` / `AnthropicMessagesForwarder`

| Field | Success | Failure |
|-------|---------|---------|
| `success` | `true` | handler returns error; no usage row written by forwarder (error propagated to caller) |
| `account_id` | `Some(account_id)` | — |
| `model` | original model string | — |
| `input_tokens` | `result.usage.prompt_tokens` | — |
| `output_tokens` | `result.usage.completion_tokens` | — |
| `cost` | calculated via `calculate_cost()` | — |
| `request_id` | `Some(upstream x-request-id)` | — |
| `error_message` | `None` | — |
| `metadata` | `{billing_model, stream, first_token_ms, duration_ms, cache_read_tokens}` | — |

Note: `BillingService::record_usage` is an alternative writer used by other call sites. It sets `metadata: None` and triggers a balance ledger deduction when `cost > 0`.

### 2b. Realtime / WebSocket (`/v1/realtime`, `/v1/responses` WS)

Writer: `record_ws_failure` in `gateway/websocket/handler.rs`

Only failure rows are recorded (success path is TODO).

| Field | Success | Failure |
|-------|---------|---------|
| `success` | *(not recorded yet)* | `false` |
| `user_id` | — | `Uuid::nil()` (user context not propagated) |
| `api_key_id` | — | `Uuid::nil()` |
| `account_id` | — | `None` |
| `model` | — | `"websocket-realtime"` |
| `input_tokens` | — | `0` |
| `output_tokens` | — | `0` |
| `cost` | — | `0` |
| `request_id` | — | `None` |
| `error_message` | — | actual error string |
| `metadata` | — | `{"gateway": "websocket", "error_type": "<type>"}` |

Failure triggers: `send_error` (failed to send frame), `ws_error` (transport error).

### 2c. Gemini Native (`/v1beta/models/{model}`)

Writer: `record_gemini_failure` in `gateway/gemini/mod.rs`

Only failure rows are recorded (success responses are proxied directly).

| Field | Success | Failure |
|-------|---------|---------|
| `success` | *(not recorded)* | `false` |
| `user_id` | — | `Uuid::nil()` |
| `api_key_id` | — | `Uuid::nil()` |
| `account_id` | — | `None` |
| `model` | — | model name from path |
| `input_tokens` | — | `0` |
| `output_tokens` | — | `0` |
| `cost` | — | `0` |
| `request_id` | — | `None` |
| `error_message` | — | `"Upstream error: {e}"` |
| `metadata` | — | `{"gateway": "gemini", "error_code": 502}` |

Failure triggers: `generate_content` or `stream_generate_content` upstream call fails.

### 2d. Sora Image/Video/Prompt-Enhance (`/v1/images/generations`, `/v1/videos/generations`, `/v1/prompts/enhance`)

Writer: `SoraRouterService::record_usage` in `gateway/sora/service.rs`

| Field | Success | Failure |
|-------|---------|---------|
| `success` | `true` | `false` |
| `user_id` | caller user_id | caller user_id |
| `api_key_id` | caller api_key_id | caller api_key_id |
| `account_id` | `None` | `None` |
| `model` | model string (e.g. `sora2-landscape-10s`) | model string |
| `input_tokens` | `0` | `0` |
| `output_tokens` | `0` | `0` |
| `cost` | calculated per-unit cost | cost (may be 0 if pre-deduction failed) |
| `request_id` | `None` | `None` |
| `error_message` | `None` | actual error string |
| `metadata` | `{"gateway": "sora"}` | `{"gateway": "sora"}` |

Note: Sora uses per-unit pricing (not token-based). Cost is computed by `SoraService::calculate_cost` based on model type, duration, resolution, and pro tier.

### 2e. BillingService Generic Path

Writer: `BillingService::record_usage` in `service/billing.rs`

| Field | Success | Failure |
|-------|---------|---------|
| `success` | `params.success` | `params.success` (caller decides) |
| `user_id` | `params.user_id` | `params.user_id` |
| `api_key_id` | `params.api_key_id` | `params.api_key_id` |
| `account_id` | `None` | `None` |
| `model` | `params.model` | `params.model` |
| `input_tokens` | `params.input_tokens` | `params.input_tokens` |
| `output_tokens` | `params.output_tokens` | `params.output_tokens` |
| `cost` | calculated | calculated |
| `request_id` | `None` | `None` |
| `error_message` | `None` | `params.error_message` |
| `metadata` | `None` | `None` |

When `cost > 0`, a `balance_ledger` entry with `source_type = "usage"` is created and user balance is atomically decremented.

### 2f. UsageLog Service Path

Writer: `UsageLog::insert` in `service/usage_log.rs`

This is the richest writer. It packs extended fields into `metadata`:

| metadata key | Source field | Type |
|-------------|-------------|------|
| `platform` | `entry.platform` | string |
| `request_type` | `entry.request_type` | i16 |
| `stream` | `entry.stream` | bool |
| `status_code` | `entry.status_code` | i16 |
| `billing_type` | `entry.billing_type` | i8 |
| `response_time_ms` | `entry.response_time_ms` | i64 |
| `total_tokens` | `entry.total_tokens` | i64 |
| `group_id` | `entry.group_id` (if present) | i64 |

The `success` column is derived: `entry.status_code < 400`.

---

## 3. `audit_logs` Table Schema

Source: `backend/src/entity/audit_logs.rs`

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `id` | `Uuid` (PK) | no | Auto-generated |
| `user_id` | `Option<Uuid>` (FK → users) | yes | Acting user; `None` for system events |
| `action` | `String` | no | Action enum string (see section 4) |
| `resource_type` | `Option<String>` | yes | Target entity type: `"user"`, `"api_key"`, `"account"`, etc. |
| `resource_id` | `Option<String>` | yes | Target entity ID |
| `ip_address` | `Option<String>` | yes | Client IP |
| `user_agent` | `Option<String>` | yes | Client User-Agent |
| `request_data` | `Option<JsonValue>` | yes | Request payload (sensitive, excluded from sanitized view) |
| `response_status` | `Option<i32>` | yes | HTTP response status code |
| `created_at` | `DateTime<Utc>` | no | Row creation timestamp |

Sanitized view (`SanitizedAuditLog`) masks IP to first two octets and truncates User-Agent to 50 chars.

---

## 4. Audit Action Types

Source: `backend/src/entity/audit_logs.rs` — `AuditAction` enum

| Variant | DB String | Sensitive | When It Fires |
|---------|-----------|-----------|---------------|
| `UserLogin` | `USER_LOGIN` | yes | Successful login (JWT issued) |
| `UserLogout` | `USER_LOGOUT` | no | Explicit logout |
| `UserRegister` | `USER_REGISTER` | no | New user registration |
| `PasswordChange` | `PASSWORD_CHANGE` | yes | User changes own password |
| `PasswordReset` | `PASSWORD_RESET` | no | Password reset via token |
| `ApiKeyCreate` | `API_KEY_CREATE` | yes | User creates a new API key |
| `ApiKeyDelete` | `API_KEY_DELETE` | yes | User deletes an API key |
| `ApiKeyUpdate` | `API_KEY_UPDATE` | no | API key metadata updated |
| `AccountUpdate` | `ACCOUNT_UPDATE` | no | Upstream account modified |
| `AccountCreate` | `ACCOUNT_CREATE` | no | New upstream account added |
| `AccountDelete` | `ACCOUNT_DELETE` | no | Upstream account removed |
| `AdminAction` | `ADMIN_ACTION` | yes | Catch-all for admin operations (resource_type/id specify target) |
| `BalanceUpdate` | `BALANCE_UPDATE` | no | Admin adjusts user balance |
| `BalanceRecharge` | `BALANCE_RECHARGE` | no | User recharges balance |
| `ApiRequest` | `API_REQUEST` | no | Generic API request audit |
| `RateLimitExceeded` | `RATE_LIMIT_EXCEEDED` | no | Rate limit hit |
| `SecurityAlert` | `SECURITY_ALERT` | yes | Suspicious activity detected |

Sensitive actions are used by `get_sensitive_logs` to filter the dedicated admin endpoint.

---

## 5. Metadata JSON Examples

### 5a. Chat/Completions — Success

```json
{
  "billing_model": "gpt-4o",
  "stream": true,
  "first_token_ms": 245,
  "duration_ms": 1832,
  "cache_read_tokens": null
}
```

### 5b. Chat/Completions — Failure

No usage row is written by `ChatCompletionsForwarder` on failure. The HTTP handler returns a `502 BAD_GATEWAY` or `500` error directly.

### 5c. WebSocket — Failure

```json
{
  "gateway": "websocket",
  "error_type": "send_error"
}
```

```json
{
  "gateway": "websocket",
  "error_type": "ws_error"
}
```

### 5d. Gemini — Failure

```json
{
  "gateway": "gemini",
  "error_code": 502
}
```

### 5e. Sora — Success / Failure

```json
{
  "gateway": "sora"
}
```

### 5f. UsageLog Service — Full Metadata

```json
{
  "platform": "openai",
  "request_type": 1,
  "stream": true,
  "status_code": 200,
  "billing_type": 0,
  "response_time_ms": 1450,
  "total_tokens": 1523,
  "group_id": 3
}
```

Failure variant (status_code >= 400):

```json
{
  "platform": "openai",
  "request_type": 1,
  "stream": false,
  "status_code": 502,
  "billing_type": 0,
  "response_time_ms": 5012,
  "total_tokens": 0
}
```

### 5g. BillingService — No Metadata

`BillingService::record_usage` sets `metadata: None`.

---

## 6. Balance Ledger Recording

### 6a. `balance_ledger` Table Schema

Source: `backend/src/entity/balance_ledger.rs`

| Column | Type | Nullable | Description |
|--------|------|----------|-------------|
| `id` | `Uuid` (PK) | no | Auto-generated |
| `user_id` | `Uuid` (FK → users) | no | Affected user |
| `source_type` | `String(30)` | no | Category of mutation (see below) |
| `source_id` | `Option<String(255)>` | yes | Reference ID (e.g. usage row UUID, redeem code) |
| `delta_cents` | `i64` | no | Signed amount in cents; negative = deduction |
| `balance_before` | `i64` | no | Snapshot before mutation |
| `balance_after` | `i64` | no | Snapshot after mutation |
| `description` | `Option<String>` | yes | Human-readable description |
| `metadata` | `Option<JsonValue>` | yes | Extra context |
| `created_at` | `DateTime<Utc>` | no | Row creation timestamp |

### 6b. Source Types and Triggers

| `source_type` | Trigger | `delta_cents` | `source_id` | `description` example |
|---------------|---------|---------------|-------------|----------------------|
| `"usage"` | `BillingService::record_usage` when `cost > 0` | negative (`-cost`) | `usage.id.to_string()` | `"Usage: gpt-4o (1523 tokens)"` |
| `"redeem"` | `RedeemCodeService::redeem_balance_with_txn` | positive (`amount * 100`) | `None` | `"Redeemed $10.00"` |

### 6c. Atomicity

- `BalanceLedgerService::record()` — wraps in a transaction: reads `user.balance`, inserts ledger row, updates `user.balance`. Rejects if `balance_after < 0`.
- `BalanceLedgerService::record_with_txn()` — same logic but joins an existing transaction (caller commits).
- `BalanceLedgerService::insert_entry_with_txn()` — inserts ledger row only, no `user.balance` update. Used by redeem flow where balance is already updated by the caller.

### 6d. Balance Deduction Flow (Usage)

```
BillingService::record_usage
  ├─ Insert usages row (cost = calculated)
  ├─ If cost > 0:
  │   └─ BalanceLedgerService::record(user_id, "usage", usage.id, -cost, description, None)
  │       ├─ BEGIN TXN
  │       ├─ SELECT user.balance → balance_before
  │       ├─ balance_after = balance_before - cost
  │       ├─ REJECT if balance_after < 0
  │       ├─ INSERT balance_ledger row
  │       ├─ UPDATE user.balance = balance_after
  │       └─ COMMIT
  └─ Return UsageRecord
```

### 6e. Balance Credit Flow (Redeem)

```
RedeemCodeService::redeem
  ├─ BEGIN TXN
  ├─ SELECT redeem_code FOR UPDATE
  ├─ Validate status, expiry, max_uses
  ├─ redeem_balance_with_txn:
  │   ├─ SELECT user → balance_before
  │   ├─ balance_after = balance_before + (amount * 100)
  │   ├─ UPDATE user.balance = balance_after
  │   └─ insert_entry_with_txn("redeem", delta, balance_before, balance_after)
  ├─ UPDATE redeem_code status
  └─ COMMIT
```

