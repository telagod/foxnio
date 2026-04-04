#!/bin/bash
# FoxNIO 开发环境启动脚本

set -e

echo "🦊 FoxNIO Development Setup"
echo "============================"

# 检查 Rust
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
fi

# 检查 Node.js
if ! command -v node &> /dev/null; then
    echo "Please install Node.js 20+ first"
    exit 1
fi

# 检查 Docker
if ! command -v docker &> /dev/null; then
    echo "Please install Docker first"
    exit 1
fi

# 启动依赖服务
echo "Starting dependencies..."
docker compose up -d postgres redis

# 等待服务就绪
echo "Waiting for services..."
sleep 5

# 设置环境变量
if [ ! -f .env ]; then
    echo "Creating .env file..."
    cp .env.example .env
fi

# 安装前端依赖
echo "Installing frontend dependencies..."
cd frontend
npm install
cd ..

# 运行数据库迁移
echo "Running migrations..."
cargo run --manifest-path backend/migration/Cargo.toml -- up || true

echo ""
echo "✅ Development environment ready!"
echo ""
echo "To start backend:"
echo "  cd backend && cargo run"
echo ""
echo "To start frontend:"
echo "  cd frontend && npm run dev"
echo ""
