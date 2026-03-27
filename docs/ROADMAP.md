# FoxNIO Development Roadmap

## Phase 1: 基础架构 ✅

- [x] 项目结构创建
- [x] Rust 后端骨架（Axum + SeaORM）
- [x] SvelteKit 前端骨架
- [x] 液态玻璃 UI 设计系统
- [x] 深浅色主题切换

## Phase 2: 核心功能（进行中）

### 后端
- [ ] 数据库迁移脚本（SeaORM Migration）
- [ ] 用户认证系统（JWT + TOTP）
- [ ] API Key 生成与管理
- [ ] 上游账号管理（OAuth + API Key）
- [ ] 网关核心逻辑
  - [ ] OpenAI 兼容端点 `/v1/chat/completions`
  - [ ] Anthropic 兼容端点 `/v1/messages`
  - [ ] Gemini 兼容端点 `/v1beta/`
- [ ] 智能调度器
- [ ] 计费系统

### 前端
- [ ] 登录/注册页面
- [ ] 用户仪表盘
- [ ] API Key 管理界面
- [ ] 账号管理界面
- [ ] 用量统计图表
- [ ] 管理后台

## Phase 3: 高级功能

- [ ] 粘性会话
- [ ] 并发控制
- [ ] 速率限制
- [ ] Webhook 支持
- [ ] 审计日志
- [ ] 告警系统

## Phase 4: 部署与优化

- [ ] Docker 镜像
- [ ] Kubernetes Helm Chart
- [ ] 性能优化
- [ ] 安全加固
- [ ] 文档完善

---

## 设计原则

1. **优雅**：代码简洁，架构清晰
2. **专业**：功能完整，文档详尽
3. **克制**：避免过度设计，保持专注

## 命名由来

FoxNIO = Fox（狐狸，聪明灵活）+ NIO（非阻塞 I/O，高性能）
