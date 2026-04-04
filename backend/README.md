# Backend

## 现状

backend 是 FoxNIO 的主服务，当前仍是单体 Axum 应用，同时承担：

- gateway：OpenAI 兼容、Anthropic `messages`、Gemini Native、Realtime、Sora / prompt enhance 等对外入口
- control plane：用户、账号、模型、代理、配额、告警、Webhook、兑换码、审计、健康检查等控制面接口

入口在 `src/main.rs`，主路由在 `src/gateway/routes.rs`。

## 已完成

- 用户侧：dashboard、`/api/v1/user/me`、`/api/v1/user/usage`、API Key 主链已接真。
- 管理侧：`/api/v1/admin/dashboard/*` 与核心管理接口已接真实数据库聚合。
- 网关侧：Realtime、Gemini Native、Sora / prompt enhance 已接真实上游与 usage/audit。
- 认证侧：注册、登录、刷新、验证码发送、密码重置、TOTP 主链已接线。
- 兑换码：已具备事务写库、幂等键与 ledger 审计。
- 配置侧：已统一支持 `config.yaml`、`.env`、`FOXNIO_CONFIG` 与关键 runtime env。
- 工程验证：
  - `cargo fmt --manifest-path backend/Cargo.toml --all`
  - `cargo check --manifest-path backend/Cargo.toml --all-targets --message-format short`
  - `cargo test --manifest-path backend/Cargo.toml config::test::tests:: -- --nocapture`

## 未完成

- 真实 provider smoke 仍因无真实密钥而跳过。
- `backup import` 未实现。
- backup 的对象存储、恢复演练与正式审计未完成。
- redeem 的运营规则、额度来源解释与产品说明还不够完整。
- `ops_*`、`batch*`、等待队列、调度恢复等模块仍偏骨架。

## 启动与配置

### 本地启动

```bash
cargo run --manifest-path backend/Cargo.toml
```

### 关键配置来源

- `config.yaml`
- `FOXNIO_CONFIG`
- `.env`
- `DATABASE_URL`
- `REDIS_URL`
- `JWT_SECRET`
- `JWT_EXPIRE_HOURS`
- `GATEWAY_API_KEY_PREFIX`
- `FOXNIO_SERVER_HOST`
- `FOXNIO_SERVER_PORT`
- `FOXNIO_MASTER_KEY`

### 运行特点

- 启动时会执行 `src/db/pool.rs` 中的 SeaORM migration。
- 数据库与 Redis 连接字符串统一通过 `Config::database_url()` / `Config::redis_url()` 生成。

## 目录

```text
backend/
├── src/main.rs
├── src/config/
├── src/gateway/
├── src/handler/
├── src/service/
├── src/entity/
├── src/db/
├── src/health/
├── src/metrics/
├── migration/
└── tests/
```

## 相关文档

- [后端设计](DESIGN.md)
- [项目当前状态](../docs/CURRENT_STATUS.md)
- [业务逻辑](../docs/BUSINESS_LOGIC.md)
- [部署说明](../docs/DEPLOYMENT.md)
- [长期 TODO](../TODO.md)
