# FoxNIO 支持的模型和服务商

**更新时间**: 2026-04-04

## ⚠️ 重要说明

FoxNIO 采用**完全动态的模型配置方案**，所有模型信息存储在数据库中。本文档列出的仅为默认配置，实际可用模型应以数据库配置为准。

**查看实际模型列表**:
```bash
# 查询当前配置的模型
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer foxnio-your-key"

# 管理员查看所有模型配置
curl http://localhost:8080/api/v1/admin/models \
  -H "Authorization: Bearer <admin-token>"
```

## 支持的服务商

| 服务商 | 状态 | 特性 | API 端点 |
|--------|------|------|---------|
| OpenAI | ✅ 完全支持 | GPT 系列，支持 Vision、Function Calling | `/v1/chat/completions` |
| Anthropic | ✅ 完全支持 | Claude 系列，支持长上下文 | `/v1/messages` |
| Google (Gemini) | ✅ 完全支持 | Gemini 系列，原生 API | `/v1beta/models` |
| DeepSeek | ✅ 完全支持 | V3、Coder 系列 | `/v1/chat/completions` |
| Mistral | ✅ 完全支持 | Mistral 全系列 | `/v1/chat/completions` |
| Cohere | ✅ 完全支持 | Command 系列 | `/v1/chat/completions` |

## 默认模型配置

以下为系统预置的默认模型配置（可通过 API 导入）：

### OpenAI

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 输入价格 ($/1M tokens) | 输出价格 ($/1M tokens) |
|---------|------|----------|---------|---------------------|---------------------|
| gpt-4-turbo | gpt4t, gpt-4-turbo-preview | 128K | 4K | $10.00 | $30.00 |
| gpt-4o | gpt4o | 128K | 4K | $5.00 | $15.00 |
| gpt-4o-mini | gpt4o-mini | 128K | 4K | $0.15 | $0.60 |
| gpt-3.5-turbo | gpt35, gpt-3.5 | 16K | 4K | $0.50 | $1.50 |

**特性支持**:
- ✅ 流式输出 (SSE)
- ✅ 函数调用 (Function Calling)
- ✅ 视觉输入 (Vision)
- ✅ 长上下文
- ✅ 多语言

### Anthropic

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 输入价格 ($/1M tokens) | 输出价格 ($/1M tokens) |
|---------|------|----------|---------|---------------------|---------------------|
| claude-3-opus | claude3-opus | 200K | 4K | $15.00 | $75.00 |
| claude-3.5-sonnet | claude35-sonnet | 200K | 8K | $3.00 | $15.00 |
| claude-3-haiku | claude3-haiku | 200K | 4K | $0.25 | $1.25 |

**特性支持**:
- ✅ 流式输出
- ✅ 函数调用
- ✅ 视觉输入
- ✅ 超长上下文 (200K)
- ✅ 多语言

### DeepSeek

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 输入价格 ($/1M tokens) | 输出价格 ($/1M tokens) |
|---------|------|----------|---------|---------------------|---------------------|
| deepseek-v3 | deepseek-chat | 64K | 4K | $0.27 | $1.10 |
| deepseek-coder | - | 64K | 4K | $0.14 | $0.28 |

**特性支持**:
- ✅ 流式输出
- ✅ 代码生成优化
- ✅ 高性价比

### Google (Gemini)

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 输入价格 ($/1M tokens) | 输出价格 ($/1M tokens) |
|---------|------|----------|---------|---------------------|---------------------|
| gemini-1.5-pro | - | 1M | 8K | $3.50 | $10.50 |
| gemini-1.5-flash | - | 1M | 8K | $0.075 | $0.30 |

**特性支持**:
- ✅ 流式输出
- ✅ 函数调用
- ✅ 视觉输入
- ✅ 超长上下文 (1M tokens)
- ✅ 原生 API 支持

### Mistral

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 特性 |
|---------|------|----------|---------|------|
| mistral-large | - | 32K | 4K | 旗舰模型 |
| mistral-medium | - | 32K | 4K | 平衡性能 |
| mistral-small | - | 32K | 4K | 高性价比 |

### Cohere

| 模型名称 | 别名 | 上下文窗口 | 最大输出 | 特性 |
|---------|------|----------|---------|------|
| command-r | - | 128K | 4K | 检索增强 |
| command-r-plus | - | 128K | 4K | 增强版 |

## 模型能力标签

每个模型都有以下能力标签：

| 能力标签 | 说明 | 示例模型 |
|---------|------|---------|
| `chat` | 对话能力 | 所有模型 |
| `code` | 代码生成 | GPT-4, Claude, DeepSeek-Coder |
| `math` | 数学推理 | GPT-4, Claude |
| `long_context` | 长上下文 | GPT-4-Turbo (128K), Claude (200K) |
| `multilingual` | 多语言支持 | 所有模型 |
| `tools` | 函数调用 | GPT-4, Claude, Gemini |
| `vision` | 视觉输入 | GPT-4o, Claude-3.5, Gemini |

## 模型降级链

默认配置包含智能降级链，当主模型不可用时自动切换：

**示例**:
```
gpt-4-turbo → gpt-4 → gpt-4o
gpt-4o → gpt-4-turbo
gpt-4o-mini → gpt-3.5-turbo
claude-3-opus → claude-3.5-sonnet → claude-3-haiku
```

## 配置管理

### 导入默认模型

```bash
curl -X POST http://localhost:8080/api/v1/admin/models/import-defaults \
  -H "Authorization: Bearer <admin-token>"
```

### 添加自定义模型

```bash
curl -X POST http://localhost:8080/api/v1/admin/models \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-custom-model",
    "provider": "openai",
    "api_name": "gpt-4o-realtime-preview",
    "display_name": "My Custom Model",
    "input_price": 5.0,
    "output_price": 20.0,
    "max_tokens": 4096,
    "context_window": 128000,
    "supports_streaming": true,
    "supports_function_calling": true,
    "supports_vision": false,
    "enabled": true
  }'
```

### 更新模型配置

```bash
curl -X PUT http://localhost:8080/api/v1/admin/models/1 \
  -H "Authorization: Bearer <admin-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "input_price": 2.5,
    "output_price": 10.0,
    "enabled": true
  }'
```

### 热加载模型配置

```bash
curl -X POST http://localhost:8080/api/v1/admin/models/reload \
  -H "Authorization: Bearer <admin-token>"
```

## 价格更新

模型价格会定期更新，建议：

1. **手动更新**: 定期检查各服务商官方价格页面
2. **自动同步**: 使用模型同步服务（计划中）
3. **价格监控**: 设置价格变更告警

**官方价格页面**:
- [OpenAI Pricing](https://openai.com/pricing)
- [Anthropic Pricing](https://www.anthropic.com/pricing)
- [Google AI Pricing](https://ai.google.dev/pricing)
- [DeepSeek Pricing](https://platform.deepseek.com/api-docs/pricing/)
- [Mistral Pricing](https://mistral.ai/technology/)
- [Cohere Pricing](https://cohere.com/pricing)

## 相关文档

- [动态模型实现](../backend/docs/DYNAMIC_MODEL_IMPLEMENTATION.md)
- [API 参考](API_REFERENCE.md)
- [功能对齐计划](FEATURE_ALIGNMENT.md)
