# FoxNIO 开发指南

## 开发环境设置

### 前置要求

- Rust 1.76+
- Node.js 20+
- Docker & Docker Compose
- PostgreSQL 16
- Redis 7

### 快速开始

```bash
# 1. 克隆项目
git clone https://github.com/your-org/foxnio.git
cd foxnio

# 2. 设置环境
make env
# 编辑 .env 文件

# 3. 启动开发环境
make dev

# 4. 运行服务
make run
```

---

## 项目结构

```
foxnio/
├── backend/                # Rust 后端
│   ├── src/
│   │   ├── config/        # 配置
│   │   ├── db/            # 数据库
│   │   ├── entity/        # ORM Entity
│   │   ├── gateway/       # 网关核心
│   │   ├── handler/       # HTTP 处理器
│   │   ├── model/         # 数据模型
│   │   ├── service/       # 业务逻辑
│   │   ├── utils/         # 工具函数
│   │   └── main.rs        # 入口
│   ├── migration/         # 数据库迁移
│   └── tests/             # 测试
├── frontend/              # SvelteKit 前端
├── docs/                  # 文档
└── docker-compose.yml     # 容器编排
```

---

## 核心模块

### 1. 网关核心 (gateway/)

**handler.rs** - 请求处理
```rust
pub struct GatewayHandler {
    // 转发请求到上游 Provider
    pub async fn forward_request(...) -> Result<Response>
}
```

**failover.rs** - 故障转移
```rust
pub struct FailoverManager {
    // 自动重试 + 指数退避
    pub async fn execute_with_retry(...) -> Result<Response>
}
```

**stream.rs** - 流式响应
```rust
pub struct StreamingBody {
    // SSE 流式处理
}
```

### 2. 业务服务 (service/)

**scheduler.rs** - 智能调度
```rust
pub enum SchedulingStrategy {
    RoundRobin,
    LeastConnections,
    PriorityFirst,
    Random,
    WeightedRoundRobin,
}
```

**rate_limit.rs** - 速率限制
```rust
pub struct RedisRateLimiter {
    pub async fn check_rate_limit(&self, key: &str) -> Result<RateLimitResult>
}
```

---

## 数据库模型

### users 表

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    balance BIGINT DEFAULT 0,
    role VARCHAR(50) DEFAULT 'user',
    status VARCHAR(50) DEFAULT 'active',
    created_at TIMESTAMP DEFAULT NOW()
);
```

### accounts 表

```sql
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    provider VARCHAR(50) NOT NULL,
    api_key TEXT NOT NULL,
    status VARCHAR(50) DEFAULT 'active',
    priority INT DEFAULT 1,
    weight INT DEFAULT 1,
    created_at TIMESTAMP DEFAULT NOW()
);
```

### api_keys 表

```sql
CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    key_hash VARCHAR(255) NOT NULL,
    key_masked VARCHAR(255),
    name VARCHAR(255),
    status VARCHAR(50) DEFAULT 'active',
    created_at TIMESTAMP DEFAULT NOW()
);
```

### usages 表

```sql
CREATE TABLE usages (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    account_id UUID REFERENCES accounts(id),
    model VARCHAR(100) NOT NULL,
    input_tokens INT NOT NULL,
    output_tokens INT NOT NULL,
    cost BIGINT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);
```

---

## 测试

### 运行测试

```bash
# 单元测试
make test

# 集成测试
make test-integration

# 端到端测试
make test-e2e

# 负载测试
make test-load
```

### 测试覆盖率

```bash
cargo tarpaulin --out Html
```

---

## 调试

### 日志级别

```bash
RUST_LOG=debug cargo run
```

### 数据库查询日志

```bash
RUST_LOG=sqlx=debug cargo run
```

---

## 性能优化

### 1. 连接池

```rust
let config = DatabaseConfig {
    max_connections: 20,
    min_connections: 5,
    ..Default::default()
};
```

### 2. 缓存

```rust
// Redis 缓存
let cached = redis.get("cache:key").await?;
```

### 3. 异步处理

```rust
// 并发请求
let results = futures::future::join_all(tasks).await;
```

---

## 部署

### Docker 部署

```bash
make docker-build
make docker-up
```

### 手动部署

```bash
make build
./deploy.sh start
```

---

## 常见问题

### Q: 如何添加新的 Provider?

A: 在 `gateway/handler.rs` 中添加新的 Provider 处理逻辑。

### Q: 如何自定义调度策略?

A: 在 `service/scheduler.rs` 中实现新的策略。

### Q: 如何调整速率限制?

A: 修改 `service/rate_limit.rs` 中的配置。

---

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing`)
5. 创建 Pull Request

---

## 许可证

MIT License
