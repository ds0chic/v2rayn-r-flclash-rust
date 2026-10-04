# RE-PROF-06 evidence

状态：`implemented`。修复“去重/移除无效范围被文字过滤缩小、结果清理误清他组失败记录”的复核问题。
2026-10-04；冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。开始 HEAD `472ca54`（工作树干净）。

## 变更点

- Dart `profiles_controller.dart`：
  - `_profileInCurrentGroup`（含 `state.filter` 文本匹配）替换为 `_profileInGroup`（仅 `subid`），供 `removeDuplicateProfiles` 与 `removeInvalidResults` 使用 → 去重/移除无效范围=当前整组。
  - `removeInvalidResults` 仅在目标全部真实删除（`result.ok && removed == targets.length`）后才调用 `_bridge.removeInvalidResults()`；删除失败/部分失败不清理结果行。
  - `removeDuplicateProfiles` 补删除失败分支（返回 0、不 reload 成功路径、不伪造）。
- Rust `application/src/speedtest.rs`：新增 `ProfileExStore::remove_invalid_in(ids)`（按组清理失败行）与 `remove_invalid_orphans(stored)`（只清 profile 已不存在的失败行）；保留 `remove_invalid`。
- Rust `bridge_api/src/api/speedtest.rs`：`speedtest_remove_invalid()` 改为孤儿清理（`delay == -1 && profile 已不存在`），保存签名、不改 `frb_generated`；新增 `speedtest_remove_invalid_group(subid)`（`#[frb(sync)]`）复用 `AppEngine::remove_invalid_profiles`，供 FRB 重生成后使用。
- Dart `bridge_port.dart`（手写 seam）：`SyntheticBridgePort.removeInvalidResults` 同步孤儿语义；新增 `failDeleteProfiles` 故障注入。

## 上游对照

- `ConfigHandler.DedupServerList:1156-1193`：对象 `ProfileItems(subId)`，无文本过滤 → 现按 `subid` 整组。
- `ConfigHandler.RemoveInvalidServerResult:1595-1611`：对象 `ProfileModels(subid, "")`，`RemoveAll(ConfigType.IsComplexType())`，join `ProfileExs` 取 `Delay == -1` → 现按 `subid` 整组 + 非复杂 + `delay == -1`。
- 结果清理：上游删除 `ProfileItem` 后对应 `ProfileExItem` 随之消失；现网以“只清已不存在 profile 的失败行”实现等价，且不触碰他组仍存在的失败行。

## 本次实际运行

见 `runs.txt`；关键场景见 `observations.json`。全部通过（Rust 2+4+5，Dart 10+4）。未运行真实 Windows 窗口/FRB 全链，未跑全量 workspace，未改宿主代理/注册表/路由/TUN，未占用 10808。
