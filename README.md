# FoxNIO - AI API Gateway

<div align="center">

🦊 **FoxNIO** - 高性能 AI API 网关

优雅 · 专业 · 克制

[![CI/CD](https://github.com/your-org/foxnio/workflows/FoxNIO%20CI/CD/badge.svg)](https://github.com/your-org/foxnio/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](#english) | [中文文档](#中文文档)

</div>

---

## English

### Overview

FoxNIO is a high-performance AI API gateway built with Rust and SvelteKit. It provides unified API access to multiple AI providers with intelligent routing, automatic failover, usage-based billing, and real-time streaming support.

### Key Features

- **🔄 Multi-Model Proxy** - OpenAI, Claude, Gemini, Antigravity, and more
- **🧠 Smart Account Scheduling** - Intelligent routing with automatic fallback and load balancing
- **💳 Usage Billing System** - Real-time usage tracking, quota management, and flexible billing
- **⚡ WebSocket Streaming** - Native streaming support for real-time AI responses
- **🎛️ Web Admin Panel** - Modern SvelteKit-based dashboard for management
- **🔐 Multi-OAuth Support** - GitHub, Google, LinuxDo, Antigravity authentication

### Quick Start

```bash
# Clone and run with Docker
git clone https://github.com/your-org/foxnio.git
cd foxnio
docker-compose up -d

# API: http://localhost:8080
# Dashboard: http://localhost:5173
```

#### Manual Setup

```bash
make install        # Install dependencies
cp .env.example .env && vim .env  # Configure
make db-up migrate  # Setup database
make run            # Start server
```

### Configuration

Key environment variables (`.env`):

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/foxnio
REDIS_URL=redis://localhost:6379

# Security
JWT_SECRET=your-secret-key
ENCRYPTION_KEY=your-encryption-key

# AI Providers
OPENAI_API_KEY=sk-xxx
ANTHROPIC_API_KEY=sk-ant-xxx
GOOGLE_API_KEY=xxx
ANTIGRAVITY_API_KEY=xxx

# OAuth Providers
GITHUB_CLIENT_ID=xxx
GITHUB_CLIENT_SECRET=xxx
GOOGLE_CLIENT_ID=xxx
GOOGLE_CLIENT_SECRET=xxx
```

### API Usage

```bash
# Chat Completions (OpenAI-compatible)
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer foxnio-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

### Supported Providers

| Provider | Models |
|----------|--------|
| OpenAI | gpt-4-turbo, gpt-4o, gpt-4, gpt-3.5-turbo |
| Anthropic | claude-3-opus, claude-3.5-sonnet, claude-3-haiku |
| Google | gemini-1.5-pro, gemini-1.5-flash |
| Antigravity | Custom model support |
| DeepSeek | deepseek-chat, deepseek-coder |

### Documentation

- 📖 [Project Overview](docs/PROJECT_OVERVIEW.md) - Architecture and tech stack
- 🔌 [API Reference](docs/API_REFERENCE.md) - Complete API documentation
- 🗃️ [Database Schema](docs/DATABASE_SCHEMA.md) - Data model
- 🚀 [Deployment Guide](docs/DEPLOYMENT.md) - Production deployment
- 🛠️ [Development Guide](docs/DEVELOPMENT.md) - Contributing guide

---

## 中文文档

### 项目简介

FoxNIO 是一个使用 Rust 和 SvelteKit 构建的高性能 AI API 网关，提供多模型统一接入、智能调度、计费系统和实时流式传输。

### 核心特性

- **🔄 多模型代理** - 支持 OpenAI、Claude、Gemini、Antigravity 等
- **🧠 智能账户调度** - 自动路由、故障转移、负载均衡
- **💳 使用量计费系统** - 实时用量追踪、配额管理、灵活计费
- **⚡ WebSocket 流式传输** - 原生支持实时 AI 响应流
- **🎛️ Web 管理后台** - 基于 SvelteKit 的现代化管理面板
- **🔐 多 OAuth 支持** - GitHub、Google、LinuxDo、Antigravity 认证

### 快速开始

```bash
# Docker 部署
git clone https://github.com/your-org/foxnio.git
cd foxnio
docker-compose up -d

# API 地址：http://localhost:8080
# 管理后台：http://localhost:5173
```

#### 手动部署

```bash
make install        # 安装依赖
cp .env.example .env && vim .env  # 配置环境变量
make db-up migrate  # 初始化数据库
make run            # 启动服务
```

### 配置说明

主要环境变量（`.env`）：

```bash
# 数据库
DATABASE_URL=postgresql://user:pass@localhost:5432/foxnio
REDIS_URL=redis://localhost:6379

# 安全
JWT_SECRET=your-secret-key
ENCRYPTION_KEY=your-encryption-key

# AI 服务商
OPENAI_API_KEY=sk-xxx
ANTHROPIC_API_KEY=sk-ant-xxx
GOOGLE_API_KEY=xxx
ANTIGRAVITY_API_KEY=xxx

# OAuth 认证
GITHUB_CLIENT_ID=xxx
GITHUB_CLIENT_SECRET=xxx
GOOGLE_CLIENT_ID=xxx
GOOGLE_CLIENT_SECRET=xxx
```

### API 调用

```bash
# 对话补全（OpenAI 兼容）
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer foxnio-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "你好"}],
    "stream": true
  }'
```

### 支持的服务商

| 服务商 | 模型 |
|--------|------|
| OpenAI | gpt-4-turbo, gpt-4o, gpt-4, gpt-3.5-turbo |
| Anthropic | claude-3-opus, claude-3.5-sonnet, claude-3-haiku |
| Google | gemini-1.5-pro, gemini-1.5-flash |
| Antigravity | 自定义模型支持 |
| DeepSeek | deepseek-chat, deepseek-coder |

### 文档链接

- 📖 [项目概述](docs/PROJECT_OVERVIEW.md) - 架构与技术栈
- 🔌 [API 参考](docs/API_REFERENCE.md) - 完整 API 文档
- 🗃️ [数据库架构](docs/DATABASE_SCHEMA.md) - 数据模型
- 🚀 [部署指南](docs/DEPLOYMENT.md) - 生产环境部署
- 🛠️ [开发指南](docs/DEVELOPMENT.md) - 贡献指南

### 许可证

本项目采用 [MIT 许可证](LICENSE)

---

<div align="center">

Made with ❤️ by FoxNIO Team

</div>
