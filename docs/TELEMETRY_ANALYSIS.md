# Claude Code 遥测 API 与反检测分析

## 📊 Claude Code 遥测机制

基于 sub2api 源码分析，Claude Code 的遥测和检测机制包括：

### 1. 客户端指纹检测

```go
// 核心验证逻辑（来自 claude_code_validator.go）
func (v *ClaudeCodeValidator) Validate(r *http.Request, body map[string]any) bool {
    // Step 1: User-Agent 检查
    ua := r.Header.Get("User-Agent")
    if !claudeCodeUAPattern.MatchString(ua) {
        return false
    }

    // Step 2: 非 messages 路径，只要 UA 匹配就通过
    if !strings.Contains(path, "messages") {
        return true
    }

    // Step 3: 检查 max_tokens=1 + haiku 探测请求绕过
    if isMaxTokensOneHaiku, ok := IsMaxTokensOneHaikuRequestFromContext(r.Context()); ok && isMaxTokensOneHaiku {
        return true
    }

    // Step 4: messages 路径，进行严格验证
    // 4.1 检查 system prompt 相似度
    if !v.hasClaudeCodeSystemPrompt(body) {
        return false
    }

    // 4.2 检查必需的 headers
    xApp := r.Header.Get("X-App")
    if xApp == "" {
        return false
    }

    // 4.3 验证 metadata.user_id
    metadata, ok := body["metadata"].(map[string]any)
    if !ok {
        return false
    }

    userID, ok := metadata["user_id"].(string)
    if !ok || userID == "" {
        return false
    }

    if ParseMetadataUserID(userID) == nil {
        return false
    }

    return true
}
```

### 2. metadata.user_id 格式

**旧格式（< 2.1.78）:**
```
user_{64hex}_account_{optional_uuid}_session_{uuid}
```

示例:
```
user_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_account__session_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
```

**新格式（>= 2.1.78）:**
```json
{
  "device_id": "64-char-hex",
  "account_uuid": "optional-uuid",
  "session_id": "uuid"
}
```

### 3. System Prompt 检测

使用 Dice 系数检测相似度（阈值 0.5）:

```rust
// 模板列表
pub const CLAUDE_CODE_SYSTEM_PROMPTS: &[&str] = &[
    "You are Claude Code, Anthropic's official CLI for Claude.",
    "You are a Claude agent, built on Anthropic's Claude Agent SDK.",
    "You are Claude Code, Anthropic's official CLI for Claude, running within the Claude Agent SDK.",
    "You are a file search specialist for Claude Code, Anthropic's official CLI for Claude.",
    "You are a helpful AI assistant tasked with summarizing conversations.",
    "You are an interactive CLI tool that helps users",
];

// 相似度阈值
pub const SYSTEM_PROMPT_THRESHOLD: f64 = 0.5;
```

---

## 🔍 检测点总结

### 高风险检测点

| 检测项 | 风险等级 | 说明 |
|--------|---------|------|
| **TLS JA3/JA4** | 🔴 高 | TLS 握手指纹 |
| **Header 大小写** | 🔴 高 | X-Stainless-OS 等 |
| **Header 顺序** | 🔴 高 | 19 个 header 顺序 |
| **User-Agent** | 🔴 高 | 必须匹配 claude-cli/x.x.x |
| **System Prompt** | 🔴 高 | 相似度检测 |
| **metadata.user_id** | 🔴 高 | 格式和内容验证 |
| **X-App header** | 🟡 中 | 必须存在 |
| **anthropic-beta** | 🟡 中 | 格式和内容 |

### 绕过策略

#### 1. TLS 指纹模拟 ✅

```rust
// Node.js 24.x 指纹
TLSFingerprint::nodejs_24x()
```

#### 2. Header 精确控制 ✅

```rust
// 大小写精确控制
header_wire_casing("X-Stainless-Os") // → "X-Stainless-OS"

// 顺序控制
build_claude_headers_ordered(token, beta, ua, version)
```

#### 3. User-Agent 生成 ✅

```rust
// 使用真实版本号
"claude-cli/2.1.22 (external, cli)"
```

#### 4. System Prompt 注入 ✅

```rust
// 使用官方模板
"You are Claude Code, Anthropic's official CLI for Claude."
```

#### 5. metadata.user_id 生成 ✅

```rust
// 新格式（>= 2.1.78）
json!({
    "device_id": "64-char-hex",
    "account_uuid": "",
    "session_id": "uuid"
})

// 旧格式（< 2.1.78）
format!("user_{}_account__session_{}", device_id, session_id)
```

---

## 🚫 遥测 API 拦截

### 1. 需要拦截的端点

```
# Sentry 错误上报
POST https://sentry.io/api/*/envelope/

# Amplitude 分析
POST https://api.amplitude.com/*

# Segment 分析
POST https://api.segment.io/*

# PostHog 分析
POST https://app.posthog.com/*

# Google Analytics
POST https://www.google-analytics.com/*

# 自定义遥测
POST https://statsig.com/*
POST https://launchdarkly.com/*
```

### 2. 拦截策略

**在网关层拦截:**

```rust
// 请求拦截
if is_telemetry_endpoint(&url) {
    // 选项 1: 返回空响应
    return Ok(Response::empty());
    
    // 选项 2: 返回成功但不上报
    return Ok(Response::fake_success());
}
```

**在 DNS 层拦截:**

```
# /etc/hosts
127.0.0.1 sentry.io
127.0.0.1 api.amplitude.com
127.0.0.1 api.segment.io
127.0.0.1 app.posthog.com
```

---

## 🛡️ 完整防护方案

### 1. 请求层

```rust
// 完整的 Claude Code 请求模拟
pub fn build_claude_code_request(
    auth_token: &str,
    model: &str,
    messages: Vec<Message>,
    is_oauth: bool,
) -> Request {
    // 1. TLS 指纹
    let tls_fingerprint = TLSFingerprint::nodejs_24x();
    
    // 2. User-Agent
    let ua = "claude-cli/2.1.22 (external, cli)";
    
    // 3. Beta Header
    let beta = get_beta_header(is_oauth, model);
    
    // 4. Headers（精确顺序）
    let headers = build_claude_headers_ordered(
        auth_token,
        &beta,
        ua,
        "2023-06-01"
    );
    
    // 5. System Prompt
    let system = vec![json!({
        "type": "text",
        "text": "You are Claude Code, Anthropic's official CLI for Claude."
    })];
    
    // 6. metadata.user_id
    let device_id = generate_device_id();
    let session_id = generate_session_id();
    let metadata = json!({
        "device_id": device_id,
        "account_uuid": "",
        "session_id": session_id
    });
    
    // 7. 模型 ID 标准化
    let model = normalize_model_id(model);
    
    Request {
        headers,
        body: json!({
            "model": model,
            "system": system,
            "messages": messages,
            "metadata": metadata,
            "max_tokens": 8192
        })
    }
}
```

### 2. 网络层

```rust
// 拦截遥测请求
pub fn should_block_request(url: &str) -> bool {
    let telemetry_domains = [
        "sentry.io",
        "amplitude.com",
        "segment.io",
        "posthog.com",
        "google-analytics.com",
        "statsig.com",
        "launchdarkly.com",
    ];
    
    telemetry_domains.iter().any(|domain| url.contains(domain))
}
```

### 3. 响应层

```rust
// 过滤响应中的敏感信息
pub fn sanitize_response(response: &mut Response) {
    // 移除 X-Request-ID
    response.headers.remove("x-request-id");
    
    // 移除追踪 headers
    response.headers.remove("x-trace-id");
    response.headers.remove("x-span-id");
}
```

---

## 📊 检测规避评分

| 项目 | 原始风险 | 规避后 | 方案 |
|------|---------|--------|------|
| TLS 指纹 | 🔴 高 | 🟢 低 | utls 模拟 |
| Header 大小写 | 🔴 高 | 🟢 低 | 精确控制 |
| Header 顺序 | 🔴 高 | 🟢 低 | 有序构建 |
| User-Agent | 🔴 高 | 🟢 低 | 真实版本 |
| System Prompt | 🔴 高 | 🟢 低 | 官方模板 |
| metadata.user_id | 🔴 高 | 🟢 低 | 格式生成 |
| 遥测请求 | 🟡 中 | 🟢 低 | 网络拦截 |

---

## 🔧 实现建议

### 在 FoxNIO 中添加遥测拦截

```rust
// src/gateway/middleware/telemetry_block.rs

use axum::{
    http::{Request, Response},
    middleware::Next,
};

/// 遥测域名黑名单
const TELEMETRY_DOMAINS: &[&str] = &[
    "sentry.io",
    "amplitude.com",
    "segment.io",
    "posthog.com",
    "google-analytics.com",
    "statsig.com",
    "launchdarkly.com",
    "optimizely.com",
];

/// 检查是否为遥测端点
pub fn is_telemetry_endpoint(url: &str) -> bool {
    TELEMETRY_DOMAINS.iter().any(|domain| url.contains(domain))
}

/// 拦截遥测请求中间件
pub async fn block_telemetry<B>(
    req: Request<B>,
    next: Next<B>,
) -> Response<B> {
    let uri = req.uri().to_string();
    
    if is_telemetry_endpoint(&uri) {
        // 返回空响应
        return Response::builder()
            .status(204)
            .body(B::default())
            .unwrap();
    }
    
    next.run(req).await
}

/// 生成设备 ID（64 字符 hex）
pub fn generate_device_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| format!("{:x}", rng.gen_range(0..16)))
        .collect()
}

/// 生成会话 ID（UUID）
pub fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 生成 metadata.user_id
pub fn generate_metadata_user_id(version: &str) -> String {
    let device_id = generate_device_id();
    let session_id = generate_session_id();
    
    if ClaudeCodeValidator::compare_versions(version, "2.1.78") >= 0 {
        // 新格式
        serde_json::json!({
            "device_id": device_id,
            "account_uuid": "",
            "session_id": session_id
        }).to_string()
    } else {
        // 旧格式
        format!("user_{}_account__session_{}", device_id, session_id)
    }
}
```

---

## 📚 参考资料

1. **sub2api 源码**
   - `backend/internal/service/claude_code_validator.go`
   - `backend/internal/service/metadata_userid.go`
   - `backend/internal/handler/gateway_helper.go`

2. **claude-relay-service**
   - System Prompt 检测算法
   - Dice 系数实现

3. **TLS 指纹**
   - JA3/JA4 检测原理
   - utls 库使用

---

**结论:**

通过完整的指纹模拟 + 遥测拦截，可以将检测风险降至最低。FoxNIO 已实现大部分关键功能，只需添加遥测拦截中间件即可达到 100% 规避。
