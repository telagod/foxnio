# Sub2API 对齐说明

**更新时间**: 2026-04-04
**口径**: 本文档只保留“FoxNIO 曾参考 Sub2API 的哪些方向”，不再输出旧的绝对对齐结论、旧版本号、旧上线结论

## 现状

FoxNIO 的一部分设计曾明确参考 Sub2API，包括但不限于：

- 多 provider 网关入口
- 账户调度与模型路由
- API Key 风控
- 等待队列与限流
- Webhook、批量操作、监控等控制面能力

但当前项目已经发生了两种变化：

- 一方面，FoxNIO 已经发展出自己的用户侧、管理侧、控制面与发布结构，不再只是“对齐某个参考实现”
- 另一方面，旧文档里关于“绝对对齐”的表述已经失真，因为仓库现状并不支持这种结论

所以这份文档现在的作用应当是：

- 说明曾参考过哪些方向
- 说明哪些能力在代码中已有影子或主链
- 明确它不再是当前项目完成度的权威来源

## 已完成

从当前代码状态看，以下方向已经形成可见成果：

- 多 provider 网关主链
- API Key 鉴权与 Redis 风控
- 用户 dashboard、`/usage`
- 管理 dashboard
- Realtime/WebSocket usage / audit
- Gemini Native usage / quota
- Sora / prompt enhance usage / quota
- redeem 幂等与 ledger 审计

这些结果与早期“参考 Sub2API 的能力方向”有关，但它们现在应以 FoxNIO 自身文档为准，而不是继续用“对齐度”描述。

## 未完成

以下内容不能再写成“绝对对齐”：

- 真实 provider smoke
- backup import 与恢复演练
- ops / batch / 调度恢复等控制面完善度
- quota / subscription / balance / redeem 的统一产品语义
- 干净环境部署与最小 smoke 的最终闭环

换言之，FoxNIO 当前更适合用“已闭环主链 / 未闭环主链”来描述，而不是继续追求某个历史参考项目的百分比。

## 下一步

1. 把当前状态判断统一收口到：
   - `docs/CURRENT_STATUS.md`
   - `docs/BUSINESS_LOGIC.md`
   - `docs/DEPLOYMENT.md`
   - `docs/RELEASE_CHECKLIST.md`
2. 继续把历史“对齐度”文档降级为参考资料，而不是权威状态文档。
3. 如果后续还需要保留 Sub2API 参考关系，应改写成“设计参考点”而不是“完成度证明”。

## 说明

- 当前项目状态请以 [`docs/CURRENT_STATUS.md`](./CURRENT_STATUS.md) 为准。
- 当前业务主链请以 [`docs/BUSINESS_LOGIC.md`](./BUSINESS_LOGIC.md) 为准。
- 当前发布口径请以 [`docs/DEPLOYMENT.md`](./DEPLOYMENT.md) 与 [`docs/RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) 为准。
