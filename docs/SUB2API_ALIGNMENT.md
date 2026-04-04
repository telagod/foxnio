# Sub2API 参考分析

**更新时间**: 2026-04-04
**口径**: 本文档是历史设计参考，不再作为当前项目实现状态或完成度证明

## 现状

FoxNIO 曾参考过 Sub2API 的一些设计点，包括：

- 账户调度
- 模型路由
- 并发控制
- 等待队列
- provider gateway

但当前 FoxNIO 的项目边界已经比“参考某个 gateway 实现”更大，包含：

- 用户自助界面
- 管理控制台
- 账务与兑换
- 部署与发布收口

所以这份文档今天只能作为“曾经参考过什么”的分析材料。

## 已完成

它仍然有一个作用：

- 帮助理解 FoxNIO 某些设计思路来自哪里

## 未完成

它不再应该承担这些职责：

- 证明当前系统已经绝对对齐某个外部项目
- 证明某个模块已完成
- 证明项目已经可直接上线

## 下一步

1. 当前实现状态统一看 [`docs/CURRENT_STATUS.md`](./CURRENT_STATUS.md)。
2. 当前业务闭环统一看 [`docs/BUSINESS_LOGIC.md`](./BUSINESS_LOGIC.md)。
3. 当前发布口径统一看 [`docs/DEPLOYMENT.md`](./DEPLOYMENT.md) 与 [`docs/RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md)。
