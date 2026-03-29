# FoxNIO 动态模型列表功能实现

## 概述

实现了动态模型列表功能，替代原有的硬编码模型配置。支持数据库持久化、运行时热加载、模型别名映射、模型降级等功能。

## 新增文件

### 1. 数据库实体 - `src/entity/model_configs.rs`

定义了模型配置的数据库实体：

```rust
pub struct Model {
    pub id: i64,
    pub name: String,           // 模型名称（唯一标识）
    pub aliases: Option<JsonValue>,  // 别名列表
    pub provider: String,       // 提供商
    pub api_name: String,       // API 名称
    pub display_name: String,   // 显示名称
    pub input_price: f64,       // 输入价格
    pub output_price: f64,      // 输出价格
    pub max_tokens: i32,        // 最大输出 tokens
    pub context_window: i32,    // 上下文窗口
    pub max_concurrent: i32,    // 最大并发数
    pub fallback_models: Option<JsonValue>,  // 降级模型
    pub capabilities: Option<JsonValue>,     // 模型能力
    pub supports_streaming: bool,
    pub supports_function_calling: bool,
    pub supports_vision: bool,
    pub enabled: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 2. 模型注册服务 - `src/service/model_registry.rs`

核心功能：
- 从数据库加载模型配置
- 运行时热加载（reload）
- 模型路由（支持别名解析）
- 模型降级（fallback）
- CRUD 操作
- 默认模型导入

关键数据结构：
```rust
pub struct ModelRegistry {
    db: DatabaseConnection,
    models: Arc<RwLock<HashMap<String, RuntimeModelConfig>>>,
    alias_index: Arc<RwLock<HashMap<String, String>>>,
    availability: Arc<RwLock<HashMap<String, bool>>>,
    fallback_history: Arc<RwLock<Vec<FallbackEvent>>>,
}
```

### 3. 模型管理处理器 - `src/handler/models.rs`

API 端点：
- `GET /api/v1/admin/models` - 列出所有模型
- `POST /api/v1/admin/models` - 创建模型
- `GET /api/v1/admin/models/:id` - 获取模型详情
- `PUT /api/v1/admin/models/:id` - 更新模型
- `DELETE /api/v1/admin/models/:id` - 删除模型
- `POST /api/v1/admin/models/reload` - 热加载
- `POST /api/v1/admin/models/import-defaults` - 导入默认模型
- `GET /api/v1/admin/models/:name/route` - 获取模型路由信息

### 4. 数据库迁移 - `migration/src/m20240328_000012_create_model_configs.rs`

创建 `model_configs` 表，包含所有模型配置字段。

## 修改文件

### 1. `src/entity/mod.rs`
- 添加 `pub mod model_configs;`

### 2. `src/service/mod.rs`
- 添加 `pub mod model_registry;`
- 导出 `pub use model_registry::ModelRegistry;`

### 3. `src/handler/mod.rs`
- 添加 `pub mod models;`

### 4. `src/service/permission.rs`
- 添加 `ModelRead` 和 `ModelWrite` 权限
- 添加 `Model` 权限分组

### 5. `src/gateway/routes.rs`
- 添加模型管理 API 路由

### 6. `migration/src/lib.rs`
- 注册新的迁移文件

## API 端点

### 公开端点
```
GET /v1/models - 列出所有模型（OpenAI 兼容格式）
```

### 管理端点（需要认证和权限）
```
GET    /api/v1/admin/models           - 列出所有模型
POST   /api/v1/admin/models           - 创建模型
GET    /api/v1/admin/models/:id       - 获取模型详情
PUT    /api/v1/admin/models/:id       - 更新模型
DELETE /api/v1/admin/models/:id       - 删除模型
POST   /api/v1/admin/models/reload    - 热加载模型配置
POST   /api/v1/admin/models/import-defaults - 导入默认模型
GET    /api/v1/admin/models/:name/route - 获取模型路由信息
```

## 使用示例

### 创建模型
```bash
curl -X POST http://localhost:3000/api/v1/admin/models \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "gpt-4o",
    "aliases": ["gpt4o", "gpt-4-o"],
    "provider": "openai",
    "api_name": "gpt-4o",
    "display_name": "GPT-4o",
    "input_price": 5.0,
    "output_price": 15.0,
    "max_tokens": 4096,
    "context_window": 128000,
    "capabilities": {
      "chat": true,
      "code": true,
      "math": true
    },
    "supports_streaming": true,
    "supports_function_calling": true,
    "supports_vision": true
  }'
```

### 热加载模型
```bash
curl -X POST http://localhost:3000/api/v1/admin/models/reload \
  -H "Authorization: Bearer <token>"
```

### 导入默认模型
```bash
curl -X POST http://localhost:3000/api/v1/admin/models/import-defaults \
  -H "Authorization: Bearer <token>"
```

## 默认模型

系统提供基础默认模型配置，可通过 API 导入：

**注意**: 默认模型列表仅供参考，实际模型配置应以数据库为准。建议：
- 使用 `POST /api/v1/admin/models/import-defaults` 导入基础配置
- 根据实际需求自定义模型配置
- 定期通过自动同步功能更新模型列表（计划中）

**推荐做法**:
1. 启动后首次运行导入默认模型
2. 根据实际账户和需求调整模型配置
3. 设置合理的降级模型链
4. 定期检查并更新价格信息

## 权限控制

新增权限：
- `model_read` - 查看模型配置
- `model_write` - 创建/编辑模型配置

权限分组：
- `Model` - 模型管理

角色权限：
- Admin: 拥有所有模型权限
- Manager: 拥有模型读写权限

## 技术特性

1. **运行时热加载**: 通过 API 调用重新加载模型配置，无需重启服务
2. **模型别名**: 支持多个别名映射到同一模型
3. **模型降级**: 当主模型不可用时自动切换到备用模型
4. **价格配置**: 支持动态配置输入/输出价格
5. **能力标记**: 支持标记模型能力（chat, code, math, vision 等）
6. **优先级**: 支持设置模型优先级

## 后续优化建议

1. 添加模型使用统计
2. 添加模型健康检查
3. 支持模型分组管理
4. 添加模型配置版本控制
5. 支持模型配置导入/导出
