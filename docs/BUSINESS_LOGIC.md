# FoxNIO 业务逻辑

本文档只回答四件事：系统现在在做什么、哪些链路已闭环、哪些还没闭环、接下来该先补什么。

## 现状

FoxNIO 当前同时承担四类职责：

1. 模型网关  
   OpenAI 兼容接口、Anthropic `messages`、Gemini Native、Realtime/WebSocket、Sora / prompt enhance。
2. 用户自助面  
   用户查看个人信息、API Key、usage、订阅与额度。
3. 管理控制台  
   管理员查看 dashboard、用户、账号、API Key、usage 分布与控制面对象。
4. 控制面  
   验证码、兑换码、配额、Webhook、告警、备份、调度、审计与部分运维能力。

结论：它不是单纯 proxy，而是一个“gateway + self-service + admin + control plane”混合系统。

## 已完成

### 用户与管理查询链

- 用户 dashboard 已读真实用户信息、API Key 与 usage 汇总。
- `/usage` 页面已读真实日维度 usage 报表。
- 管理后台已通过 `/api/v1/admin/dashboard/*` 读取真实聚合统计。

### 网关鉴权与计费链

- `/v1/*` 统一走 FoxNIO API Key 鉴权。
- API Key 级 RPM / concurrency 已用 Redis 落地。
- 用户级 quota 主语义已回到 `subscriptions.user_id`。
- usage 成功/失败写库与 quota 消耗已不再是空壳。

### Realtime / WebSocket

- 握手前做 API Key 鉴权与 Redis 风控。
- 已建立真实 OpenAI Realtime 上游 WebSocket 转发。
- request/response 级关键事件会写 audit。
- `response.completed` / `response.done` 等成功事件会写 usage 并消耗 quota。
- `upstream_prepare_failed`、`upstream_connect_failed`、`upstream_stream_error`、`response.failed` 会写失败 usage。

### Gemini Native

- `generateContent`、`streamGenerateContent`、`countTokens`、`embedContent` 已接真实上游。
- OAuth/API Key 凭据解析与持久化已接上。
- transport / 429 / 5xx 已有 retry。
- 成功与失败 usage、quota 消耗、错误分类都已落地。

### Sora / prompt enhance

- `/v1/images/generations`
- `/v1/videos/generations`
- `/v1/videos/generations/:id`
- `/v1/prompts/enhance`

以上入口都已进入真实 OpenAI HTTP 转发，并补 usage / quota / failed usage。

### 验证码 / redeem / 订阅

- 验证码发送会写 Redis、做发送频控并调用 SMTP。
- 注册与改邮箱已消费验证码。
- redeem code 已具备事务写库、幂等键与审计 ledger 的主闭环。

## 未完成

### 真实 smoke 还缺输入条件

当前真实 OpenAI / Gemini smoke 仍未完成，原因不是代码主链未写，而是没有真实密钥与账号输入：

- `OPENAI_API_KEY`
- `GEMINI_API_KEY` / `GOOGLE_API_KEY`
- 可用的 provider account / OAuth token

### 控制面还不够完整

- backup 仍缺 `import`、对象存储、恢复演练与正式审计。
- redeem 仍缺活动规则、额度来源解释与更清晰的产品说明。
- 账务 / quota / subscription / redeem 的语义对外解释仍未统一。

### 运维与异步仍偏骨架

- `ops_*`
- `batch` / `batch_operations`
- 等待队列
- 调度恢复
- 快照恢复
- 模型级 / 账号级限流与排队

这些模块不决定页面能否打开，但决定系统能否长期稳定运营。

### 发布链刚完成第一轮收口

- backend 已支持 `config.yaml + .env + runtime env`。
- frontend Docker 已切回 `adapter-node` 正确口径。
- compose 已分 core / `ui` / `edge` 三层。
- 但还缺“干净环境从 build 到 up 到 smoke”的完整上线回归。

## 下一步

1. 先完成 backend image 与 core compose `/health` 回归。
2. 再把发布链写成可直接接真实密钥即跑的状态。
3. 然后补 backup / redeem / 账务解释这三块最影响产品完整度的断点。
4. 最后等真实密钥到位，再做 OpenAI / Gemini 真 smoke 与 failure integration。
