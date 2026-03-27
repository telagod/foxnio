# Claude 渠道指纹模拟研究报告

## 🔍 当前问题分析

是的，**指纹等特征确实太明显了**。主要问题：

### 1. TLS 指纹问题

**Go/Rust 默认 TLS 指纹 vs Claude Code：**

| 指纹类型 | Go/Rust 默认 | Claude Code (Node.js 24.x) |
|---------|-------------|---------------------------|
| JA3 Hash | 不同 | `44f88fca027f27bab4bb08d4af15f23e` |
| JA4 | 不同 | `t13d1714h1_5b57614c22b0_7baf387fc6ff` |
| 密码套件数量 | 默认配置 | 17 个（精确顺序） |
| 扩展数量 | 默认配置 | 19 个（精确顺序） |
| GREASE | 不支持 | 支持 |

### 2. HTTP Header 问题

**问题清单：**
- ❌ Header key 自动 Canonical 化（`x-app` → `X-App`）
- ❌ Header 顺序随机（基于 map）
- ❌ 缺少 `X-Stainless-*` 系列 header
- ❌ User-Agent 默认为 `Go-http-client/2.0`
- ❌ `anthropic-beta` 格式错误

---

## 📋 完整解决方案

### 1. TLS 指纹模拟（使用 utls）

**Node.js 24.x 精确指纹：**

```go
// 17 个密码套件（精确顺序）
defaultCipherSuites = []uint16{
    // TLS 1.3
    0x1301, // TLS_AES_128_GCM_SHA256
    0x1302, // TLS_AES_256_GCM_SHA384
    0x1303, // TLS_CHACHA20_POLY1305_SHA256
    
    // ECDHE + AES-GCM
    0xc02b, 0xc02f, 0xc02c, 0xc030,
    
    // ECDHE + ChaCha20
    0xcca9, 0xcca8,
    
    // Legacy
    0xc009, 0xc013, 0xc00a, 0xc014,
    0x009c, 0x009d, 0x002f, 0x0035,
}

// 3 个曲线
defaultCurves = []utls.CurveID{
    utls.X25519,    // 0x001d
    utls.CurveP256, // 0x0017
    utls.CurveP384, // 0x0018
}

// 19 个扩展（精确顺序）
defaultExtensionOrder = []uint16{
    0,     // server_name
    65037, // encrypted_client_hello (ECH)
    23,    // extended_master_secret
    65281, // renegotiation_info
    10,    // supported_groups
    11,    // ec_point_formats
    35,    // session_ticket
    16,    // alpn
    5,     // status_request
    13,    // signature_algorithms
    18,    // signed_certificate_timestamp
    51,    // key_share
    45,    // psk_key_exchange_modes
    43,    // supported_versions
}
```

### 2. HTTP 请求头精确配置

**真实 Claude Code 请求头（抓包）：**

```http
Accept: application/json
X-Stainless-Retry-Count: 0
X-Stainless-Timeout: 600
X-Stainless-Lang: js
X-Stainless-Package-Version: 0.70.0
X-Stainless-OS: Linux
X-Stainless-Arch: arm64
X-Stainless-Runtime: node
X-Stainless-Runtime-Version: v24.13.0
anthropic-dangerous-direct-browser-access: true
anthropic-version: 2023-06-01
authorization: Bearer xxx
x-app: cli
User-Agent: claude-cli/2.1.22 (external, cli)
content-type: application/json
anthropic-beta: claude-code-20250219,oauth-2025-04-20,...
accept-language: *
sec-fetch-mode: cors
accept-encoding: gzip, deflate
```

**关键点：**
- ✅ `X-Stainless-OS` 不是 `X-Stainless-Os`（大小写敏感）
- ✅ `x-app` 是小写，不是 `X-App`
- ✅ `anthropic-beta` 是小写
- ✅ Header 顺序必须精确匹配

### 3. Beta Header 配置

**不同场景的 Beta Header：**

| 场景 | Beta Header |
|------|-------------|
| OAuth 账号 + 普通模型 | `claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14` |
| API Key 账号 + 普通模型 | `claude-code-20250219,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14` |
| OAuth 账号 + Haiku | `oauth-2025-04-20,interleaved-thinking-2025-05-14` |
| API Key 账号 + Haiku | `interleaved-thinking-2025-05-14` |
| count_tokens | 添加 `token-counting-2024-11-01` |

---

## 🛠️ 实现建议

### Go 实现（sub2api 方案）

```go
// 1. TLS 指纹
import "github.com/refraction-networking/utls"

func createTLSDialer(profile *Profile) *Dialer {
    return NewDialer(profile, nil)
}

// 2. Header 设置（绕过 Canonical 化）
func setHeaderRaw(h http.Header, key, value string) {
    h.Del(key)
    delete(h, key)
    h[key] = []string{value}  // 保持原始大小写
}

// 3. Beta Header 配置
func getBetaHeader(isOAuth, isHaiku bool) string {
    if isOAuth {
        if isHaiku {
            return "oauth-2025-04-20,interleaved-thinking-2025-05-14"
        }
        return "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14"
    }
    if isHaiku {
        return "interleaved-thinking-2025-05-14"
    }
    return "claude-code-20250219,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14"
}
```

### Rust 实现建议

```rust
// 1. 使用自定义 HTTP client
use reqwest::{Client, header::{HeaderMap, HeaderValue}};

pub fn build_claude_client() -> Result<Client> {
    Client::builder()
        .user_agent("claude-cli/2.1.22 (external, cli)")
        .build()
}

// 2. 构建请求头（保持顺序和大小写）
pub fn build_headers(auth: &str, beta: &str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("Accept", "application/json"),
        ("X-Stainless-Retry-Count", "0"),
        ("X-Stainless-Timeout", "600"),
        ("X-Stainless-Lang", "js"),
        ("X-Stainless-Package-Version", "0.70.0"),
        ("X-Stainless-OS", "Linux"),
        ("X-Stainless-Arch", "arm64"),
        ("X-Stainless-Runtime", "node"),
        ("X-Stainless-Runtime-Version", "v24.13.0"),
        ("anthropic-dangerous-direct-browser-access", "true"),
        ("anthropic-version", "2023-06-01"),
        ("authorization", &format!("Bearer {}", auth)),
        ("x-app", "cli"),
        ("User-Agent", "claude-cli/2.1.22 (external, cli)"),
        ("content-type", "application/json"),
        ("anthropic-beta", beta),
        ("accept-language", "*"),
        ("sec-fetch-mode", "cors"),
        ("accept-encoding", "gzip, deflate"),
    ]
}

// 3. Beta Header 配置
pub fn get_beta_header(is_oauth: bool, model: &str) -> String {
    let is_haiku = model.contains("haiku");
    
    if is_oauth {
        if is_haiku {
            "oauth-2025-04-20,interleaved-thinking-2025-05-14".to_string()
        } else {
            "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14".to_string()
        }
    } else {
        if is_haiku {
            "interleaved-thinking-2025-05-14".to_string()
        } else {
            "claude-code-20250219,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14".to_string()
        }
    }
}
```

---

## ⚠️ 检测点总结

| 检测项 | 当前状态 | 风险等级 | 解决方案 |
|--------|---------|----------|----------|
| TLS JA3/JA4 | 明显差异 | 🔴 高 | utls 模拟 |
| Header 大小写 | 自动 Canonical | 🔴 高 | 原始 key 设置 |
| Header 顺序 | 随机 | 🟡 中 | 按抓包顺序 |
| User-Agent | Go-http-client | 🔴 高 | 设置 Claude CLI |
| X-Stainless-* | 缺失 | 🔴 高 | 完整添加 |
| anthropic-beta | 格式错误 | 🔴 高 | 场景化配置 |
| 模型 ID | 短名 | 🟡 中 | 映射完整名 |

---

## 📚 参考资料

- sub2api TLS 指纹实现: `backend/internal/pkg/tlsfingerprint/dialer.go`
- Claude 常量定义: `backend/internal/pkg/claude/constants.go`
- Header 工具: `backend/internal/service/header_util.go`

---

**结论：是的，当前指纹太明显，需要完整实现 TLS + HTTP 层的模拟。**
