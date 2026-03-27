# JWT 刷新机制实现文档

## 概述

本实现为 FoxNIO v0.2.0 添加了完整的 JWT 刷新机制，支持安全的 token 轮换和 token 黑名单功能。

## 架构设计

### Token 类型

1. **Access Token**
   - 短期有效（默认 24 小时，可配置）
   - 包含用户身份信息（id, email, role）
   - 每次请求携带

2. **Refresh Token**
   - 长期有效（默认 7 天）
   - 仅包含用户 ID 和唯一标识（JTI）
   - 用于刷新 access token
   - 存储 hash 值到数据库

### 安全机制

1. **Token 轮换**
   - 每次刷新时生成新的 token 对
   - 旧的 refresh token 自动撤销

2. **Token 黑名单**
   - 使用 Redis 存储已撤销的 token
   - TTL 与 token 过期时间一致
   - 登出时将 token 加入黑名单

3. **设备追踪**
   - 记录 User-Agent 和 IP 地址
   - 支持查看和撤销特定设备的 token

## API 端点

### POST /auth/login

登录并获取 token 对。

**请求：**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**响应：**
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_expires_in": 604800,
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "role": "user",
    "status": "active",
    "balance": 0
  }
}
```

### POST /auth/refresh

刷新 access token。

**请求：**
```json
{
  "refresh_token": "eyJ..."
}
```

**响应：**
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "refresh_expires_in": 604800
}
```

### POST /auth/logout

登出当前会话。

**请求头：**
```
Authorization: Bearer <access_token>
```

**请求体（可选）：**
```json
{
  "refresh_token": "eyJ..."
}
```

**响应：**
```json
{
  "message": "Successfully logged out"
}
```

### POST /auth/logout-all

登出所有设备。

**请求头：**
```
Authorization: Bearer <access_token>
```

**响应：**
```json
{
  "message": "Successfully logged out from 3 devices"
}
```

## 数据库表

### refresh_tokens

```sql
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    revoked_at TIMESTAMP WITH TIME ZONE,
    revoked_reason VARCHAR(255),
    user_agent VARCHAR(255),
    ip_address VARCHAR(45)
);

CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);
```

## Redis 键设计

- `token_blacklist:<jti>` - Access token 黑名单
- `refresh_blacklist:<jti>` - Refresh token 黑名单

TTL 与对应 token 的剩余有效期一致。

## 文件变更

### 新增文件

1. `migration/src/m20240327_000005_create_refresh_tokens.rs` - 数据库迁移
2. `src/entity/refresh_tokens.rs` - Entity 定义
3. `src/handler/auth/refresh.rs` - 刷新/登出处理器
4. `src/handler/auth/mod.rs` - Auth 模块入口
5. `tests/jwt_refresh_test.rs` - 单元测试
6. `tests/common/mock_redis.rs` - 模拟 Redis
7. `tests/common/fixtures.rs` - 测试数据

### 修改文件

1. `migration/src/lib.rs` - 注册新迁移
2. `src/entity/mod.rs` - 导出新 entity
3. `src/service/user.rs` - 添加刷新 token 功能
4. `src/handler/mod.rs` - 导出新路由

## 使用示例

### Rust 客户端

```rust
// 登录
let response = client
    .post("/auth/login")
    .json(&LoginRequest {
        email: "user@example.com".to_string(),
        password: "password123".to_string(),
    })
    .send()
    .await?;

let auth: AuthResponse = response.json().await?;

// 保存 token
save_tokens(auth.access_token, auth.refresh_token);

// 刷新 token
let refresh_response = client
    .post("/auth/refresh")
    .json(&RefreshRequest {
        refresh_token: auth.refresh_token,
    })
    .send()
    .await?;

let new_auth: RefreshResponse = refresh_response.json().await?;

// 登出
client
    .post("/auth/logout")
    .header("Authorization", format!("Bearer {}", access_token))
    .json(&LogoutRequest {
        refresh_token: Some(refresh_token),
    })
    .send()
    .await?;
```

## 安全考虑

1. **HTTPS 必需** - 所有 token 传输必须通过 HTTPS
2. **Token 存储** - 建议使用 httpOnly cookie 或安全存储
3. **刷新频率** - 不要频繁刷新，根据业务需求调整
4. **设备管理** - 监控异常登录行为
5. **清理策略** - 定期清理过期的 refresh token

## 测试覆盖

- Token 生成和验证
- Token 过期处理
- Token 轮换安全性
- 黑名单功能
- 并发安全性
- 错误处理
