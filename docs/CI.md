# CI/CD 配置文档

## 概述

FoxNIO 项目使用 GitHub Actions 进行持续集成和持续部署。CI 配置包含了代码检查、测试、覆盖率报告、安全审计等完整流程。

## 工作流程

### 1. 代码检查（Lint）

**并行运行：**
- `lint-backend`: Rust 代码格式化和 clippy 检查
- `lint-frontend`: 前端代码 linting 和类型检查

**特点：**
- 使用缓存加速构建
- 严格的 clippy 规则（-D warnings）
- 自动格式化检查

### 2. 测试与覆盖率

**并行运行：**
- `test-backend`: 后端测试 + 覆盖率报告
- `test-frontend`: 前端测试 + 覆盖率报告

**覆盖率工具：**
- 后端：`cargo-llvm-cov`
- 前端：`vitest --coverage`

**覆盖率报告：**
- 自动上传到 Codecov
- 保存为 GitHub Artifacts
- 支持覆盖率徽章

**服务容器：**
- PostgreSQL 16（测试数据库）
- Redis 7（缓存测试）

### 3. 构建（Build）

**串行执行（在测试通过后）：**
- 后端：Rust release 构建
- 前端：SvelteKit 生产构建

**产物：**
- `foxnio-backend`: 可执行文件
- `foxnio-frontend`: 构建后的静态文件

### 4. 安全审计（Audit）

**并行运行：**
- Rust: `cargo audit`
- Node.js: `npm audit`

### 5. 代码质量（Quality）

**串行执行（在测试通过后）：**
- Clippy pedantic 检查
- 文档生成检查

## 优化措施

### 1. 缓存策略

**Rust 缓存：**
- `~/.cargo/registry`: Cargo 注册表
- `~/.cargo/git`: Git 依赖
- `backend/target`: 构建产物

**Node.js 缓存：**
- `npm cache`: 依赖缓存
- 使用 `package-lock.json` 作为缓存键

### 2. 并行执行

以下任务并行运行以减少总时间：
- `lint-backend` 和 `lint-frontend`
- `test-backend` 和 `test-frontend`
- `audit` 任务

### 3. 增量构建

启用 `CARGO_INCREMENTAL=1` 以加速后续构建。

## 覆盖率目标

- **最低覆盖率**: 85%
- **目标覆盖率**: 90%+

## 徽章

README.md 中包含以下徽章：
- CI/CD 状态
- 后端覆盖率
- 前端覆盖率
- 安全审计状态

**注意**: 需要将徽章 URL 中的 `your-org` 替换为实际的 GitHub 组织或用户名。

## 本地测试

### 后端测试

```bash
cd backend

# 运行测试
cargo test

# 运行覆盖率
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# 查看覆盖率报告
cargo llvm-cov --open
```

### 前端测试

```bash
cd frontend

# 运行测试
npm test

# 运行覆盖率
npm run test:coverage

# 查看覆盖率报告
npm run test:coverage -- --reporter=html
```

## 故障排查

### clippy 警告

如果 clippy 检查失败，运行：
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 格式化问题

如果格式检查失败，运行：
```bash
cargo fmt
```

### 测试失败

查看详细日志：
```bash
cargo test -- --nocapture
```

## Codecov 配置

创建 `codecov.yml` 文件以配置覆盖率阈值：

```yaml
coverage:
  status:
    project:
      default:
        target: 85%
        threshold: 1%
    patch:
      default:
        target: 85%
        threshold: 1%

flags:
  backend:
    paths:
      - backend/
  frontend:
    paths:
      - frontend/

comment:
  layout: "reach,diff,flags,files,footer"
  behavior: default
  require_changes: false
```

## 性能指标

**预期构建时间：**
- Lint: ~2-3 分钟
- Test: ~5-7 分钟
- Build: ~3-5 分钟
- **总计**: < 10 分钟

**影响因素：**
- 缓存命中率
- 依赖更新频率
- 测试数量和复杂度

## 最佳实践

1. **提交前本地测试**: 在提交前运行本地测试
2. **保持覆盖率**: 确保新代码有测试
3. **修复警告**: 及时修复 clippy 和 lint 警告
4. **安全更新**: 定期更新依赖以修复安全问题

## 环境变量

测试需要以下环境变量：
- `DATABASE_URL`: PostgreSQL 连接字符串
- `REDIS_URL`: Redis 连接字符串
- `CODECOV_TOKEN`: Codecov 上传令牌（存储在 GitHub Secrets）

## 支持的分支

- `main`: 生产分支，触发完整 CI
- `develop`: 开发分支，触发完整 CI
- Pull Request: 触发完整 CI
