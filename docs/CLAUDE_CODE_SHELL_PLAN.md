# Claude Code Shell - 轻量级 API 转发方案

**创建时间**: 2026-03-30 18:40
**分支**: `claude-code-shell`
**目标**: 提取 Claude Code 的网络层，创建轻量级 API 转发壳子

---

## 🎯 核心需求

**不要**：
- ❌ 完整的 Claude Code CLI（太重，~100MB+）
- ❌ 内置提示词和业务逻辑
- ❌ 工具调用、文件操作等复杂功能

**只要**：
- ✅ TLS 指纹（Node.js 24.x JA3/JA4）
- ✅ 请求签名逻辑
- ✅ 认证流程
- ✅ HTTP 头模板
- ✅ 轻量级（<10MB）

**目的**：
- 🎯 转发 API 请求到 Anthropic
- 🎯 避免风控检测
- 🎯 随官方更新同步

---

## 📊 技术分析

### Claude Code CLI 包含什么？

| 组件 | 大小 | 我们需要？ |
|------|------|----------|
| **网络层** | ~2MB | ✅ **是** |
| TLS 指纹配置 | ~100KB | ✅ **是** |
| 请求签名 | ~50KB | ✅ **是** |
| HTTP 头模板 | ~10KB | ✅ **是** |
| 认证逻辑 | ~200KB | ✅ **是** |
| **提示词引擎** | ~20MB | ❌ 否 |
| **工具调用** | ~30MB | ❌ 否 |
| **文件操作** | ~10MB | ❌ 否 |
| **CLI 界面** | ~5MB | ❌ 否 |
| **依赖库** | ~50MB | ❌ 否 |
| **总计** | ~100MB+ | **只需 ~2MB** |

---

## 🔍 核心提取目标

### 1. TLS 指纹配置

**已知**（已在 tls.rs 中）：
```rust
// Node.js 24.x 指纹
pub const DEFAULT_CIPHER_SUITES: &[u16] = &[
    0x1301, // TLS_AES_128_GCM_SHA256
    0x1302, // TLS_AES_256_GCM_SHA384
    0x1303, // TLS_CHACHA20_POLY1305_SHA256
    // ... 17 个密码套件
];

pub const DEFAULT_CURVES: &[u16] = &[
    0x001d, // X25519
    0x0017, // P-256
    0x0018, // P-384
];

pub const DEFAULT_EXTENSION_ORDER: &[u16] = &[
    0x0000, // server_name
    0x0017, // extended_master_secret
    // ... 19 个扩展
];
```

**需要更新**：
- ✅ 已有基础实现
- ⚠️ 需要定期同步 Node.js 版本更新

---

### 2. HTTP 头模板

**已知**（部分在 headers.rs）：
```rust
// Claude Code 标准请求头
pub const CLAUDE_CODE_HEADERS: &[(&str, &str)] = &[
    ("content-type", "application/json"),
    ("accept", "application/json"),
    ("anthropic-version", "2023-06-01"),
    ("user-agent", "claude-cli/1.0.0"),
    ("x-api-key", ""), // 动态填充
    // ...
];
```

**需要提取**：
- 完整的请求头列表
- 必需字段 vs 可选字段
- 特殊标记字段

---

### 3. 请求签名逻辑

**未知**（需要逆向）：
- 是否有额外签名？
- 请求体是否需要 hash？
- 时间戳处理？

**逆向方法**：
```bash
# 抓包分析
mitmproxy --mode reverse:https://api.anthropic.com@8080

# 运行 Claude Code 并观察
claude code --prompt "test"
```

---

### 4. 认证流程

**已知**：
- API Key 认证
- OAuth 流程（可选）

**需要确认**：
- Token 刷新机制
- Session 管理

---

## 💡 实施方案

### 方案 A: 最小化提取（推荐）

**核心思路**：
只提取网络通信必需的部分，忽略所有业务逻辑

**架构**：
```
Claude Code Shell (轻量级)
├── tls_config.rs      - TLS 指纹 (100KB)
├── headers.rs         - HTTP 头模板 (10KB)
├── signature.rs       - 签名逻辑 (50KB)
├── auth.rs           - 认证流程 (200KB)
├── sync.rs           - 自动更新 (100KB)
└── client.rs         - HTTP 客户端 (500KB)
```

**总大小**: < 1MB

---

### 方案 B: 运行时监控提取

**核心思路**：
通过监控 Claude Code CLI 的实际行为，提取网络层模板

**步骤**：
1. 启动 Claude Code CLI
2. 使用 mitmproxy 抓包
3. 分析请求格式
4. 提取模板
5. 实现轻量级客户端

**工具**：
```bash
# 1. 启动代理
mitmproxy --mode reverse:https://api.anthropic.com@8443 \
  --ssl-insecure

# 2. 配置 Claude Code 使用代理
export HTTPS_PROXY=http://localhost:8443
export NODE_EXTRA_CA_CERTS=~/.mitmproxy/mitmproxy-ca-cert.pem

# 3. 运行 Claude Code
claude code --prompt "test"

# 4. 分析抓包结果
# 提取请求头、签名、TLS 配置
```

---

### 方案 C: 源码分析提取

**核心思路**：
从 Claude Code 的 npm 包中提取关键代码

**步骤**：
```bash
# 1. 下载 Claude Code 包
npm pack @anthropic-ai/claude-code

# 2. 解压并分析
tar -xzf anthropic-ai-claude-code-*.tgz
cd package

# 3. 查找网络层代码
find . -name "*.js" | xargs grep -l "anthropic\|api\|request"

# 4. 提取关键文件
# - dist/api-client.js
# - dist/auth.js
# - dist/http.js
```

**注意**：
- ⚠️ 需要遵守 Apache 2.0 许可证
- ⚠️ 不能直接复制代码，只能参考实现

---

## 📋 详细实施计划

### Phase 1: 信息收集（1-2 天）

#### 任务 1.1: 网络层逆向
```bash
# 使用 mitmproxy 抓包
# 目标：获取完整的请求格式

# 分析内容：
- [ ] HTTP 头完整列表
- [ ] 请求体格式
- [ ] 签名算法（如果有）
- [ ] TLS ClientHello 抓包
- [ ] 认证流程
```

#### 任务 1.2: Claude Code 包分析
```bash
# 下载并解压
npm pack @anthropic-ai/claude-code
tar -xzf *.tgz

# 分析文件结构
tree package/dist

# 提取关键代码位置
grep -r "anthropic-version" package/
grep -r "x-api-key" package/
```

#### 任务 1.3: 文档查阅
- [ ] Anthropic API 文档
- [ ] Claude Code GitHub Issues
- [ ] 社区讨论

---

### Phase 2: 核心实现（2-3 天）

#### 任务 2.1: 创建 claude-shell 模块

```rust
// backend/src/gateway/claude_shell/mod.rs
pub mod tls;
pub mod headers;
pub mod signature;
pub mod auth;
pub mod client;
pub mod sync;

pub struct ClaudeShell {
    tls_config: TLSConfig,
    headers: HeaderTemplate,
    signer: RequestSigner,
    client: HttpClient,
}

impl ClaudeShell {
    /// 创建新的 Claude Shell 客户端
    pub fn new(api_key: String) -> Self {
        Self {
            tls_config: TLSConfig::nodejs_24x(),
            headers: HeaderTemplate::claude_code(),
            signer: RequestSigner::new(),
            client: HttpClient::new(),
        }
    }

    /// 发送请求到 Anthropic API
    pub async fn send(&self, request: Request) -> Result<Response> {
        // 1. 应用 TLS 指纹
        let tls_stream = self.tls_config.connect().await?;

        // 2. 构建请求头
        let headers = self.headers.build(&request);

        // 3. 签名请求（如果需要）
        let signed_request = self.signer.sign(request)?;

        // 4. 发送请求
        let response = self.client.send(tls_stream, headers, signed_request).await?;

        Ok(response)
    }
}
```

#### 任务 2.2: TLS 指纹实现

```rust
// backend/src/gateway/claude_shell/tls.rs
use rustls::{ClientConfig, SupportedCipherSuite, SignatureScheme};

pub struct TLSConfig {
    cipher_suites: Vec<SupportedCipherSuite>,
    curves: Vec<SupportedCurve>,
    extensions: Vec<Extension>,
}

impl TLSConfig {
    /// Node.js 24.x TLS 指纹（Claude Code）
    pub fn nodejs_24x() -> Self {
        Self {
            cipher_suites: vec![
                // TLS 1.3
                TLS_AES_128_GCM_SHA256,
                TLS_AES_256_GCM_SHA384,
                TLS_CHACHA20_POLY1305_SHA256,
                // TLS 1.2
                TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
                TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
                // ... 完整的 17 个密码套件
            ],
            curves: vec![
                X25519,
                P256,
                P384,
            ],
            extensions: vec![
                SERVER_NAME,
                EXTENDED_MASTER_SECRET,
                // ... 完整的 19 个扩展
            ],
        }
    }

    /// 自定义 TLS 配置
    pub fn custom() -> Self {
        // 可配置不同的指纹
    }
}
```

#### 任务 2.3: HTTP 头模板

```rust
// backend/src/gateway/claude_shell/headers.rs
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub struct HeaderTemplate {
    base_headers: HeaderMap,
}

impl HeaderTemplate {
    /// Claude Code 标准请求头
    pub fn claude_code() -> Self {
        let mut headers = HeaderMap::new();

        // 必需头
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("accept", "application/json".parse().unwrap());
        headers.insert("anthropic-version", "2023-06-01".parse().unwrap());

        // Claude Code 特征头
        headers.insert("user-agent", "claude-cli/1.0.0".parse().unwrap());
        headers.insert("x-client-name", "claude-code".parse().unwrap());

        Self { base_headers: headers }
    }

    /// 构建请求头
    pub fn build(&self, api_key: &str) -> HeaderMap {
        let mut headers = self.base_headers.clone();
        headers.insert("x-api-key", api_key.parse().unwrap());
        headers
    }
}
```

#### 任务 2.4: 请求签名（如果需要）

```rust
// backend/src/gateway/claude_shell/signature.rs
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};

pub struct RequestSigner {
    // 签名配置（待逆向确认）
}

impl RequestSigner {
    /// 签名请求（如果 Anthropic 要求）
    pub fn sign(&self, request: &mut Request) -> Result<()> {
        // 待实现：根据逆向结果
        // 可能的签名方式：
        // 1. HMAC-SHA256
        // 2. 请求体 hash
        // 3. 时间戳 + nonce

        Ok(())
    }
}
```

---

### Phase 3: 自动同步（1-2 天）

#### 任务 3.1: 版本监控

```rust
// backend/src/gateway/claude_shell/sync.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ClaudeCodeRelease {
    pub version: String,
    pub published_at: String,
    pub tarball_url: String,
}

pub struct ClaudeCodeSync {
    check_interval: Duration,
    current_version: String,
}

impl ClaudeCodeSync {
    /// 检查 Claude Code 新版本
    pub async fn check_latest_version(&self) -> Result<Option<ClaudeCodeRelease>> {
        // 从 npm registry 查询最新版本
        let url = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest";
        let response = reqwest::get(url).await?;
        let release: ClaudeCodeRelease = response.json().await?;

        if release.version != self.current_version {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    }

    /// 同步最新配置
    pub async fn sync(&self, release: &ClaudeCodeRelease) -> Result<()> {
        // 1. 下载最新版本
        let tarball = self.download_tarball(&release.tarball_url).await?;

        // 2. 提取网络层配置
        let config = self.extract_network_config(&tarball)?;

        // 3. 更新本地配置
        self.update_local_config(&config)?;

        Ok(())
    }

    /// 启动自动同步
    pub async fn start_auto_sync(self) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.check_interval);

            loop {
                interval.tick().await;

                if let Ok(Some(release)) = self.check_latest_version().await {
                    info!("New Claude Code version available: {}", release.version);

                    if let Err(e) = self.sync(&release).await {
                        error!("Failed to sync: {}", e);
                    }
                }
            }
        });
    }
}
```

---

### Phase 4: 集成测试（1 天）

#### 任务 4.1: 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config() {
        let config = TLSConfig::nodejs_24x();
        assert_eq!(config.cipher_suites.len(), 17);
        assert_eq!(config.curves.len(), 3);
    }

    #[test]
    fn test_header_template() {
        let template = HeaderTemplate::claude_code();
        let headers = template.build("test-api-key");

        assert!(headers.contains_key("x-api-key"));
        assert!(headers.contains_key("anthropic-version"));
    }

    #[tokio::test]
    async fn test_claude_shell_request() {
        let shell = ClaudeShell::new("test-api-key".to_string());

        let request = Request {
            model: "claude-3-5-sonnet-20241022".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
        };

        // Mock 测试，不实际发送
        // let response = shell.send(request).await.unwrap();
    }
}
```

#### 任务 4.2: 集成测试

```rust
#[tokio::test]
#[ignore] // 需要真实 API key
async fn test_real_anthropic_api() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    let shell = ClaudeShell::new(api_key);

    let request = Request {
        model: "claude-3-5-sonnet-20241022".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "Say hello".to_string(),
        }],
        max_tokens: 100,
    };

    let response = shell.send(request).await.unwrap();

    assert!(response.content.len() > 0);
}
```

---

## 📊 文件结构

```
backend/src/gateway/claude_shell/
├── mod.rs              - 模块入口 (200 行)
├── tls.rs              - TLS 指纹配置 (300 行)
├── headers.rs          - HTTP 头模板 (150 行)
├── signature.rs        - 请求签名 (200 行，待实现)
├── auth.rs            - 认证流程 (150 行)
├── client.rs          - HTTP 客户端 (300 行)
├── sync.rs            - 自动同步 (250 行)
└── test.rs            - 测试用例 (200 行)

总计: ~1750 行
大小: < 100KB
```

---

## ⚠️ 风险评估

### 法律风险
- ⚠️ **逆向工程**: 可能违反 Anthropic ToS
- ⚠️ **源码分析**: 需遵守 Apache 2.0 许可证

**缓解措施**:
- ✅ 只提取公开的网络协议信息
- ✅ 不复制 Claude Code 源码
- ✅ 参考公开文档和社区讨论

### 技术风险
- ⚠️ **签名算法未知**: 可能需要额外签名
- ⚠️ **协议变更**: Anthropic 可能更改协议

**缓解措施**:
- ✅ 实现自动同步机制
- ✅ 版本监控和告警
- ✅ 快速响应更新

---

## 🚀 执行计划

### Week 1: 信息收集 + 核心实现
- Day 1-2: 逆向分析，提取模板
- Day 3-4: 实现 TLS + Headers + Client
- Day 5: 测试和调试

### Week 2: 自动同步 + 集成
- Day 1-2: 实现自动同步
- Day 3: 集成到 FoxNIO
- Day 4: 文档编写
- Day 5: 发布和监控

---

## 📝 下一步

**立即行动**:
1. ✅ 创建新分支 `claude-code-shell`
2. ⏳ 开始信息收集（逆向分析）
3. ⏳ 实现核心模块

**是否现在开始逆向分析？**
