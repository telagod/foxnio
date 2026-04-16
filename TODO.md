# FoxNIO TODO

**更新时间**: 2026-04-13  
**目标**: 把项目从“主链已接真”推进到“可部署、可回归、可运营、可解释的完整可用产品”

## 现状

当前仓库最重要的变化不是“又补了一个页面”，而是三类主链已经基本接真：

- 用户侧：dashboard、API Key、`/usage`
- 管理侧：`/api/v1/admin/dashboard/*` 与基础控制台查询
- 网关侧：Realtime、Gemini Native、Sora / prompt enhance

当前主矛盾也已经很明确：

- 不是“系统完全跑不起来”
- 而是“真实 smoke 缺密钥、发布链还差端到端回归、控制面仍有明显骨架、文档体系尚未完全收束”

## 已完成

- [x] 用户 dashboard 真实化
- [x] 用户 `/usage` 日维度 usage 聚合
- [x] 管理 dashboard `/api/v1/admin/dashboard/*`
- [x] `usage_log` 接真实 `usages` 表
- [x] API Key 级 Redis RPM / concurrency 风控
- [x] 用户侧 API Key 创建、删除、列表主链
- [x] 注册 / 登录 / 刷新 / 验证码发送主链
- [x] 改邮箱验证码消费主链
- [x] redeem code 写库、幂等键、ledger 审计主链
- [x] Realtime/WebSocket 鉴权、上游转发、usage / audit
- [x] Gemini Native retry、typed error、usage / audit / quota
- [x] Sora / prompt enhance 真实 OpenAI 转发、usage、quota
- [x] smoke seed CLI 与真实 smoke 脚本骨架
- [x] runtime env / config 统一加载
- [x] frontend `adapter-node` Docker 运行口径
- [x] compose `core` / `ui` / `edge` 分层
- [x] `deploy.sh` 第一轮收口
- [x] backend `cargo check`
- [x] frontend `npm run check`
- [x] frontend Tailwind v3 → v4 (CSS-first)
- [x] frontend ESLint v8 → v10 (flat config)
- [x] frontend 全页面去 emoji 换 SVG、a11y 修复、深浅色、响应式
- [x] frontend svelte-check 0 errors 0 warnings
- [x] deploy.sh MASTER_KEY 格式修正 (hex → base64)
- [x] docker-compose.yml 容器内 DB/Redis 地址硬编码

## 未完成

### P0 - 发布与上线阻断

- [x] backend release image 完整构建并记录最终镜像口径
- [x] `docker compose up -d postgres redis backend` 后 `/health` 回归通过
- [x] `./deploy.sh build && ./deploy.sh start` 在干净环境跑通
- [x] `./deploy.sh build-ui && ./deploy.sh start-ui` 跑通 frontend Node server
- [x] `./deploy.sh build-edge && ./deploy.sh start-edge` 跑通 nginx profile
- [x] 建最小上线 smoke：
- [x] `GET /health`
- [x] `GET /v1/models`
- [x] 一条认证接口
- [x] `GET /api/v1/user/usage`
- [x] `GET /api/v1/admin/dashboard/stats`
- [x] 写清最小回滚手册
- [x] 写清环境变量权威说明
- [x] 修正 frontend lint 范围，排除 `frontend/build/**`

### P0 - 真实 provider smoke

- [ ] 准备最小真实输入物
- [ ] 1 个用户
- [ ] 1 个 FoxNIO API Key
- [ ] 1 个 OpenAI account / key
- [ ] 1 个 Gemini account / OAuth token
- [ ] 跑真实 OpenAI smoke
- [ ] `chat/completions`
- [ ] `responses`
- [ ] `realtime`
- [ ] `images/videos`
- [ ] `prompt enhance`
- [ ] 跑真实 Gemini smoke
- [ ] `generateContent`
- [ ] `streamGenerateContent`
- [ ] `countTokens`
- [ ] `embedContent`
- [ ] 把真 smoke 成功 / 失败结果写回文档

### P1 - failure chain 与账务口径

- [x] Realtime integration tests
- [x] `upstream_prepare_failed`
- [x] `upstream_connect_failed`
- [x] `upstream_stream_error`
- [x] `response.failed`
- [x] Gemini integration tests
- [x] stream read error
- [x] quota consume metadata
- [x] failed usage metadata
- [x] Sora / prompt enhance integration tests
- [x] 成功 usage
- [x] 失败 usage
- [x] 状态查询失败
- [x] quota consume
- [x] 输出 usage / audit 权威核验表
- [x] 每条成功写什么
- [x] 每条失败写什么
- [x] metadata 长什么样

### P1 - 控制面补齐

- [x] backup 正式化
- [x] `import`
- [x] 对象存储
- [x] 生命周期
- [x] 恢复演练
- [x] 审计
- [x] redeem 正式化
- [x] 活动规则
- [x] 限次与幂等
- [x] 额度来源解释
- [x] ledger 查询接口
- [x] 账务 / quota / subscription / redeem 解释统一
- [x] account / group / control-plane 查询链继续接真

### P2 - 运维与异步能力

- [x] `batch.rs` / `batch_operations.rs` 真执行链
- [x] scheduled test plan 真执行
- [x] `timing_wheel_service.rs` 真任务执行
- [x] `scheduler_snapshot_service.rs` 恢复能力
- [x] `token_refresh_service.rs` 真刷新链
- [x] 模型级 / 账号级 / 排队层限流统一
- [x] 等待队列 Redis 持久化、超时、取消、恢复
- [x] `ops_*` 模块真实写入、聚合、清理、leader lock

### P2 - 前端产品化

- [x] `dashboard` 页面测试
- [x] `usage` 页面测试
- [x] `apikeys` 页面测试
- [x] `admin` 页面测试
- [x] loading / error / empty state 统一
- [x] 用户侧与管理侧 UI 文案统一
- [x] 权限不足时前端回退行为明确化
- [x] 现有 a11y warnings 逐页清理

### P3 - 文档与口径清理

- [x] 清理 `docs/` 历史专题文档中的旧完成度表述
- [x] 统一 migration 权威路径说明
- [x] 统一 frontend 构建 / 运行 / Docker 命令口径
- [x] 统一 `npm` / `pnpm` / `make` 的权威命令说明
- [x] 统一 `.env` / `config.yaml` / runtime env 关系说明
- [x] 输出”最小上线架构图”和”最小回归矩阵”

## 下一步

1. 把 scheduler / cooldown / sticky session 指标推进到 admin stats，补成运营可直接判断的看板。
2. 固化 benchmark methodology，并生成 1k / 10k / 100k 号池样本口径。
3. 用真实密钥完成 OpenAI / Gemini 真 smoke，补齐真实 provider 结果。
4. 继续把批量导入、批量轮换、批量状态治理做成运营级能力，而不是单次管理动作。
