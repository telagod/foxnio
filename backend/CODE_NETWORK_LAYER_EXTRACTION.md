# Claude Code 网络层提取总结

**提取时间**: 2026-03-30 18:50
**Claude Code 版本**: 2.1.87
**包大小**: 30.4 MB (cli.js: 12.9 MB)

---

## 📋 提取成果

### 1. API 端点

```
主端点:
- api.anthropic.com/v1/messages          # 主要 API
- api.anthropic.com/v1/models/{model}    # 模型信息

辅助端点:
- api.anthropic.com/api/claude_cli_feedback           # 反馈
- api.anthropic.com/api/claude_code/metrics           # 指标
- api.anthropic.com/api/oauth/claude_cli/create_api_key  # OAuth
- api.anthropic.com/api/claude_code_shared_session_transcripts
```

---

### 2. HTTP 头配置

#### 核心头
```
x-api-key: {API_KEY}
anthropic-version: 2023-06-01
content-type: application/json
accept: application/json
```

#### 客户端标识头
```
x-client-app: claude-code
x-client-request-id: {UUID}
x-client-current-telemetry: {TELEMETRY}
x-client-last-telemetry: {TELEMETRY}
x-client-xtra-sku: {SKU_INFO}
```

#### 可选头
```
anthropic-beta: {BETA_FEATURES}
anthropic-dangerous-direct-browser-access: true
```

---

### 3. TLS 配置

**支持的 Node.js 版本**:
- ❌ Node.js 16.x (2025-01-06 停止支持)
- ✅ Node.js 18.x
- ✅ Node.js 20.x
- ✅ Node.js 22.x
- ✅ Node.js 24.x (推荐)

**TLS 特征**:
- TLS 1.3 优先
- HTTP/2 支持
- 连接复用

---

### 4. User-Agent 格式

```
claude-cli/{version}
claude-code/{version}
```

---

### 5. 请求体格式

#### Messages API
```json
{
  "model": "claude-3-5-sonnet-20241022",
  "messages": [
    {
      "role": "user",
      "content": "Hello"
    }
  ],
  "max_tokens": 4096,
  "stream": true
}
```

#### 特殊参数
```
- stream: boolean (流式输出)
- max_tokens: number (最大 token 数)
- temperature: number (温度，可选)
- system: string (系统提示，可选)
- tools: array (工具定义，可选)
- metadata: object (元数据，可选)
```

---

## 🔍 未发现的内容

**不需要签名**:
- ✅ 没有发现额外的请求签名机制
- ✅ 只需要 `x-api-key` 认证
- ✅ 没有复杂的 HMAC 签名

**TLS 指纹**:
- ⚠️ 需要使用 Node.js 24.x 的 TLS 栈
- ⚠️ 或者使用 rustls 配置为匹配 Node.js

---

## 💡 关键发现

### 1. 简单的认证机制
只需要 `x-api-key` 头，不需要复杂的签名流程。

### 2. 轻量级网络层
只需要实现：
- HTTP 客户端
- TLS 配置
- 头模板
- 请求格式化

### 3. 版本标识
通过 `anthropic-version: 2023-06-01` 头标识 API 版本。

---

## 📊 实现计划

### Phase 1: 基础模块 (今天完成)
```
backend/src/gateway/claude_shell/
├── mod.rs           - 模块入口
├── headers.rs       - HTTP 头配置
├── client.rs        - HTTP 客户端
└── request.rs       - 请求构建
```

### Phase 2: TLS 配置 (明天)
```
backend/src/gateway/claude_shell/
└── tls.rs           - TLS 指纹配置
```

### Phase 3: 集成测试 (后天)
```
backend/tests/
└── claude_shell_test.rs
```

---

## 🎯 下一步

立即开始实现 `backend/src/gateway/claude_shell/` 模块。
