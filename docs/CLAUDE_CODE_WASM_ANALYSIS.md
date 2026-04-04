# Claude Code WASM 嵌入可行性分析

> 历史研究记录。
> 本文档讨论的是一个专项技术探索，不代表 FoxNIO 当前产品主线、发布状态或项目完成度。

**分析时间**: 2026-03-30 18:35
**目标**: 探索将 Claude Code WASM 嵌入 FoxNIO 的可能性

---

## 🎯 核心想法

**用户需求**:
- 直接嵌入最新 Claude Code 的 WASM
- 随 Claude Code 更新而更新
- 避免风控和指纹问题

---

## 📊 现有实现分析

### 当前 FoxNIO Claude Code 支持

**文件结构**:
```
backend/src/gateway/claude/
├── tls.rs           - TLS 指纹配置 (Node.js 24.x)
├── validator.rs     - Claude Code 验证器
├── headers.rs       - HTTP 头配置
├── header_util.rs   - 头工具
├── constants.rs     - 常量定义
└── tool_use.rs      - 工具调用处理

backend/src/service/
├── claude_code_validator.rs - 代码验证
├── claude_token_provider.rs - Token 管理
└── oauth/claude.rs           - OAuth 集成
```

**当前方案**:
- ✅ TLS 指纹模拟 (Node.js 24.x)
- ✅ User-Agent 验证
- ✅ System Prompt 验证
- ⚠️ 需要持续维护指纹库
- ⚠️ 可能被 Anthropic 检测

---

## 🔍 Claude Code 技术分析

### Claude Code 架构

**官方组件**:
1. **CLI 核心** (TypeScript/Node.js)
   - 消息处理
   - 工具调用
   - 流式响应

2. **TLS 客户端**
   - Node.js 24.x TLS 栈
   - JA3/JA4 指纹

3. **Anthropic API 客户端**
   - 认证管理
   - 请求签名

**是否有 WASM 版本**？
- ❌ Anthropic 未发布官方 WASM 版本
- ❌ Claude Code CLI 未编译为 WASM
- ⚠️ 可能需要自行编译或逆向工程

---

## 💡 WASM 嵌入方案探索

### 方案 1: 自行编译 Claude Code → WASM ⚠️

**步骤**:
1. 克隆 Claude Code 源码
2. 使用 `wasm-pack` 或 `wasmpack` 编译
3. 集成到 FoxNIO

**技术栈**:
```
Claude Code (TypeScript)
    ↓ 编译
WASM (WebAssembly)
    ↓ 集成
FoxNIO (Rust + wasmtime)
```

**挑战**:
- ❌ Claude Code 未开源（闭源）
- ❌ 依赖 Node.js 运行时（难以编译为 WASM）
- ⚠️ 法律风险（逆向工程可能违反 ToS）

**可行性**: **低**

---

### 方案 2: 使用 Anthropic SDK WASM 版本 ⚠️

**现状**:
- Anthropic 官方 SDK 有 JavaScript/Python 版本
- ❌ 无官方 WASM 版本
- 社区可能有非官方移植

**步骤**:
1. 查找社区 WASM 移植
2. 验证功能和稳定性
3. 集成到 FoxNIO

**挑战**:
- ⚠️ 非官方版本可能不稳定
- ⚠️ 更新滞后
- ⚠️ 仍需处理 TLS 指纹

**可行性**: **中低**

---

### 方案 3: 嵌入 V8/QuickJS WASM 运行时 ✅

**核心思路**:
- 将 Claude Code CLI 作为 JS 脚本
- 嵌入 WASM 版本的 V8 或 QuickJS
- 在 WASM 中运行 Claude Code

**架构**:
```
FoxNIO (Rust)
    ↓ 加载
QuickJS/V8 WASM
    ↓ 执行
Claude Code CLI (JavaScript)
    ↓ 调用
Anthropic API
```

**优势**:
- ✅ Claude Code 原生行为
- ✅ 自动跟随更新（只需更新 JS 脚本）
- ✅ 无需维护指纹库

**实现**:

```rust
// backend/src/gateway/claude_wasm.rs
use wasmer::{Instance, Module, Store, WasmerEnv};
use wasmer_wasi::WasiEnv;

pub struct ClaudeCodeWasm {
    instance: Instance,
    store: Store,
}

impl ClaudeCodeWasm {
    /// 从 WASM 文件加载 Claude Code
    pub async fn load(wasm_path: &str) -> Result<Self> {
        let mut store = Store::default();
        let module = Module::from_file(&store, wasm_path)?;

        // WASI 环境配置
        let wasi_env = WasiEnv::builder("claude-code")
            .inherit_stdio()
            .args(&["claude", "code"])
            .build();

        let instance = Instance::new(&mut store, &module, &wasi_env)?;

        Ok(Self { instance, store })
    }

    /// 执行 Claude Code 请求
    pub async fn execute(&mut self, prompt: &str) -> Result<String> {
        // 调用 WASM 导出的函数
        let run = self.instance.exports
            .get_function("run")?;

        let result = run.call(&mut self.store, &[prompt.into()])?;

        Ok(result[0].to_string())
    }
}
```

**挑战**:
- ⚠️ WASM 运行时开销
- ⚠️ Claude Code CLI 可能依赖 Node.js 特定 API
- ⚠️ 需要打包所有依赖

**可行性**: **中**

---

### 方案 4: 使用真实 Claude Code CLI + 进程隔离 ✅

**核心思路**:
- 不使用 WASM，而是直接运行 Claude Code CLI
- 使用进程隔离和沙箱技术
- 通过 IPC/RPC 与 FoxNIO 通信

**架构**:
```
FoxNIO (Rust)
    ↓ IPC/RPC
Claude Code CLI (Node.js 进程)
    ↓ 真实调用
Anthropic API
```

**优势**:
- ✅ 100% 原生行为
- ✅ 自动跟随官方更新
- ✅ 无需逆向工程
- ✅ 隔离安全

**实现**:

```rust
// backend/src/gateway/claude_process.rs
use tokio::process::Command;
use std::process::Stdio;

pub struct ClaudeCodeProcess {
    cli_path: String,
}

impl ClaudeCodeProcess {
    /// 调用 Claude Code CLI
    pub async fn execute(&self, prompt: &str) -> Result<String> {
        let output = Command::new(&self.cli_path)
            .arg("code")
            .arg("--prompt")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?)
        } else {
            Err(anyhow::anyhow!("Claude Code failed: {:?}",
                String::from_utf8(output.stderr)))
        }
    }

    /// 检查 Claude Code 版本
    pub async fn check_version(&self) -> Result<String> {
        let output = Command::new(&self.cli_path)
            .arg("--version")
            .output()
            .await?;

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }
}
```

**优势**:
- ✅ 最简单、最可靠
- ✅ 完全合规
- ✅ 易于维护

**劣势**:
- ⚠️ 需要 Node.js 运行时
- ⚠️ 需要安装 Claude Code CLI

**可行性**: **高**

---

### 方案 5: 复用 Claude Code 的请求模板 ✅

**核心思路**:
- 不运行 Claude Code 本身
- 只复制其请求模板和签名逻辑
- 定期同步更新

**实现**:

```rust
// backend/src/gateway/claude_template.rs
pub struct ClaudeCodeTemplate {
    /// 请求模板（从 Claude Code 提取）
    request_template: RequestTemplate,

    /// 签名算法（逆向分析）
    signature_algorithm: SignatureAlgorithm,

    /// 更新源（GitHub Release 或官方 API）
    update_source: String,
}

impl ClaudeCodeTemplate {
    /// 从 Claude Code 发布版本同步模板
    pub async fn sync_from_release(&mut self) -> Result<()> {
        // 下载最新 Claude Code CLI
        let cli_url = "https://github.com/anthropics/claude-code/releases/latest";

        // 提取请求模板
        let template = self.extract_template(&cli_content).await?;

        self.request_template = template;

        Ok(())
    }

    /// 生成与 Claude Code 一致的请求
    pub fn generate_request(&self, prompt: &str) -> Request {
        Request {
            headers: self.request_template.headers.clone(),
            body: self.format_body(prompt),
            signature: self.sign_request(),
        }
    }
}
```

**优势**:
- ✅ 无需运行完整 CLI
- ✅ 性能开销小
- ✅ 可自动化更新

**劣势**:
- ⚠️ 需要逆向工程
- ⚠️ 法律风险
- ⚠️ 可能被检测

**可行性**: **中**

---

## 📊 方案对比

| 方案 | 可行性 | 合规性 | 维护成本 | 性能 | 推荐度 |
|------|--------|--------|----------|------|--------|
| 1. 自行编译 WASM | 低 | ❌ 低 | 高 | 中 | ⭐ |
| 2. SDK WASM 版本 | 中低 | ⚠️ 中 | 中 | 中 | ⭐⭐ |
| 3. 嵌入 JS 运行时 | 中 | ✅ 高 | 中 | 低 | ⭐⭐⭐ |
| 4. 真实 CLI 进程 | 高 | ✅ 高 | 低 | 中 | ⭐⭐⭐⭐⭐ |
| 5. 请求模板复制 | 中 | ⚠️ 低 | 中 | 高 | ⭐⭐⭐ |

---

## 🎯 推荐方案

### 最佳方案: **方案 4 - 真实 CLI 进程隔离**

**理由**:
1. ✅ **完全合规** - 使用官方 CLI
2. ✅ **原生行为** - 100% 与 Claude Code 一致
3. ✅ **自动更新** - 只需更新 CLI 版本
4. ✅ **易于实现** - 无需逆向工程
5. ✅ **无风控风险** - 真实客户端行为

**实现路径**:

#### Phase 1: 基础集成 (1-2 天)
```rust
// 1. 添加 Claude Code CLI 管理器
pub struct ClaudeCodeManager {
    cli_path: PathBuf,
    version: String,
}

// 2. 实现进程调用
impl ClaudeCodeManager {
    pub async fn install(&self) -> Result<()> {
        // npm install -g @anthropic-ai/claude-code
    }

    pub async fn update(&self) -> Result<()> {
        // npm update -g @anthropic-ai/claude-code
    }

    pub async fn execute(&self, request: Request) -> Result<Response> {
        // 调用 CLI 并返回结果
    }
}
```

#### Phase 2: 进程池管理 (2-3 天)
```rust
// 3. 实现进程池
pub struct ClaudeCodePool {
    processes: Vec<ClaudeCodeProcess>,
    max_concurrent: usize,
}

// 4. 负载均衡
impl ClaudeCodePool {
    pub async fn get_available(&self) -> Result<ClaudeCodeProcess> {
        // 获取空闲进程
    }
}
```

#### Phase 3: 监控和自动更新 (1-2 天)
```rust
// 5. 版本监控
pub struct ClaudeCodeMonitor {
    check_interval: Duration,
}

impl ClaudeCodeMonitor {
    pub async fn start_monitoring(&self) {
        // 定期检查新版本
        // 自动更新
    }
}
```

---

## 🚀 实施建议

### 立即行动
1. **安装 Claude Code CLI**
   ```bash
   npm install -g @anthropic-ai/claude-code
   ```

2. **创建集成模块**
   ```bash
   touch backend/src/gateway/claude_cli.rs
   ```

3. **实现基础调用**
   - 进程管理
   - 参数传递
   - 结果解析

### 中期优化
1. 进程池和并发控制
2. 自动更新机制
3. 错误处理和重试

### 长期增强
1. 性能优化（缓存、预热）
2. 多版本管理
3. 降级策略

---

## ⚠️ 风险评估

### 法律风险
- ✅ **方案 4**: 无风险（使用官方工具）
- ⚠️ **方案 1/5**: 中风险（逆向工程）

### 技术风险
- ✅ **方案 4**: 低风险（成熟技术）
- ⚠️ **方案 3**: 中风险（WASM 运行时复杂）

### 维护风险
- ✅ **方案 4**: 低风险（自动更新）
- ⚠️ **方案 5**: 高风险（需持续逆向）

---

## 📝 结论

**推荐采用方案 4（真实 CLI 进程隔离）**，原因：

1. ✅ 技术可行性最高
2. ✅ 完全合规，无法律风险
3. ✅ 维护成本最低
4. ✅ 自动跟随官方更新
5. ✅ 无需关心指纹和风控

**WASM 方案当前不可行**，因为：
- ❌ Claude Code 无官方 WASM 版本
- ❌ 自行编译法律风险高
- ❌ 技术复杂度超过收益

**下一步**: 启动方案 4 的实现，创建 Claude Code CLI 集成模块。

---

**是否立即开始实现方案 4？**
