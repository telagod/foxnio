#!/bin/bash
# FoxNIO 编译错误快速修复脚本

set -e

echo "=== FoxNIO 编译错误修复脚本 ==="
echo ""

cd /fs1/openclaw-data/workspace/foxnio/backend

# 1. 运行 cargo fix
echo "1. 运行 cargo fix (自动修复简单问题)..."
cargo fix --allow-dirty --lib 2>&1 || true
cargo fix --allow-dirty --tests 2>&1 || true

# 2. 修复密码重置测试中的临时值问题
echo "2. 修复 password_reset_test.rs 中的临时值问题..."
if grep -q '"a".repeat(128).as_str()' src/service/password_reset_test.rs; then
    sed -i 's/"a".repeat(128).as_str()/let long_pwd = "a".repeat(128); long_pwd.as_str()/g' src/service/password_reset_test.rs || true
fi

# 3. 添加缺失的 trait 实现
echo "3. 检查并添加缺失的 trait 实现..."

# 检查是否需要为 ModelRouter 实现 Clone
if grep -q "no method named \`clone\` found for struct \`ModelRouter\`" < <(cargo build 2>&1); then
    echo "  - 为 ModelRouter 添加 Clone 实现"
    # 这个需要手动添加，因为需要确定哪些字段可以 clone
fi

# 检查是否需要为 CompressedResponse 实现 Clone
if grep -q "no method named \`clone\` found for struct \`CompressedResponse\`" < <(cargo build 2>&1); then
    echo "  - 为 CompressedResponse 添加 Clone 实现"
fi

# 4. 检查静态上下文错误
echo "4. 统计静态上下文错误 (E0015)..."
E0015_COUNT=$(cargo build 2>&1 | grep -c "error\[E0015\]" || true)
if [ "$E0015_COUNT" -gt 0 ]; then
    echo "  发现 $E0015_COUNT 个 E0015 错误，需要手动修复"
    echo "  建议: 使用 lazy_static! 或 once_cell::sync::Lazy"
fi

# 5. 检查方法不存在错误
echo "5. 检查方法不存在错误..."
cargo build 2>&1 | grep "no method named" | sort -u || true

# 6. 生成错误报告
echo ""
echo "=== 生成详细错误报告 ==="
cargo build 2>&1 | grep "^error" | sort | uniq -c | sort -rn > /tmp/error_summary.txt || true
echo "错误摘要已保存到 /tmp/error_summary.txt"
cat /tmp/error_summary.txt

echo ""
echo "=== 修复完成 ==="
echo "剩余错误数量:"
cargo build 2>&1 | grep -c "^error" || echo "0"
