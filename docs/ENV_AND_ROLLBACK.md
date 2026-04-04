# FoxNIO 环境变量与配置说明

## 配置加载优先级

```
环境变量 > .env 文件 > config.yaml > 硬编码默认值
```

## 三层配置关系

| 文件 | 用途 | 谁读 |
|------|------|------|
| `.env` | 本地开发 + deploy.sh 密钥生成 | backend 源码运行、docker-compose 变量插值 |
| `config.yaml` | 连接池、HTTP/2、TLS、网关高级参数 | backend 启动时加载 |
| `docker-compose.yml` | 容器内环境变量（DATABASE_URL/REDIS_URL 硬编码容器网络地址） | Docker 容器 |

**关键区别**：`.env` 中的 `DATABASE_URL` / `REDIS_URL` 指向 `localhost`（本地开发用），`docker-compose.yml` 中硬编码 `postgres:5432` / `redis:6379`（容器网络），互不干扰。

## 必需变量

| 变量 | 格式 | 生成方式 | 说明 |
|------|------|----------|------|
| `JWT_SECRET` | hex string | `openssl rand -hex 32` | JWT 签名密钥 |
| `FOXNIO_MASTER_KEY` | base64 string | `openssl rand -base64 32` | AES-256-GCM 加密主密钥，保护 API Key / OAuth token / TOTP |
| `DATABASE_URL` | postgres URI | 手动 | PostgreSQL 连接串 |
| `REDIS_URL` | redis URI | 手动 | Redis 连接串 |

## 可选变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `JWT_EXPIRE_HOURS` | 24 | JWT 过期时间（小时） |
| `GATEWAY_API_KEY_PREFIX` | sk- | API Key 前缀 |
| `RUST_LOG` | info,foxnio=info | 日志级别 |
| `FOXNIO_SERVER_HOST` | 0.0.0.0 | 监听地址 |
| `FOXNIO_SERVER_PORT` | 8080 | 监听端口 |
| `VITE_API_URL` | http://localhost:8080 | 前端构建时注入的 API 地址 |
| `SMTP_*` | 空 | 邮件发送（注册验证码、密码重置） |
| `*_CLIENT_ID` / `*_CLIENT_SECRET` | 空 | OAuth 提供商凭据 |

## 密钥轮换

`FOXNIO_MASTER_KEY` 支持轮换格式：`新密钥:旧密钥`

```bash
export FOXNIO_MASTER_KEY="new_base64_key:old_base64_key"
```

旧密钥用于解密历史数据，新密钥用于加密新数据。

---

# 最小回滚手册

## 前置条件

- 数据库备份存在于 `/var/backups/foxnio/`
- 旧镜像 tag 已知（`docker images | grep foxnio`）

## 回滚步骤

### 1. 停止服务

```bash
docker compose down
```

### 2. 恢复数据库（如需）

```bash
# 查看可用备份
ls -lt /var/backups/foxnio/

# 恢复指定备份
./deploy.sh restore /var/backups/foxnio/backup_YYYYMMDD_HHMMSS.sql.gz
```

### 3. 回退代码

```bash
git log --oneline -10          # 找到目标 commit
git checkout <commit-hash>     # 切换到目标版本
```

### 4. 重建并启动

```bash
./deploy.sh build
./deploy.sh start
```

### 5. 验证

```bash
bash scripts/smoke-test.sh
```

## 紧急止血（不回退代码）

```bash
# 仅重启服务
docker compose restart backend

# 查看日志定位问题
docker compose logs -f backend --tail 100
```

## 备份策略

- `./deploy.sh backup` — 手动备份（pg_dump + gzip）
- 自动清理 30 天以上备份
- 备份路径：`/var/backups/foxnio/`
