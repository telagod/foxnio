#!/bin/bash

# FoxNIO 性能优化 - 完整部署脚本
# 使用方法: ./optimize_and_deploy.sh

set -e

echo "======================================"
echo "  FoxNIO 性能优化部署脚本"
echo "======================================"
echo ""

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 检查 Rust 版本
echo -e "${YELLOW}步骤 1: 检查 Rust 版本${NC}"
RUST_VERSION=$(rustc --version | grep -oP '\d+\.\d+\.\d+')
echo "当前 Rust 版本: $RUST_VERSION"

if [[ "$RUST_VERSION" < "1.82.0" ]]; then
    echo -e "${RED}错误: Rust 版本过低，需要 >= 1.82.0${NC}"
    echo "请运行以下命令升级 Rust："
    echo "  rustup update stable"
    echo ""
    echo "如果没有 rustup，请先安装："
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo -e "${GREEN}✓ Rust 版本满足要求${NC}"
echo ""

# 检查数据库服务
echo -e "${YELLOW}步骤 2: 检查数据库服务${NC}"
if command -v docker &> /dev/null; then
    echo "检查 Docker 容器状态..."
    if docker ps | grep -q postgres; then
        echo -e "${GREEN}✓ PostgreSQL 容器运行中${NC}"
    else
        echo "启动 PostgreSQL 容器..."
        docker compose up -d postgres
    fi

    if docker ps | grep -q redis; then
        echo -e "${GREEN}✓ Redis 容器运行中${NC}"
    else
        echo "启动 Redis 容器..."
        docker compose up -d redis
    fi
else
    echo -e "${YELLOW}未检测到 Docker，假设数据库服务已手动启动${NC}"
fi
echo ""

# 运行数据库迁移
echo -e "${YELLOW}步骤 3: 运行数据库迁移${NC}"
cd backend

if [ -f "../config.yaml" ]; then
    echo "配置文件存在: ✓"
else
    echo -e "${RED}错误: 配置文件不存在${NC}"
    exit 1
fi

echo "运行迁移..."
cargo run --manifest-path migration/Cargo.toml -- up

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ 数据库迁移成功${NC}"
else
    echo -e "${RED}错误: 数据库迁移失败${NC}"
    exit 1
fi
echo ""

# 构建项目
echo -e "${YELLOW}步骤 4: 构建 Release 版本${NC}"
echo "开始编译（这可能需要几分钟）..."
cargo build --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ 编译成功${NC}"
else
    echo -e "${RED}错误: 编译失败${NC}"
    exit 1
fi
echo ""

echo -e "${GREEN}======================================"
echo "  优化部署完成！"
echo "======================================${NC}"
echo ""
echo "启动服务："
echo "  ./target/release/foxnio"
echo ""
