# FoxNIO 对齐规划 - 下一阶段计划

**制定时间**: 2026-03-28 12:05 GMT+8  
**制定者**: FoxNIO 对齐规划主管 (AI Agent)  
**版本**: v2.0

---

## 📊 当前状态概览

| 指标 | 数值 |
|------|------|
| **FoxNIO 对齐度** | 65% |
| **已实现功能** | OAuth、动态模型、分组、WebSocket、健康评分、粘性会话、QPS监控 |
| **测试状态** | 全部通过 |
| **代码质量** | 60% 测试覆盖率，0 个 allow(dead_code) |
| **待对齐功能** | 5 大类，约 20 项 |

---

## 🆕 sub2api 最新功能分析 (2026-03)

### 1. TLS 指纹 Profile (新增，8,000+ 行代码)

#### 功能概述
TLS 指纹模板用于模拟特定客户端（如 Claude Code / Node.js）的 TLS 握手特征，通过定制 ClientHello 参数来绕过上游平台的客户端识别。

#### 核心特性
- **数据库管理**: `tls_fingerprint_profiles` 表存储模板配置
- **账号绑定**: 通过 `Account.Extra.tls_fingerprint_profile_id` 绑定
- **完整 ClientHello 参数**:
  - `cipher_suites`: TLS 加密套件列表（顺序敏感，影响 JA3）
  - `curves`: 椭圆曲线/支持的组列表
  - `point_formats`: EC 点格式列表
  - `signature_algorithms`: 签名算法列表
  - `alpn_protocols`: ALPN 协议列表
  - `supported_versions`: 支持的 TLS 版本列表
  - `key_share_groups`: Key Share 中发送的曲线组
  - `psk_modes`: PSK 密钥交换模式
  - `extensions`: TLS 扩展类型 ID 列表
- **GREASE 扩展**: Chrome 风格随机扩展支持
- **运行时解析**: `ResolveTLSProfile()` 动态选择模板

#### 技术架构
```
数据库 (tls_fingerprint_profiles)
    ↓
Repository (TLSFingerprintProfileRepository)
    ↓
Service (TLSFingerprintProfileService)
    ├── 本地缓存 (map[int64]*Profile)
    ├── Redis 缓存 (订阅更新)
    └── CRUD API
    ↓
Gateway (DoWithTLS → dialer 应用 Profile)
```

#### 重要性评估
| 维度 | 评分 | 说明 |
|------|------|------|
| 用户需求 | ⭐⭐⭐⭐ | 防止被上游识别为代理，提升账号稳定性 |
| 实现复杂度 | ⭐⭐⭐⭐⭐ | 需要深度理解 TLS 协议，Go 特有库难以移植 |
| 与现有功能关系 | ⭐⭐⭐ | 独立功能，但需要与账号管理集成 |

---

### 2. OpenAI 隐私模式 (新增)

#### 功能概述
自动调用 ChatGPT 后端 API 关闭"改进模型"选项，保护用户隐私，防止对话内容被用于训练。

#### 核心特性
- **隐私设置 API**: `PATCH /backend-api/settings/account_user_setting`
- **异步设置**: 在 Token 刷新时自动触发
- **状态追踪**: `privacy_mode` 字段记录设置状态
  - `training_off`: 成功关闭
  - `training_set_failed`: 设置失败
  - `training_set_cf_blocked`: 被 Cloudflare 拦截
- **手动触发**: 前端提供手动设置按钮
- **智能重试**: 刷新失败时自动重试

#### 技术实现
```go
func disableOpenAITraining(ctx context.Context, clientFactory, accessToken, proxyURL) string {
    // 调用 ChatGPT 设置 API
    // feature=training_allowed&value=false
    // 返回状态码: training_off / failed / cf_blocked
}
```

#### 重要性评估
| 维度 | 评分 | 说明 |
|------|------|------|
| 用户需求 | ⭐⭐⭐ | 隐私保护需求存在，但非核心功能 |
| 实现复杂度 | ⭐⭐ | 简单的 HTTP 调用，易于实现 |
| 与现有功能关系 | ⭐⭐ | 仅适用于 OpenAI OAuth 账号 |

---

### 3. Responses API 支持 (新增)

#### 功能概述
支持 OpenAI 新的 Responses API 格式，实现 Responses ↔ Anthropic 格式双向转换，使 Anthropic 平台组能够服务 Responses API 客户端。

#### 核心特性
- **新路由**: `POST /v1/responses`
- **格式转换**:
  - `ResponsesToAnthropicRequest()`: Responses → Anthropic Messages
  - `AnthropicToResponsesResponse()`: Anthropic → Responses
  - `AnthropicEventToResponsesEvents()`: 流式事件转换
- **平台级路由**: 支持 Anthropic 平台组接收 Responses 请求
- **Claude Code 限制**: `/v1/responses` 端点拒绝 Claude Code Only 分组

#### 技术架构
```
客户端 (Responses API 格式)
    ↓
POST /v1/responses
    ↓
ForwardAsResponses()
    ├── 解析 ResponsesRequest
    ├── ResponsesToAnthropicRequest() 转换
    ├── 发送到 Anthropic 上游
    ├── 接收 Anthropic SSE 响应
    └── AnthropicEventToResponsesEvents() 转换
    ↓
客户端 (Responses API 格式)
```

#### 重要性评估
| 维度 | 评分 | 说明 |
|------|------|------|
| 用户需求 | ⭐⭐⭐⭐ | OpenAI 新 API，客户端逐渐迁移 |
| 实现复杂度 | ⭐⭐⭐⭐ | 需要完整的格式转换逻辑 |
| 与现有功能关系 | ⭐⭐⭐⭐ | 与 Anthropic 支持紧密相关 |

---

### 4. 请求整流器增强

#### 功能概述
增强请求处理管道，支持 API Key 账号签名整流、Wire Casing 保持、转发行为开关。

#### 核心特性
- **API Key 账号签名整流**: 确保请求签名正确传递
- **Wire Casing 保持**: 保持原始请求格式
- **转发行为开关**: 可配置转发策略

#### 重要性评估
| 维度 | 评分 | 说明 |
|------|------|------|
| 用户需求 | ⭐⭐⭐ | 优化请求处理，提升兼容性 |
| 实现复杂度 | ⭐⭐⭐ | 需要深入理解请求处理流程 |
| 与现有功能关系 | ⭐⭐⭐⭐ | 与网关核心功能相关 |

---

### 5. 其他增强

| 功能 | 描述 | 重要性 |
|------|------|--------|
| **Antigravity TierInfo** | 显示账号套餐等级信息 | ⭐⭐⭐ |
| **Antigravity 隐私设置** | Antigravity 平台隐私配置 | ⭐⭐ |
| **Mobile RT 手动输入入口** | 移动端实时 API 手动配置 | ⭐⭐ |
| **自定义端点配置** | 灵活配置上游端点 | ⭐⭐⭐ |
| **错误可观测性字段** | OpsUpstreamErrorEvent 增强 | ⭐⭐⭐ |

---

## 🎯 下一阶段对齐计划

### P0 功能 (必须实现，预计 25-35 人天)

#### 1. TLS 指纹 Profile 支持 (10-15 人天) ⭐ **最高优先级**

##### 功能描述
实现 TLS 指纹模板管理，支持账号绑定自定义 TLS Profile，模拟特定客户端的 TLS 握手特征。

##### 实现方案

**阶段 1: 数据模型 (2 人天)**
```rust
// src/entity/tls_fingerprint_profile.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tls_fingerprint_profiles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enable_grease: bool,
    pub cipher_suites: Vec<u16>,
    pub curves: Vec<u16>,
    pub point_formats: Vec<u16>,
    pub signature_algorithms: Vec<u16>,
    pub alpn_protocols: Vec<String>,
    pub supported_versions: Vec<u16>,
    pub key_share_groups: Vec<u16>,
    pub psk_modes: Vec<u16>,
    pub extensions: Vec<u16>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}
```

**阶段 2: 服务层 (4 人天)**
```rust
// src/service/tls_fingerprint.rs
pub struct TLSFingerprintService {
    db: DatabaseConnection,
    redis: RedisConnection,
    local_cache: Arc<RwLock<HashMap<i64, Profile>>>,
}

impl TLSFingerprintService {
    pub async fn list(&self) -> Result<Vec<Profile>>;
    pub async fn create(&self, profile: Profile) -> Result<Profile>;
    pub async fn update(&self, profile: Profile) -> Result<Profile>;
    pub async fn delete(&self, id: i64) -> Result<()>;
    pub fn resolve_profile(&self, account: &Account) -> Option<Profile>;
}
```

**阶段 3: TLS Dialer 集成 (4 人天)**
```rust
// 使用 rustls 或 native-tls 配置 ClientHello
// 参考: https://docs.rs/rustls/latest/rustls/

pub struct TLSFingerprintDialer {
    profile: Option<Profile>,
}

impl TLSFingerprintDialer {
    pub fn new(profile: Option<Profile>) -> Self;
    pub async fn connect(&self, url: &str) -> Result<TlsStream>;
}
```

**阶段 4: 账号绑定 (2 人天)**
```rust
// 在 Account.Extra 中添加 tls_fingerprint_profile_id
pub struct AccountExtra {
    // ... 现有字段
    pub tls_fingerprint_profile_id: Option<i64>,
}
```

**阶段 5: 测试与文档 (2 人天)**
- 单元测试
- 集成测试
- API 文档

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 数据模型 | 2 人天 |
| 服务层 | 4 人天 |
| TLS Dialer | 4 人天 |
| 账号绑定 | 2 人天 |
| 测试文档 | 2 人天 |
| **总计** | **14 人天** |

##### 优先级理由
1. **关键安全功能**: 防止被上游平台识别为代理
2. **账号稳定性**: 直接影响 OAuth 账号的存活时间
3. **竞品差异**: sub2api 核心优势功能
4. **技术挑战**: 需要深入 TLS 协议，但实现后形成技术壁垒

---

#### 2. Responses API 支持 (8-12 人天)

##### 功能描述
支持 OpenAI Responses API 格式，实现 Responses ↔ Anthropic 双向格式转换。

##### 实现方案

**阶段 1: 数据结构定义 (2 人天)**
```rust
// src/gateway/responses.rs
#[derive(Serialize, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: ResponsesInput,
    #[serde(default)]
    pub stream: bool,
    pub reasoning: Option<ReasoningConfig>,
}

#[derive(Serialize, Deserialize)]
pub struct ResponsesResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub output: ResponsesOutput,
    pub usage: Usage,
}
```

**阶段 2: 格式转换器 (4 人天)**
```rust
// src/gateway/responses_converter.rs
pub fn responses_to_anthropic(req: &ResponsesRequest) -> AnthropicRequest;
pub fn anthropic_to_responses(resp: &AnthropicResponse) -> ResponsesResponse;
pub fn anthropic_event_to_responses_events(event: &AnthropicStreamEvent) -> Vec<ResponsesEvent>;
```

**阶段 3: 路由集成 (2 人天)**
```rust
// src/handler/gateway.rs
pub async fn responses(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    // 转换 Responses → Anthropic
    // 转发到 Anthropic 上游
    // 转换响应 → Responses
}
```

**阶段 4: 测试 (2 人天)**
- 格式转换测试
- 流式响应测试
- 端到端测试

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 数据结构 | 2 人天 |
| 格式转换 | 4 人天 |
| 路由集成 | 2 人天 |
| 测试 | 2 人天 |
| **总计** | **10 人天** |

##### 优先级理由
1. **API 趋势**: OpenAI 正在推广 Responses API
2. **客户端兼容**: 部分客户端开始使用新 API
3. **平台支持**: 使 Anthropic 平台能服务更多客户端

---

#### 3. OpenAI 隐私模式 (3-5 人天)

##### 功能描述
自动关闭 OpenAI 账号的"改进模型"选项，保护用户隐私。

##### 实现方案

**阶段 1: 服务实现 (2 人天)**
```rust
// src/service/openai_privacy.rs
pub struct OpenAIPrivacyService {
    http_client: reqwest::Client,
}

impl OpenAIPrivacyService {
    pub async fn disable_training(&self, access_token: &str, proxy_url: Option<&str>) -> PrivacyMode;
    pub async fn fetch_account_info(&self, access_token: &str, proxy_url: Option<&str>) -> Option<ChatGPTAccountInfo>;
}
```

**阶段 2: Token 刷新集成 (2 人天)**
```rust
// 在 Token 刷新时自动触发隐私设置
async fn refresh_token_with_privacy(&self, account: &Account) -> Result<()> {
    let token = self.refresh_token(account).await?;
    if account.platform == Platform::OpenAI {
        self.privacy_service.disable_training(&token, &account.proxy_url).await;
    }
    Ok(())
}
```

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 服务实现 | 2 人天 |
| Token 刷新集成 | 1 人天 |
| 测试 | 1 人天 |
| **总计** | **4 人天** |

##### 优先级理由
1. **隐私保护**: 用户日益重视隐私
2. **易于实现**: 简单的 HTTP 调用
3. **增值功能**: 差异化特性

---

### P1 功能 (建议实现，预计 15-20 人天)

#### 4. 请求整流器增强 (5-7 人天)

##### 功能描述
增强请求处理管道，支持签名整流、Wire Casing 保持。

##### 实现方案

**阶段 1: 签名整流 (3 人天)**
```rust
// src/gateway/request_signer.rs
pub struct RequestSigner {
    api_key: String,
}

impl RequestSigner {
    pub fn sign(&self, request: &mut Request);
    pub fn verify(&self, request: &Request) -> bool;
}
```

**阶段 2: Wire Casing (2 人天)**
```rust
// 保持原始请求格式
pub struct WireCasing {
    preserve_original_format: bool,
}

impl WireCasing {
    pub fn wrap(&self, request: Request) -> WireRequest;
    pub fn unwrap(&self, wire: WireRequest) -> Request;
}
```

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 签名整流 | 3 人天 |
| Wire Casing | 2 人天 |
| 测试 | 2 人天 |
| **总计** | **7 人天** |

---

#### 5. 错误可观测性增强 (3-4 人天)

##### 功能描述
增强上游错误追踪，提供更详细的错误上下文。

##### 实现方案

```rust
// src/model/ops_error.rs
pub struct OpsUpstreamErrorEvent {
    pub platform: Platform,
    pub account_id: i64,
    pub account_name: String,
    pub upstream_status_code: u16,
    pub upstream_request_id: Option<String>,
    pub kind: String,
    pub message: String,
}

// 在 GatewayService 中记录
fn record_upstream_error(&self, event: OpsUpstreamErrorEvent) {
    // 记录到日志
    // 发送到监控系统
    // 更新健康评分
}
```

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 模型定义 | 1 人天 |
| 集成到网关 | 2 人天 |
| 测试 | 1 人天 |
| **总计** | **4 人天** |

---

#### 6. Antigravity 增强 (5-8 人天)

##### 功能描述
添加 TierInfo 显示、隐私设置支持。

##### 实现方案

**阶段 1: TierInfo (3 人天)**
```rust
// src/service/antigravity/tier_info.rs
pub struct TierInfo {
    pub tier: String,
    pub credits: u64,
    pub usage: u64,
}

impl AntigravityService {
    pub async fn fetch_tier_info(&self, account: &Account) -> Result<TierInfo>;
}
```

**阶段 2: 隐私设置 (2 人天)**
```rust
// src/service/antigravity/privacy.rs
impl AntigravityService {
    pub async fn configure_privacy(&self, account: &Account) -> Result<()>;
}
```

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| TierInfo | 3 人天 |
| 隐私设置 | 2 人天 |
| 测试 | 2 人天 |
| **总计** | **7 人天** |

---

#### 7. 自定义端点配置 (3-5 人天)

##### 功能描述
支持灵活配置上游端点，满足不同部署需求。

##### 实现方案

```rust
// src/entity/custom_endpoint.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "custom_endpoints")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub platform: Platform,
    pub base_url: String,
    pub headers: HashMap<String, String>,
    pub enabled: bool,
}

// 在账号中引用
pub struct Account {
    // ...
    pub custom_endpoint_id: Option<i64>,
}
```

##### 工作量估算
| 任务 | 工作量 |
|------|--------|
| 数据模型 | 1 人天 |
| 服务层 | 2 人天 |
| 网关集成 | 1 人天 |
| 测试 | 1 人天 |
| **总计** | **5 人天** |

---

### P2 功能 (可选实现，预计 8-12 人天)

#### 8. Mobile RT 手动输入入口 (2-3 人天)

##### 功能描述
为移动端实时 API 提供手动配置入口。

##### 工作量估算
2-3 人天

---

#### 9. 其他小功能 (5-8 人天)

| 功能 | 工作量 |
|------|--------|
| 账号状态丰富 | 2 人天 |
| API Key 元数据 | 1 人天 |
| 日志增强 | 2 人天 |

---

## 📊 功能对比表

| 功能 | sub2api | FoxNIO | 对齐状态 | 优先级 | 工作量 |
|------|---------|--------|----------|--------|--------|
| **TLS 指纹 Profile** | ✅ 完整 | ❌ 无 | 🔴 需实现 | P0 | 10-15 天 |
| **Responses API** | ✅ 完整 | ❌ 无 | 🔴 需实现 | P0 | 8-12 天 |
| **OpenAI 隐私模式** | ✅ 完整 | ❌ 无 | 🔴 需实现 | P0 | 3-5 天 |
| **请求整流器增强** | ✅ 增强 | ⚠️ 基础 | 🟡 需增强 | P1 | 5-7 天 |
| **错误可观测性** | ✅ 增强 | ⚠️ 基础 | 🟡 需增强 | P1 | 3-4 天 |
| **Antigravity TierInfo** | ✅ 有 | ❌ 无 | 🟡 需实现 | P1 | 3-4 天 |
| **自定义端点** | ✅ 有 | ❌ 无 | 🟡 需实现 | P1 | 3-5 天 |
| **Mobile RT 入口** | ✅ 有 | ❌ 无 | 🟢 可选 | P2 | 2-3 天 |

---

## 📈 优先级矩阵

```
高用户需求
    ↑
    │  TLS 指纹 ★        Responses API ★
    │
    │  隐私模式 ★        请求整流器
    │
    │  错误可观测性      Antigravity 增强
    │
    │  自定义端点        Mobile RT 入口
    │
    └────────────────────────────────→ 高实现复杂度
```

---

## 🗓️ 执行路线图

### Phase 1: 核心安全功能 (2-3 周)

**Week 1-2: TLS 指纹 Profile**
- 数据模型设计
- 服务层实现
- TLS Dialer 集成

**Week 3: 测试与优化**
- 单元测试
- 集成测试
- 性能优化

### Phase 2: API 兼容性 (1-2 周)

**Week 1: Responses API**
- 数据结构定义
- 格式转换器

**Week 2: 集成与测试**
- 路由集成
- 端到端测试

### Phase 3: 增强功能 (2-3 周)

**Week 1: OpenAI 隐私模式**
- 服务实现
- Token 刷新集成

**Week 2-3: 其他 P1 功能**
- 请求整流器增强
- 错误可观测性
- 自定义端点

### Phase 4: 完善与优化 (1 周)

- 文档更新
- 性能优化
- 代码审查

---

## 💰 资源需求

### 人力资源

| 角色 | 数量 | 职责 |
|------|------|------|
| **后端开发** | 2 人 | 核心功能实现 |
| **测试工程师** | 1 人 | 测试用例编写、自动化测试 |
| **技术文档** | 0.5 人 | API 文档、用户手册 |

### 技术资源

| 资源 | 用途 |
|------|------|
| **Rust TLS 库** | rustls / native-tls-custom |
| **测试环境** | OpenAI / Anthropic 测试账号 |
| **监控工具** | Prometheus / Grafana |

---

## ⚠️ 风险与挑战

### TLS 指纹 Profile

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **Rust TLS 库限制** | 高 | 研究原生 TLS 配置方案，必要时使用 FFI |
| **协议理解难度** | 中 | 参考 sub2api 实现，学习 TLS 规范 |
| **上游对抗升级** | 中 | 保持模板更新，监控成功率 |

### Responses API

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **格式变化** | 中 | 持续监控 OpenAI 文档更新 |
| **兼容性问题** | 中 | 充分测试，逐步上线 |

---

## 📝 总结

### 核心结论

1. **TLS 指纹 Profile 是最高优先级功能**
   - 关键安全功能
   - 直接影响账号稳定性
   - 技术壁垒价值

2. **Responses API 是必要的兼容性功能**
   - OpenAI 新 API 趋势
   - 扩大客户端兼容性

3. **隐私模式是增值功能**
   - 用户隐私保护需求
   - 实现简单，投入产出比高

### 工作量汇总

| 优先级 | 工作量 | 日历时间 (1人) | 日历时间 (2人) |
|--------|--------|----------------|----------------|
| P0 | 25-35 人天 | 25-35 天 | 13-18 天 |
| P1 | 15-20 人天 | 15-20 天 | 8-10 天 |
| P2 | 8-12 人天 | 8-12 天 | 4-6 天 |
| **总计** | **48-67 人天** | **48-67 天** | **25-34 天** |

### 建议执行策略

1. **立即启动 P0 功能开发**
   - 优先 TLS 指纹 Profile
   - 并行开展 Responses API

2. **按需启动 P1 功能**
   - 根据用户反馈决定优先级
   - 与 P0 功能穿插进行

3. **P2 功能视资源情况决定**
   - 低优先级
   - 可延后到下一迭代

---

**制定完成时间**: 2026-03-28 12:30 GMT+8  
**下一步行动**: 启动 TLS 指纹 Profile 数据模型设计
