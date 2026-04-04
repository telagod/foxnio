# FoxNIO 业务逻辑

本文档只回答四件事：系统现在在做什么、哪些业务链已闭环、哪些业务链还没闭环、下一步该先补什么。

## 现状

FoxNIO 当前的业务不是单点代理，而是四层叠加：

- 上游网关层：统一对接 OpenAI、Anthropic、Gemini、Realtime、Sora
- 用户自助层：用户注册、登录、查看余额、管理 API Key、查看 usage
- 管理控制层：管理员看 dashboard、用户、账号、API Key 与部分控制面数据
- 控制面层：验证码、兑换码、quota、审计、告警、备份、调度

从业务角色看，系统里有五类核心对象：

- anonymous visitor：注册、登录、收验证码
- user：消费 API、查看自身 usage、管理自己的 API Key
- admin：查看全局状态、管理用户与账号、做控制台操作
- upstream account：真实 provider 凭据与转发目标
- platform ledger：usage、quota、balance、redeem、audit 等账务与审计记录

当前业务形态的核心判断：

- FoxNIO 已具备“用户消费 + 平台计费 + 管理查看 + 多 provider 转发”的主结构。
- 但它还没有完全达到“稳定运营平台”的成熟度，因为备份恢复、运维异步、真实 smoke、账务解释仍未完全闭环。

## 已完成

### 认证与用户自助主链

- 用户可以注册、登录、刷新 token、重置密码、使用 TOTP。
- 验证码发送已写 Redis、做发送频控并调用 SMTP。
- 改邮箱已要求验证码消费，不再只是前端提交一个新邮箱。

这意味着“用户身份建立与基本自助操作”已经不是空壳。

### API Key 与调用入口主链

- 用户可创建、查看、删除自己的 API Key。
- 网关入口按 FoxNIO API Key 做鉴权。
- API Key 级 RPM / concurrency 已落 Redis。

这意味着“发 key -> 带 key 调用 -> 进入网关”的最小平台主链已成立。

### usage、quota 与用户可见报表主链

- usage 已落到真实 `usages` 表。
- 用户 dashboard 已读真实用户信息、API Key 与 usage 汇总。
- `/usage` 页面已读真实日维度 usage 报表。
- quota 消耗与 usage 回写已进入主调用链，而不是纯前端展示。

这意味着“用户发请求后，平台能记账并展示给用户看”已经闭环。

### 管理侧 dashboard 主链

- 管理端已通过 `/api/v1/admin/dashboard/*` 读取真实聚合统计。
- 管理员可看到用户、账号、API Key、usage 趋势、模型分布、平台分布等核心指标。
- dashboard 查询逻辑已从 handler 下沉到独立 query service。

这意味着“管理员看全局状态”的基础控制台已成立。

### 多 provider 网关主链

#### Realtime / WebSocket

- 握手前做 API Key 鉴权与 Redis 风控。
- 已建立真实 OpenAI Realtime 上游 WebSocket 转发。
- request / response 级关键事件会写 audit。
- 成功与失败事件会写 usage，并进入 quota 口径。

#### Gemini Native

- `generateContent`
- `streamGenerateContent`
- `countTokens`
- `embedContent`

以上链路已接真实上游。

- OAuth / API Key 凭据解析已接上。
- transport / 429 / 5xx 已有 retry。
- 成功 / 失败 usage、quota 消耗、错误分类都已落地。

#### Sora / prompt enhance

- `/v1/images/generations`
- `/v1/videos/generations`
- `/v1/videos/generations/:id`
- `/v1/prompts/enhance`

以上入口都已进入真实 OpenAI HTTP 转发，并补 usage / quota / failed usage。

### redeem 与账务审计主链

- redeem code 已具备事务写库。
- 幂等键已进入兑换主链。
- ledger / audit 已记录关键兑换事件。

这意味着“发活动码 -> 用户兑换 -> 平台记账”已经有业务骨架，不再只是表结构。

## 未完成

### 真实 provider 验证还没形成权威结果

虽然代码主链已经接真，但仍没有真实 OpenAI / Gemini 回归报告。原因不是业务逻辑没写，而是缺少：

- 真实 API key
- 真实 provider account
- 真实 OAuth token

所以现在能说“链路存在”，还不能说“真实线上口径已经完全验明”。

### 账务语义还没完全产品化

目前系统里已经同时存在：

- usage
- quota
- balance
- subscription
- redeem

它们在代码里已有主链，但对外产品语义还不够统一。还缺：

- 额度来源解释
- 余额与 quota 的边界说明
- subscription 与 quota 的优先级说明
- redeem 对账说明

### 控制面不够完整

- backup 缺 `import`
- backup 缺对象存储与恢复演练
- redeem 缺活动规则与查询解释
- 告警、Webhook、运维聚合、恢复类模块仍有明显骨架

也就是说，平台已经能“提供能力”，但还没完全达到“稳定运营平台”的控制面成熟度。

### 异步与恢复能力仍偏弱

以下能力仍决定平台是否能长期稳定运行：

- `batch` / `batch_operations`
- 等待队列
- 调度恢复
- 快照恢复
- `ops_*`
- token refresh 与后台任务恢复

这些模块不一定影响首页和 dashboard 是否能打开，但会直接影响长期运行质量。

### 发布链仍缺最终业务闭环

当前部署口径已经收口，但还差最后一层业务确认：

- 干净环境 build
- 起服
- `/health`
- 最小 smoke
- 回滚

这一步做完，业务文档里的“可上线”才算真正成立。

## 下一步

1. 先完成发布闭环，让系统在无真实 provider 密钥时也能稳定起服并通过最小 smoke。
2. 再用真实 OpenAI / Gemini 输入跑真 smoke，把“代码接真”升级成“真实回归通过”。
3. 然后统一 quota / subscription / balance / redeem 的产品语义，把账务解释写实。
4. 最后补 backup、ops、恢复类控制面，把系统从“能跑”推到“能长期运营”。
