# UX-PARITY-FIX-09C — 订阅前置/后置节点链

任务卡：`docs/tasks/FIX-09C.md`。状态：`implemented`。

## 上游对照（冻结 `7d6a967`）

- `CoreConfigContextBuilder.ResolveNodeAsync`（:220-244）：活动节点带 Subid 时尝试 `BuildSubscriptionChainNodeAsync`；命中则把虚拟 `ProxyChain` 加入 `AllProxiesMap` 并递归解析（不再递归建链）；未命中但存在悬空告警时，按原节点注册并前置告警。
- `BuildSubscriptionChainNodeAsync`（:258-315）：
  - 守卫 `node.Subid.IsNullOrEmpty() || node.ConfigType == EConfigType.Custom` → 不建链。
  - `SubItem` 缺失 → 不建链。
  - `prevNode = GetProfileItemViaRemarks(SubItem.PrevProfile)`；`nextNode = ...NextProfile`；`null` 分别告警 `MsgSubscriptionPrevProfileNotFound` / `MsgSubscriptionNextProfileNotFound`。
  - `prevNode is null && nextNode is null` → 不建链。
  - 合成节点：`IndexId = inner-{guid}`、`ConfigType = ProxyChain`、`CoreType = GetCoreType(node)`、`Remarks = node.Remarks`、`GroupType = "ProxyChain"`、`ChildItems = join([prev?.IndexId, node.IndexId, next?.IndexId], ",")`。
- `AppManager.GetProfileItemViaRemarks`（:287）：`FirstOrDefault(it => it.Remarks == remarks)`，精确匹配。

本项目实现：`crates/application/src/subs.rs::build_subscription_chain_node`（合成 + 告警），`crates/application/src/engine.rs::build_codegen_input_with_warnings`（装配期替换 `input.profile`）。Remarks 匹配取 index_id 升序首个（等价于上游 FirstOrDefault，去掉 DB 无序性）。`ChildItems` 保序去重，对应上游遍历期 `globalVisitedGroup` 去重。

链式生成未改动，复用 FIX-03C 的 `build_chain_outbounds_list`：`ChildItems` 反序、Xray `streamSettings.sockopt.dialerProxy` / sing-box `detour`、入口 tag=`proxy`、无 balancer。因此订阅链的 `ChildItems=[prev,node,next]` 生成时反序为 `[next,node,prev]`，`proxy` 入口落在原 `next`。

## 实际命令与结果

```
cargo fmt -p application -- --check
# -> exit 0（干净）

cargo clippy -p application --all-targets --locked -- -D warnings
# -> Finished dev profile，无告警

cargo test -p application --lib subscription_chain --locked
# -> 4 passed
#    engine::tests::subscription_chain_synthesizes_prev_active_next
#    engine::tests::subscription_chain_dangling_reference_warns_and_falls_back
#    engine::tests::subscription_chain_missing_both_does_not_wrap
#    engine::tests::subscription_chain_excludes_custom_and_unsubscribed_nodes

cargo test -p application --lib --locked
# -> 192 passed; 0 failed

cargo test -p bridge_api --lib --locked
# -> 51 passed; 0 failed
```

未运行：原版实机双窗口逐事件对照、真实窗口集成（登记为待根代理统一跑）；未跑全量 workspace 测试、未跑 flutter build windows。

## 断言覆盖

| 合同 | 断言 |
|---|---|
| 生成期合成头/尾节点并按上游命名/串联 | `subscription_chain_synthesizes_prev_active_next`：`input.profile.config_type == ProxyChain`、`index_id` 前缀 `inner-`、`group_type == "ProxyChain"`、`child_items == prev,active,next` |
| 与 FIX-03C 链式生成一致（反序/dialerProxy/detour/入口 proxy） | 同测试：Xray `outbounds[0].tag == "proxy"` 且 `settings.address == next.address`、`streamSettings.sockopt.dialerProxy` 存在；sing-box `outbounds[0].tag == "proxy"` 且 `detour` 存在 |
| 不修改存储 ProfileItem | 同测试：`profile_by_id(active)` 仍为 Vless、`child_items.is_none()`、remarks 不变 |
| 悬空引用明确告警 + 回退 | `subscription_chain_dangling_reference_warns_and_falls_back`：1 条 `error.subscription_prev_profile_not_found`（`field_path=SubItem.PrevProfile`），链回退为 `active,next` |
| 双空不建链 | `subscription_chain_missing_both_does_not_wrap`：无告警、`input.profile` 仍为 Vless |
| 排除 Custom / 无 Subid | `subscription_chain_excludes_custom_and_unsubscribed_nodes` |

## 未完成 / 接口缺口

- 上游节点选择器（`SetConfigTypeFilter([Custom], exclude:true)`）未实现；本项目 UI 用文本输入框写 Remarks，数据流可用但交互不等价。
- 未做上游 `NodeValidator` 对 prev/next 叶子的逐节点校验与 `MsgGroupChildNode*` 告警；本卡只做 Remarks 悬空告警。
- 未做原版实机双窗口逐事件对照。

## 下一步前置

- 若要 `verified`：跑原版实机双窗口同数据目录对照（订阅设置前后置 → 生成 → 启动内核），并补 UI 节点选择器与 widget 测试。
