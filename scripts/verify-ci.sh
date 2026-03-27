#!/bin/bash
# CI 配置验证脚本

set -e

echo "🔍 验证 CI 配置..."
echo ""

# 检查必要文件
echo "📋 检查必要文件..."
files=(
    ".github/workflows/ci.yml"
    "codecov.yml"
    "docs/CI.md"
    "backend/Cargo.toml"
    "frontend/package.json"
    "frontend/package-lock.json"
    "frontend/vite.config.ts"
    "Makefile"
    "README.md"
)

for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo "  ✅ $file"
    else
        echo "  ❌ $file (缺失)"
        exit 1
    fi
done

echo ""
echo "🔍 检查 CI 配置语法..."

# 检查 YAML 语法（如果安装了 yamllint）
if command -v yamllint &> /dev/null; then
    yamllint .github/workflows/ci.yml
    echo "  ✅ YAML 语法正确"
else
    echo "  ⚠️  yamllint 未安装，跳过 YAML 检查"
fi

echo ""
echo "🔍 检查 CI 工作流程..."

# 检查 CI 配置中的关键步骤
ci_file=".github/workflows/ci.yml"

# 检查覆盖率步骤
if grep -q "cargo-llvm-cov" "$ci_file"; then
    echo "  ✅ 后端覆盖率配置存在"
else
    echo "  ❌ 后端覆盖率配置缺失"
    exit 1
fi

if grep -q "test:coverage" "$ci_file"; then
    echo "  ✅ 前端覆盖率配置存在"
else
    echo "  ❌ 前端覆盖率配置缺失"
    exit 1
fi

# 检查缓存配置
if grep -q "actions/cache@v4" "$ci_file"; then
    echo "  ✅ 缓存配置存在"
else
    echo "  ❌ 缓存配置缺失"
    exit 1
fi

# 检查并行任务
if grep -q "lint-backend" "$ci_file" && grep -q "lint-frontend" "$ci_file"; then
    echo "  ✅ 并行 Lint 任务配置正确"
else
    echo "  ❌ 并行 Lint 任务配置缺失"
    exit 1
fi

# 检查服务容器
if grep -q "postgres:16" "$ci_file" && grep -q "redis:7" "$ci_file"; then
    echo "  ✅ 服务容器配置存在"
else
    echo "  ❌ 服务容器配置缺失"
    exit 1
fi

echo ""
echo "🔍 检查 README 徽章..."

if grep -q "CI/CD" README.md && grep -q "Coverage" README.md; then
    echo "  ✅ CI 徽章存在"
else
    echo "  ❌ CI 徽章缺失"
    exit 1
fi

echo ""
echo "🔍 检查 Makefile 命令..."

if grep -q "test-coverage" Makefile; then
    echo "  ✅ test-coverage 命令存在"
else
    echo "  ❌ test-coverage 命令缺失"
    exit 1
fi

echo ""
echo "✅ 所有检查通过！"
echo ""
echo "📊 CI 配置摘要:"
echo "  - 工作流程: 6 个并行任务"
echo "  - 覆盖率工具: cargo-llvm-cov (后端), vitest (前端)"
echo "  - 缓存: Rust + Node.js"
echo "  - 服务容器: PostgreSQL 16, Redis 7"
echo "  - 预期时间: < 10 分钟"
echo ""
echo "📝 后续步骤:"
echo "  1. 提交并推送更改"
echo "  2. 在 GitHub Actions 中查看运行结果"
echo "  3. 替换 README.md 中的徽章 URL (your-org -> 实际组织名)"
echo "  4. 设置 Codecov (可选)"
