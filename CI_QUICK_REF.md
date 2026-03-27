# CI 快速参考

## 🚀 快速命令

```bash
# 完整测试
make test

# 带覆盖率测试
make test-coverage

# 代码检查
make lint

# 格式化代码
make fmt

# 本地开发
make dev
```

## 📊 CI 状态

- ✅ **Lint**: 代码格式和静态检查
- ✅ **Test**: 单元测试 + 集成测试
- ✅ **Coverage**: 覆盖率报告（目标 85%+）
- ✅ **Build**: 生产构建
- ✅ **Audit**: 安全审计
- ✅ **Quality**: 代码质量检查

## 🔧 本地修复

### 格式检查失败
```bash
cd backend && cargo fmt
cd frontend && npm run format
```

### Clippy 警告
```bash
cd backend && cargo clippy --fix
```

### 测试失败
```bash
# 详细输出
cargo test -- --nocapture

# 运行特定测试
cargo test test_name
```

## 📝 配置文件

- `.github/workflows/ci.yml` - CI 工作流
- `codecov.yml` - 覆盖率配置
- `docs/CI.md` - 详细文档
- `CI_FIX_REPORT.md` - 修复报告

## 🎯 覆盖率目标

- **最低**: 85%
- **推荐**: 90%
- **优秀**: 95%

## ⏱️ 预期时间

- Lint: 2-3 分钟
- Test: 5-7 分钟
- Build: 3-5 分钟
- **总计**: < 10 分钟

## 🔗 相关链接

- [GitHub Actions](https://github.com/your-org/foxnio/actions)
- [Codecov](https://codecov.io/gh/your-org/foxnio)
- [CI 文档](docs/CI.md)
