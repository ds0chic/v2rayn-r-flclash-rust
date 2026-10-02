# T18b 决策 — 运行时计划构建与生效语义

1. **唯一计划来源**：`apply_runtime` 只能通过 `build_runtime_plan` 从持久化状态（活动节点/策略组展开、AppSettings、活动 Routing/DNS、规则模式）构建；硬编码冒烟计划仅保留在 `bridge_api` 的 test-only 命名入口，生产路径不可达（T18-F03 根因）。
2. **失败语义**：无活动节点、计划校验失败（环/端口/悬空引用/资源缺失）→ 结构化错误，不启动任何内核进程；不静默降级。
3. **desired/applied 分离**：设置/路由/DNS/节点变更只 bump `desired_revision`；成功应用后写 `applied_runtime_revision`。UI 显示"已保存，未应用/已应用 rev"，运行中切换节点等价于 stop→apply。
4. **入站协议**（FLD-CFG-036）：按存储的 `Inbound.Protocol` 生成对应入站结构（mixed/socks/http 等按上游语义），不再恒 `mixed`。
5. **EnableLegacyProtect**（FLD-CFG-102）：按上游消费点核对；无等效内核语义时在设置 UI 标注不适用并登记，不生成悬空字段。
6. **端口安全**：计划校验包含绝对禁令：任何入站/状态端口等于 10808 → 拒绝生成（沿用 T06b 守卫）。
