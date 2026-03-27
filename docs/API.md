# FoxNIO API 文档

## 基础信息

- **Base URL**: `http://localhost:3000`
- **认证方式**: Bearer Token (API Key)
- **内容类型**: `application/json`

---

## 认证端点

### 注册用户

```http
POST /api/v1/auth/register
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "Password123",
  "username": "user123"
}
```

**响应**:
```json
{
  "success": true,
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "username": "user123"
  }
}
```

### 登录

```http
POST /api/v1/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "Password123"
}
```

**响应**:
```json
{
  "success": true,
  "token": "jwt_token",
  "user": {
    "id": "uuid",
    "email": "user@example.com"
  }
}
```

---

## OpenAI 兼容端点

### Chat Completions

```http
POST /v1/chat/completions
Authorization: Bearer foxnio-your-api-key
Content-Type: application/json

{
  "model": "gpt-4",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello"}
  ],
  "temperature": 0.7,
  "max_tokens": 100,
  "stream": false
}
```

**响应**:
```json
{
  "id": "chatcmpl-123",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gpt-4",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  }
}
```

### 流式响应

```http
POST /v1/chat/completions
Authorization: Bearer foxnio-your-api-key
Content-Type: application/json

{
  "model": "gpt-4",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": true
}
```

**响应** (SSE):
```
data: {"id":"chatcmpl-123","choices":[{"delta":{"content":"Hello"},"index":0}]}

data: {"id":"chatcmpl-123","choices":[{"delta":{"content":"!"},"index":0}]}

data: [DONE]
```

### 列出模型

```http
GET /v1/models
Authorization: Bearer foxnio-your-api-key
```

**响应**:
```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4",
      "object": "model",
      "owned_by": "openai"
    },
    {
      "id": "claude-3-opus",
      "object": "model",
      "owned_by": "anthropic"
    }
  ]
}
```

---

## API Key 管理

### 创建 API Key

```http
POST /api/v1/user/apikeys
Authorization: Bearer jwt_token
Content-Type: application/json

{
  "name": "My API Key"
}
```

**响应**:
```json
{
  "id": "uuid",
  "key": "foxnio-xxxxxxxxxxxxxxxx",
  "name": "My API Key",
  "created_at": "2024-01-01T00:00:00Z"
}
```

### 列出 API Keys

```http
GET /api/v1/user/apikeys
Authorization: Bearer jwt_token
```

**响应**:
```json
{
  "object": "list",
  "data": [
    {
      "id": "uuid",
      "key": "foxnio-****...****",
      "name": "My API Key",
      "status": "active",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ]
}
```

### 删除 API Key

```http
DELETE /api/v1/user/apikeys/{id}
Authorization: Bearer jwt_token
```

---

## 用户端点

### 获取用户信息

```http
GET /api/v1/user/me
Authorization: Bearer jwt_token
```

**响应**:
```json
{
  "id": "uuid",
  "email": "user@example.com",
  "username": "user123",
  "balance": 1000,
  "role": "user",
  "created_at": "2024-01-01T00:00:00Z"
}
```

### 获取使用统计

```http
GET /api/v1/user/usage
Authorization: Bearer jwt_token
```

**响应**:
```json
{
  "total_requests": 1000,
  "total_tokens": 50000,
  "total_cost": 500,
  "by_model": {
    "gpt-4": {"requests": 500, "tokens": 25000},
    "claude-3-opus": {"requests": 500, "tokens": 25000}
  }
}
```

---

## 管理端点

### 列出用户

```http
GET /api/v1/admin/users
Authorization: Bearer admin_token
```

### 创建账号

```http
POST /api/v1/admin/accounts
Authorization: Bearer admin_token
Content-Type: application/json

{
  "name": "OpenAI Main",
  "provider": "openai",
  "api_key": "sk-xxx",
  "priority": 1,
  "weight": 10
}
```

### 获取统计

```http
GET /api/v1/admin/stats
Authorization: Bearer admin_token
```

**响应**:
```json
{
  "total_users": 100,
  "total_accounts": 10,
  "total_requests_today": 5000,
  "total_revenue": 50000
}
```

---

## 健康检查

### 健康检查

```http
GET /health
```

**响应**:
```json
{
  "status": "healthy",
  "checks": {
    "database": {"status": "healthy"},
    "redis": {"status": "healthy"}
  },
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### 就绪检查

```http
GET /ready
```

### 存活检查

```http
GET /live
```

---

## 错误响应

所有错误响应格式:

```json
{
  "error": {
    "type": "invalid_request_error",
    "message": "Invalid API key",
    "code": "invalid_api_key"
  }
}
```

### 常见错误码

| 状态码 | 错误码 | 描述 |
|--------|--------|------|
| 400 | invalid_request | 请求格式错误 |
| 401 | invalid_api_key | API Key 无效 |
| 402 | insufficient_balance | 余额不足 |
| 429 | rate_limit_exceeded | 超过速率限制 |
| 500 | internal_error | 服务器内部错误 |

---

## 速率限制

- **默认限制**: 60 次/分钟
- **Header**: `X-RateLimit-Limit`, `X-RateLimit-Remaining`

---

## 费率

| 模型 | 输入价格 | 输出价格 |
|------|---------|---------|
| gpt-4-turbo | $0.01/1K | $0.03/1K |
| gpt-4o | $0.0025/1K | $0.01/1K |
| claude-3-opus | $0.015/1K | $0.075/1K |
| claude-3-sonnet | $0.003/1K | $0.015/1K |
| gemini-1.5-pro | $0.00125/1K | $0.005/1K |
