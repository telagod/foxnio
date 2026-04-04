# FoxNIO 项目概述

**更新时间**: 2026-04-04
**口径**: 本文档用于说明项目是什么、现在做到哪一步、还缺什么，不写未验证的营销指标与性能承诺

## 现状

FoxNIO 当前不是单一 AI proxy，而是一个由 Rust/Axum backend 与 SvelteKit frontend 组成的混合系统，覆盖四层职责：

- gateway：OpenAI 兼容、Anthropic `messages`、Gemini Native、Realtime/WebSocket、Sora / prompt enhance
- user self-service：注册、登录、个人信息、API Key、dashboard、`/usage`
- admin console：dashboard、用户、账号、API Key 与部分控制面查询
- control plane：验证码、兑换码、quota、审计、告警、备份、调度

当前最准确的项目判断：

- 它已经具备“多 provider 转发 + 用户自助 + 管理控制台 + 基础账务审计”的主结构
- 但还没有达到“所有控制面、恢复能力、真实 smoke、发布回归都完成”的成熟产品状态

## 已完成

### 技术结构

- backend：Rust + Axum + SeaORM + PostgreSQL + Redis
- frontend：SvelteKit + TypeScript + Vite
- 部署：Docker + Docker Compose + Nginx 可选边缘层
- CI：GitHub Actions 已覆盖 lint / check / test / audit / build 等基础流程

### 已接真的业务主链

- 用户 dashboard 已接真实用户、API Key、usage 数据
- 用户 `/usage` 已接真实日维度 usage 聚合
- 管理 dashboard 已接 `/api/v1/admin/dashboard/*`
- API Key 鉴权、RPM / concurrency 风控、usage 回写、quota 口径已进入主链
- Realtime/WebSocket 已接真实上游转发与 usage / audit
- Gemini Native 已接真实上游、retry、typed error、usage / quota
- Sora / prompt enhance 已接真实 OpenAI HTTP 转发与 usage / quota
- 注册、登录、验证码发送、改邮箱验证码消费、密码重置、TOTP 已接主链
- redeem code 已接事务写库、幂等键、ledger 审计

### 工程收口进展

- backend 与 frontend 当前关键静态校验可过
- runtime config、Docker、compose、deploy 脚本已完成第一轮统一
- 文档主线已开始按“现状 / 已完成 / 未完成 / 下一步”重写

## 未完成

### 真实 provider 回归

当前仍缺：

- 真实 OpenAI / Gemini 密钥
- 真实 provider account / OAuth token
- 真 smoke 与 failure 回归记录

所以现在能确认“代码链存在”，但不能宣称“全部 provider 已真实验明”。

### 控制面完整度

当前仍有明显缺口：

- `backup import`
- 备份恢复演练
- redeem 运营规则与对账解释
- quota / subscription / balance / redeem 的统一产品语义
- `ops_*`、`batch*`、等待队列、调度恢复等异步能力

### 发布与运行完整度

当前部署口径已收口，但还缺：

- 干净环境完整起服演练
- 最小上线 smoke
- 最小回滚手册
- frontend lint 范围清理

## 下一步

1. 先补发布闭环，让 `core` / `ui` / `edge` 路径都能形成权威演练记录。
2. 再补真实 OpenAI / Gemini smoke，把“代码接真”升级成“真实回归通过”。
3. 再补 backup、redeem、quota / subscription 解释统一，把系统从“能跑”推到“可运营”。
4. 最后继续清理历史专题文档，收束为少量权威入口文档。

## 相关文档

- [当前状态](./CURRENT_STATUS.md)
- [业务逻辑](./BUSINESS_LOGIC.md)
- [部署说明](./DEPLOYMENT.md)
- [长期 TODO](../TODO.md)
