# FoxNIO 功能对齐计划

## 概述

本文档记录 FoxNIO 与行业标准 AI API 网关（如 Sub2API 等）的功能对比和对齐计划。

**更新时间**: 2026-03-30
**当前版本**: v0.2.0
**目标版本**: v1.0.0

## ⚠️ 重要说明

**模型配置策略**: FoxNIO 采用完全动态的模型配置方案，不硬编码任何模型列表。所有模型信息存储在数据库中，支持：
- 实时热加载
- 运行时增删改查
- 模型别名映射
- 自动降级配置

**服务商支持**: 当前支持 6 家主流 AI 服务商（OpenAI、Anthropic、Google、DeepSeek、Mistral、Cohere），未来可扩展。

## 已实现功能 ✅

### 核心 API 网关
- ✅ OpenAI 兼容 API（/v1/chat/completions, /v1/models）
- ✅ 多服务商支持（OpenAI、Anthropic、Google、DeepSeek、Mistral、Cohere）
- ✅ 动态模型配置（数据库持久化、热加载、运行时管理）
- ✅ 智能模型路由（别名解析、自动降级）
- ✅ 故障转移机制（指数退避重试）
- ✅ SSE 流式响应
- ✅ HTTP/2 支持
- ✅ 等待队列机制（粘性会话优先）**[NEW]**
- ✅ 模型级 RPM 限制 **[NEW]**

### 账户与调度
- ✅ 多账户管理
- ✅ 智能账户调度（负载感知、粘性会话）
- ✅ 等待队列机制
- ✅ 账户健康检查

### 认证与授权
- ✅ JWT 认证
- ✅ API Key 管理
- ✅ TOTP 两步验证
- ✅ OAuth 集成（GitHub、Google、LinuxDo、Antigravity）
- ✅ 基于角色的访问控制（RBAC）

### 计费与配额
- ✅ 订阅管理
- ✅ 配额控制（按用户/组）
- ✅ 使用统计
- ✅ 促销码系统
- ✅ 兑换码系统

### 监控与告警
- ✅ Prometheus 指标
- ✅ 健康检查（PostgreSQL、Redis、磁盘、内存）
- ✅ 告警系统（邮件、Slack、钉钉、飞书）
- ✅ 审计日志
- ✅ WebSocket 实时监控

### 安全特性
- ✅ AES-256-GCM 数据加密
- ✅ TLS 指纹识别
- ✅ 分布式速率限制
- ✅ 角色权限控制

### 管理功能
- ✅ 用户管理
- ✅ 账户管理
- ✅ 模型配置
- ✅ 群组管理
- ✅ 公告系统
- ✅ 备份恢复

### 图像/视频生成
- ✅ Sora 图像/视频生成支持

### 其他
- ✅ 响应压缩（gzip、brotli）
- ✅ 连接池优化
- ✅ Redis 缓存

## 待实现功能 📋

### 高优先级 (P0)

#### 1. Webhook 支持
**优先级**: P0 - 高
**预计工作量**: 3-5 天

**功能描述**:
- 事件订阅系统（账户状态变化、配额耗尽、错误告警等）
- Webhook 端点管理
- 事件重试机制
- 事件日志记录

**实现要点**:
```rust
// 新增实体
pub struct WebhookEndpoint {
    pub id: i64,
    pub user_id: i64,
    pub url: String,
    pub events: JsonValue, // ["account.failed", "quota.exhausted"]
    pub secret: String,
    pub enabled: bool,
}

// 新增服务
pub struct WebhookService {
    // 发送 webhook
    pub async fn send_webhook(&self, event: WebhookEvent) -> Result<()>;
    // 重试失败的 webhook
    pub async fn retry_failed(&self) -> Result<()>;
}
```

**API 端点**:
```
POST   /api/v1/webhooks          - 创建 webhook
GET    /api/v1/webhooks          - 列出 webhooks
GET    /api/v1/webhooks/:id      - 获取 webhook 详情
PUT    /api/v1/webhooks/:id      - 更新 webhook
DELETE /api/v1/webhooks/:id      - 删除 webhook
GET    /api/v1/webhooks/:id/logs - 获取 webhook 日志
```

---

#### 2. API 文档（OpenAPI/Swagger）
**优先级**: P0 - 高
**预计工作量**: 2-3 天

**功能描述**:
- 自动生成 OpenAPI 3.0 规范
- Swagger UI 界面
- API 测试工具
- 示例代码生成

**实现要点**:
- 使用 `utoipa` crate 自动生成 OpenAPI 文档
- 添加 `/api-docs/openapi.json` 端点
- 添加 `/swagger-ui` 界面
- 为所有 API 添加文档注释

---

#### 3. API Key 权限细分
**优先级**: P0 - 高
**预计工作量**: 2-3 天

**功能描述**:
- API Key 级别的权限控制
- 限制可访问的模型
- 限制请求速率
- 限制 IP 白名单
- 设置过期时间

**实现要点**:
```rust
pub struct ApiKeyPermissions {
    pub allowed_models: Option<Vec<String>>,  // 允许的模型列表
    pub rate_limit: Option<i32>,              // 请求速率限制
    pub ip_whitelist: Option<Vec<String>>,    // IP 白名单
    pub expires_at: Option<DateTime<Utc>>,    // 过期时间
    pub daily_quota: Option<i64>,             // 每日配额
}
```

---

### 中优先级 (P1)

#### 4. 模型性能监控
**优先级**: P1 - 中
**预计工作量**: 3-4 天

**功能描述**:
- 实时监控各模型的响应时间
- 成功率统计
- 错误率分析
- 成本追踪
- 性能报表

**实现要点**:
```rust
pub struct ModelMetrics {
    pub model_name: String,
    pub avg_response_time: f64,
    pub success_rate: f64,
    pub error_count: i64,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub timestamp: DateTime<Utc>,
}
```

**API 端点**:
```
GET /api/v1/admin/metrics/models          - 模型性能指标
GET /api/v1/admin/metrics/models/:name    - 特定模型详情
GET /api/v1/admin/metrics/models/:name/history - 历史数据
```

---

#### 5. 批量操作 API
**优先级**: P1 - 中
**预计工作量**: 2-3 天

**功能描述**:
- 批量创建 API Keys
- 批量删除资源
- 批量更新模型配置
- 批量导入用户

**API 端点**:
```
POST /api/v1/admin/api-keys/batch-create   - 批量创建 API Keys
POST /api/v1/admin/api-keys/batch-delete   - 批量删除 API Keys
POST /api/v1/admin/models/batch-import     - 批量导入模型
POST /api/v1/admin/users/batch-import      - 批量导入用户
```

---

#### 6. 模型能力自动更新
**优先级**: P1 - 中
**预计工作量**: 4-5 天

**功能描述**:
- 自动从各提供商获取最新模型列表（调用 `/v1/models` API）
- 自动更新模型价格、能力等信息
- 自动检测模型能力（vision、function calling 等）
- 模型变更通知（新增、弃用、价格调整）

**实现要点**:
```rust
pub struct ModelSyncService {
    // 从 OpenAI 同步模型（调用 GET /v1/models）
    pub async fn sync_openai_models(&self) -> Result<Vec<ModelInfo>>;
    // 从 Anthropic 同步模型
    pub async fn sync_anthropic_models(&self) -> Result<Vec<ModelInfo>>;
    // 从 Google 同步模型
    pub async fn sync_google_models(&self) -> Result<Vec<ModelInfo>>;
    // 定时同步任务
    pub async fn start_sync_scheduler(&self, interval: Duration);
}

// 同步结果
pub struct ModelSyncResult {
    pub provider: String,
    pub new_models: Vec<String>,
    pub updated_models: Vec<String>,
    pub deprecated_models: Vec<String>,
    pub price_changes: Vec<PriceChange>,
}
```

**API 端点**:
```
POST /api/v1/admin/models/sync              - 手动触发同步
GET  /api/v1/admin/models/sync/status       - 查看同步状态
GET  /api/v1/admin/models/sync/history      - 查看同步历史
```

---

#### 7. 成本优化建议
**优先级**: P1 - 中
**预计工作量**: 2-3 天

**功能描述**:
- 分析用户的使用模式
- 推荐更经济的模型
- 识别异常使用
- 生成成本报告

**API 端点**:
```
GET /api/v1/users/me/cost-optimization - 获取成本优化建议
GET /api/v1/admin/cost-analysis        - 管理员成本分析
```

---

### 低优先级 (P2)

#### 8. 国际化支持
**优先级**: P2 - 低
**预计工作量**: 5-7 天

**功能描述**:
- 多语言界面（中文、英文、日文等）
- 时区支持
- 货币本地化
- 错误消息本地化

---

#### 9. API 版本管理
**优先级**: P2 - 低
**预计工作量**: 3-4 天

**功能描述**:
- 支持 API 多版本共存（/v1、/v2）
- 版本迁移指南
- 版本废弃通知
- 版本兼容性检查

---

#### 10. 高级分析报表
**优先级**: P2 - 低
**预计工作量**: 4-5 天

**功能描述**:
- 使用趋势分析
- 用户行为分析
- 模型使用热度图
- 自定义报表生成
- 导出报表（CSV、PDF）

---

#### 11. 模型对比功能
**优先级**: P2 - 低
**预计工作量**: 3-4 天

**功能描述**:
- 同一提示词多模型对比
- 响应质量评分
- 成本对比
- 速度对比

**API 端点**:
```
POST /api/v1/compare - 多模型对比
```

---

#### 12. 测试与 Mock
**优先级**: P2 - 低
**预计工作量**: 2-3 天

**功能描述**:
- Mock API 端点（用于测试）
- 录制/回放功能
- 自动化测试集成

---

## 功能对比表

| 功能 | FoxNIO v0.2.0 | 行业标准 | 优先级 |
|------|--------------|---------|-------|
| OpenAI 兼容 API | ✅ | ✅ | - |
| 多服务商支持 | ✅ (6家) | ✅ | - |
| **动态模型配置** | ✅ | ✅ | - |
| 模型热加载 | ✅ | ✅ | - |
| 模型自动同步 | ❌ | ⚠️ | P1 |
| Webhook 支持 | ❌ | ✅ | P0 |
| OpenAPI 文档 | ❌ | ✅ | P0 |
| API Key 权限细分 | ⚠️ 基础 | ✅ | P0 |
| 模型性能监控 | ⚠️ 基础 | ✅ | P1 |
| 批量操作 API | ❌ | ✅ | P1 |
| 成本优化建议 | ❌ | ✅ | P1 |
| 国际化 | ❌ | ✅ | P2 |
| API 版本管理 | ❌ | ⚠️ | P2 |
| 高级报表 | ⚠️ 基础 | ✅ | P2 |
| 模型对比 | ❌ | ⚠️ | P2 |
| Mock API | ❌ | ⚠️ | P2 |

**图例**:
- ✅ 完全实现
- ⚠️ 部分实现
- ❌ 未实现

## 实施计划

### Sprint 1 (Week 1-2): P0 功能
1. **Week 1**: Webhook 支持
2. **Week 2**: OpenAPI 文档 + API Key 权限细分

### Sprint 2 (Week 3-4): P1 功能
1. **Week 3**: 模型性能监控 + 批量操作 API
2. **Week 4**: 模型能力自动更新 + 成本优化建议

### Sprint 3 (Week 5-6): P2 功能
1. **Week 5**: 国际化支持 + API 版本管理
2. **Week 6**: 高级报表 + 模型对比 + Mock API

## 成功指标

- 所有 P0 功能在 Sprint 1 结束时完成
- API 文档覆盖率 > 95%
- 模型同步延迟 < 1 小时
- Webhook 可靠性 > 99%
- 用户满意度 > 4.5/5

## 相关文档

- [API 参考](API_REFERENCE.md)
- [模块参考](MODULE_REFERENCE.md)
- [数据库架构](DATABASE_SCHEMA.md)
- [部署指南](DEPLOYMENT.md)
- [开发指南](DEVELOPMENT.md)
