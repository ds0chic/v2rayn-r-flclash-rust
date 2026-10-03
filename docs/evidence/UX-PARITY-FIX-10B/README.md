# UX-PARITY-FIX-10B — 节点表拖动/表头/结果排序写原版排序字段并重开保持

状态：`implemented`。修复对象：`FIX-10B`（来源 `FIX-10` 的 PR-15 接口缺口登记；`ACT-PROF-020/031/033`、`F-PROFILE-008/011`、`FLD-ENT-107`）。基线应用 HEAD `72e4c1e`（工作树含根代理已完成的 FRB 再生成），冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本目录只放本卡证据；未改 `work/`、`outputs/`、`dist/`，未 commit。

## 1. 根因（PR-15）

FIX-10 时：拖动、表头排序、按结果排序只改 Dart 内存顺序，不写上游 `ProfileExItem.Sort`，重开丢失。当时缺两件东西：
1. Dart 桥接没有写 `Sort` 的函数（`ProfileDto` 无 `Sort` 字段，`SpeedTestResultDto` 无 `sort`）。
2. UI 三种排序路径都没有触发任何持久化。

本轮根代理已生成新桥接 `speedtestApplyProfileOrder(ordered_ids)` → Rust `ProfileExStore::apply_order`（写 `(i+1)*10`）→ `profile_ex_flush` 落 SQLite。

## 2. 修复（本卡）

- **Dart 桥接 seam**：`apps/desktop/lib/bridge/bridge_port.dart` 新增 `applyProfileOrder(List<String>)`；`FrbBridgePort` 转发到生成的 `speedtest.speedtestApplyProfileOrder`；`SyntheticBridgePort` 记录 `appliedProfileOrders`（仅测试，非成功信号）。未改任何生成文件。
- **控制器**（`apps/desktop/lib/features/profiles/profiles_controller.dart`）：
  - 新增 `_persistOrder()`：取 `state.visible` 的 id 序列，`length < 2` 直接返回（空/单行不写、不报错），否则 `_bridge.applyProfileOrder(ids)`。
  - `sortBy(key)`（表头排序）、`sortByResult()`（按测试结果排序）、`moveSelected(...)`（上移/下移/置顶/置底）与 `handleDrop(source, target)`（拖动排序）在更新顺序后调用 `_persistOrder()`。
  - `handleDrop` 从「只记日志」改为真实重排：把 source 行移到 target 行位置，经 `_withVisibleOrder` 把可见序映射回 `all`（隐藏行保持相对位置），再持久化；拖到自身/未知 id/单行是 no-op。
- **Rust 读回单测**（`crates/application/src/speedtest.rs`，只加）：`apply_order_writes_step_ten_and_reads_back_stably` 证明 `(i+1)*10`、`max_sort`、按 `Sort` 重排读回序、且不触碰 delay/speed；`apply_order_is_noop_for_empty_list` 证明空列表不写。

与上游对照：`ConfigHandler.SortServers`（`:1004`）按列排序后对整表 `SetSort((i+1)*10)`，Delay/Speed 失败行沉底 `maxSort`；`MoveServer`（`:476`）对可见列表 `SetSort((i+1)*10)` 再微调被移动行。本卡统一为「UI 给出最终有序 id 列表 → 整表 `(i+1)*10`」；结果排序的失败/未知沉底由 Dart 排序实现，与上游 `DelayVal` maxSort 沉底等价。未做原版实机双窗口逐事件对照。

## 3. 证据

合成/单元级：`SyntheticBridgePort` 合成节点（无原生库），控制器经 `ProviderContainer` 覆盖 `bridgePortProvider`。

| 用例 | 断言 |
|---|---|
| 表头排序 | `sortBy('Remarks')` 后 `appliedProfileOrders.last == 新可见序` |
| 结果排序 | 注入 delay（200/10）后 `sortByResult()` 持久化 delay 升序，低延迟在首 |
| 拖动排序 | `handleDrop(rows[2], rows[0])` 后首行＝原第 3 行且持久化；拖到自身不多写 |
| 上移/下移/置底 | 每步都持久化当前可见序，行位置符合预期 |
| 空/单行 | 过滤到 1 行/0 行后排序/拖动均不调用桥接、不抛错 |

`observations.json` 记录上述用例与命令结果。

## 4. 命令与结果

```
# Dart 定向
dart format --output=none --set-exit-if-changed lib/bridge/bridge_port.dart lib/features/profiles/profiles_controller.dart test/fix10b_profile_order_test.dart   # 0 changed
flutter analyze lib/bridge/bridge_port.dart lib/features/profiles/profiles_controller.dart test/fix10b_profile_order_test.dart                                  # No issues found
flutter test test/fix10b_profile_order_test.dart                                                                                                                # 5 passed

# Rust
cargo fmt -p application -- --check                                                                        # 无差异
cargo test -p application --lib speedtest --locked                                                         # 19 passed（含新增 2）
cargo test -p application --test fix10_speedtest_result --locked                                           # 4 passed（含 profile_ex_and_sort_survive_reopen 读回）
```

未跑：全 workspace 测试、`flutter build windows --release`、真实窗口 reopen 并集（根代理统一跑）。`flutter analyze` 全量当前因另一子代理并行改 `features/monitor/**` 报 `ClashModeDto`/`monitorSetDelayUrl` 等错误，与本卡文件无关；本卡文件单独 analyze 干净。

## 5. 限制与未验证

- Dart 层「按 Sort 重排节点表」受 DTO 缺口限制：生成的 `SpeedTestResultDto` 无 `sort`、`ProfileDto` 无 `Sort`，Dart 读不回 `Sort`。本卡按任务以 Rust 层证明读回；若要在 Dart 层恢复节点表顺序，需给共享桥接 DTO 增 `sort` 字段（改生成文件，本卡禁改）。
- 未做原版实机双窗口逐事件对照；状态 `implemented`，不写 `verified`。
- 仅 Windows 验证的路径未跑；macOS/Linux/ARM64 未验证。
