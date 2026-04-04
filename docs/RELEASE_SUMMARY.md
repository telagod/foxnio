# FoxNIO 发布总结

**更新时间**: 2026-04-04
**口径**: 本文档记录当前仓库已完成的发布收口，不再保留旧版本号、旧日期与旧上线结论

## 现状

FoxNIO 当前的发布状态可以概括为一句话：

主业务链已经大体接真，发布口径已经完成第一轮收口，但还没有完成“干净环境部署 + 最小 smoke + 真实 provider smoke”的最终闭环。

当前真正发生过的进展：

- 用户 dashboard、`/usage`、管理 dashboard 已接真实后端聚合
- Realtime/WebSocket、Gemini Native、Sora / prompt enhance 已接真实上游与 usage / audit / quota
- runtime config、Docker、compose profile、deploy 脚本已收口

当前仍不能下的结论：

- 不能写“所有 provider 已可直接上线”
- 不能写“某个旧版本已发布完成”
- 不能写“整体功能已全部完成”

## 已完成

### 已完成的业务主链

- 用户侧：个人信息、API Key、dashboard、`/usage`
- 管理侧：`/api/v1/admin/dashboard/*` 与基础控制台查询
- 网关侧：Realtime、Gemini Native、Sora / prompt enhance
- 认证侧：注册、登录、刷新、验证码发送、改邮箱验证码消费、密码重置、TOTP
- 账务侧：usage 写库、quota 消耗、redeem 幂等与 ledger 审计

### 已完成的工程收口

- backend 已统一 `config.yaml`、`.env`、runtime env 读取
- frontend Docker 已切回 `adapter-node`
- compose 已拆为 `core` / `ui` / `edge`
- `deploy.sh` 已成为统一部署入口

### 当前已核验证

- backend `cargo check`
- frontend `npm run check`
- `git diff --check`

## 未完成

### 发布链仍缺最终结论

还缺：

- backend release image 最终构建结果
- 干净环境起 `core` 栈并记录 `/health`
- `ui` 与 `edge` 组合起服验证
- 最小 smoke 脚本与回滚手册

### 真实 provider smoke 未完成

阻断条件仍然存在：

- 缺真实 OpenAI / Gemini 密钥
- 缺真实 provider account / OAuth token

所以目前只能说“代码链存在”，还不能说“真实 provider 已回归通过”。

### 文档收口未完成

虽然主文档已重写，但 `docs/` 下仍有其他历史专题文档需要继续清理旧完成度口径。

## 下一步

1. 先完成发布链最终演练：build、start、`/health`、最小 smoke。
2. 再补真实 provider smoke，把发布结论从“代码已接真”推进到“真实回归通过”。
3. 最后继续清理历史专题文档，保证全仓库只保留一套权威状态口径。
