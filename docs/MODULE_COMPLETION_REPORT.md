# FoxNIO 模块状态报告

**更新时间**: 2026-04-04
**口径**: 本文档不再输出伪精确完成度或“可直接上线”标签，只描述当前模块状态、已闭环能力、明显缺口与下一步

## 现状

FoxNIO 当前模块结构很大，已经超出“几个网关文件”的规模。按职责可以拆成六个层次：

- `gateway/`: 对外请求入口、协议适配、转发、风控、部分审计
- `handler/`: 用户侧、管理侧、认证侧、控制面 HTTP 入口
- `service/`: 业务逻辑、聚合查询、账务、provider 对接、异步与控制面服务
- `entity/` + `migration/`: 数据模型与数据库演进
- `health/` + `metrics/`: 健康检查、Prometheus 指标
- `frontend/`: 用户台与管理台界面

当前最重要的现实判断：

- 模块数量很多，但完成度并不均匀
- 用户侧、管理 dashboard、主要 provider 网关链路已接真
- 控制面、异步恢复、备份、部分 ops 模块仍存在明显骨架

## 已完成

### `gateway/`

已接真或已形成主链的方向：

- OpenAI 兼容入口
- Anthropic `messages`
- Gemini Native
- Realtime/WebSocket
- Sora / prompt enhance
- API Key 鉴权、RPM / concurrency Redis 风控

结论：`gateway/` 已经不是 placeholder 层，而是项目最成熟的模块之一。

### `handler/`

已形成主链的方向：

- `auth`
- `user`
- `admin`
- `dashboard`
- `verify`

其中最明显的已闭环项：

- 注册 / 登录 / 刷新 / 验证码发送
- 改邮箱验证码消费
- 用户 dashboard 与 `/usage`
- 管理 dashboard `/api/v1/admin/dashboard/*`

### `service/`

已形成主链的方向：

- `usage_log`
- `billing`
- `api_key`
- `dashboard_query_service`
- Realtime / Gemini / Sora 相关 provider service
- `redeem_code`

结论：`service/` 是当前最关键也最不均匀的层，一部分已经承担真实主链，一部分仍偏骨架。

### 数据模型与基础设施层

- `entity/` 与 `migration/` 已支撑当前主业务链
- `health/`、`metrics/` 已具备基础健康检查与指标输出
- deploy、Docker、compose 已完成第一轮收口

### frontend

已形成主链的页面：

- 用户 dashboard
- `usage`
- 管理 dashboard

当前页面能读到真实后端数据，不再只是静态 mock。

## 未完成

### `service/` 仍是最大缺口来源

仍需重点继续收口：

- `backup`：尤其是 `import`、恢复演练、对象存储
- `ops_*`：真实写入、聚合、清理、leader lock
- `batch*`
- 等待队列、调度恢复、快照恢复
- quota / subscription / balance / redeem 的统一解释层

### `handler/` 与 `frontend/` 仍有产品化缺口

- 页面测试仍不足
- loading / error / empty state 还未统一
- 现有 a11y warnings 还没逐页清理
- 权限不足时的前端退化行为还不够明确

### 发布与真实回归仍未完成

- backend image、`core` 栈、`/health` 最终演练仍待完成
- 真实 OpenAI / Gemini smoke 仍未运行
- frontend lint 仍会扫到 `frontend/build/**`

## 下一步

1. 优先补发布闭环，不再继续堆“完成度表格”。
2. 再补真实 provider smoke，把主链从“代码已接真”推进到“真实回归通过”。
3. 然后集中收 `service/` 的控制面缺口：backup、redeem、ops、恢复能力。
4. 最后补 frontend 测试、a11y 与产品化细节，让界面层与后端成熟度对齐。
