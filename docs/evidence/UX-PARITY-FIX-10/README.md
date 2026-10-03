# UX-PARITY-FIX-10 — 选中测速→结果→按结果处理→重开

状态：`implemented`。修复对象：`FIX-10`（审查台账 `F-PROFILE-005/008/010/011/015`、`F-TEST-001..007`、`ACT-PROF-003/014..021/031/033`，PR-11/12/15/16/17/28）。基线应用 HEAD `c1c9c77`，冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本目录只放本卡证据；未改 `dist/`、`work/`、`outputs/`，未 commit。

## 1. 根因

1. **PR-16**：`ProfilesController.startSpeedTest` 对 Mixed/Fast 传 `ids=[]`，Rust `speedtest_start` 把空集合解释为“全库”；其它测速无选择也回落全库。上游 `ServerSpeedtest` 的 Mixed/Fast 只测 `ProfileItems.OrderBy(Sort)`（当前组+过滤后的可见列表），其它才用 `SelectedProfiles`。
2. **PR-11**：`removeInvalidResults → speedtest_remove_invalid → ProfileExStore.remove_invalid` 只删进程内 `delay=-1` 的测量行，节点本体仍在，重开节点还在。上游 `ConfigHandler.RemoveInvalidServerResult` 按当前组找到 `Delay=-1` 的 `ProfileItem` 并删除（复杂节点排除）。
3. **PR-12**：右键「移除重复」`enabled=false`、动作 `notImplemented`，底层 `subscriptions.deduplicate` 只是纯函数，无 UI→DB 动作。
4. **PR-17**：`SpeedTestHub.results` 为纯内存 `ProfileExStore`，不读/写 SQLite；重开丢失 delay/speed/sort。
5. **PR-15**：拖动/表头/结果排序只改内存，不写上游 `ProfileExItem.Sort`。
6. **PR-28**：UDP 永久禁用。

## 2. 修复

- **PR-16（Dart + Rust）**：`startSpeedTest` 改为 Mixed/Fast 取 `state.visible` 的 id、其它取 `state.selected`；空集合直接判“没有可测试节点”，不再回落全库。`reload()` 改走 `_recompute`，可见集合带当前组 + 文本过滤，保存/删除/刷新后不会漏进别组隐藏节点（同时收口 PR-18）。Rust `speedtest_start` 去掉“空=全库”分支，空集合即无节点。
- **PR-11（Dart + Rust）**：`removeInvalidResults()` 从 `speedTestResults()` 取 `delay == -1` 的 id，仅保留当前组内、非复杂的已存 `ProfileItem`，经既有 `deleteProfiles` 真实删除落库，再清结果行并 reload，返回真实删除数。
- **PR-12（Dart）**：`context_menu.dart` 恢复「移除重复」`enabled`；新增 `profile_dedup.dart` 忠实移植上游 `ConfigHandler.CompareProfileItem`/`DedupServerList`（`KeepOlderDedupl`、复杂节点保留、`eq_opt` 空值等价）；`profiles_table.dart::_removeDuplicate` 弹确认后调 `removeDuplicateProfiles()`，经 `deleteProfiles` 落库。
- **PR-17（Rust）**：`ProfileExItem` 补 `sort` 字段；新增 `AppEngine::profile_ex_all/profile_ex_flush/set_profile_sort/remove_invalid_profiles/deduplicate_profiles`，hub 首次读结果时从 SQLite 懒加载、作业结束/清理/测试写结果时 flush；`ProfileExItem.Sort` 随 SQLite 持久化。
- **PR-15（Rust 能力就绪，UI 未接）**：`AppEngine::set_profile_sort`（`(i+1)*10`）与 `ProfileExItem.Sort` 落库单测；因 `ProfileDto` 无 `Sort` 字段且无对应 FRB 函数，未接 UI，登记接口缺口。
- **PR-28（未完成）**：UDP 仍诚实禁用，标注平台/范围差异。

## 3. 证据

真实 Windows 窗口 + 真实 FRB/Rust/SQLite，隔离 `V2RAYN_R_DATA_DIR`；合成订阅（URL 为 loopback，从未请求）+ 三个 `vless://` 合成节点：两个指向本地 loopback 监听端口（`11808..`，仅 accept），一个指向已释放死端口。不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。

| 文件 | 内容 |
|---|---|
| `observations.json` | run：5/5 check 通过，`recordingComplete=true`，`failures=[]` |
| `reopen-observations.json` | reopen（新进程、同 data dir、新 engine/controller）：2/2 通过 |
| `01-results.png` | 选中 3 节点 Tcping 后：两个 `1 ms`、死端口节点失败 |
| `02-after-remove-invalid.png` | 「按测试结果移除无效」后：剩 2 个有效节点 |
| `reopen-window.png` | 重开进程后失败节点仍不在、两个有效节点仍在 |

run 连接（`observations.json`）：`subscription-created` → `nodes-imported-into-group`(3) → `selection-ready`(3) → `tcping-results-measured`（liveDelays=[1,1]、deadDelay=-1）→ `remove-invalid-deletes-profile`（removed=1，remaining=2）。

reopen（`reopen-observations.json`）：`reopen-removed-node-absent`（removedId 不在）、`reopen-survived-nodes-exist`（2 个有效节点存在），证明删除落库且重开一致。

## 4. 命令与结果

```
# 真实窗口（新 data dir）
$env:V2RAYN_R_DATA_DIR=<temp>\fix10-data
$env:V2RAYN_R_FIX10_EVIDENCE=docs\evidence\UX-PARITY-FIX-10
$env:V2RAYN_R_FIX10_MODE=run
flutter test integration_test/ux_parity_fix10_speedtest_result_test.dart -d windows   # +1 All tests passed

# 重开（同 data dir，新进程）
$env:V2RAYN_R_FIX10_MODE=reopen
flutter test integration_test/ux_parity_fix10_speedtest_result_test.dart -d windows   # +1 All tests passed

# widget/单元
flutter test test/t15b_speedtest_test.dart    # +7
flutter test test/fix10_dedup_test.dart       # +4

# Rust
cargo fmt -p application -p bridge_api -- --check                                     # 无差异
cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings       # exit 0
cargo test -p application -p bridge_api --locked                                      # 全绿（含 fix10_speedtest_result 4/4）
```

首次 run/reopen 各遇到一次已记录的 flutter_tester/Windows 原生间歇崩溃（`did not complete`，无 Dart 栈），清理 data dir 重跑即通过；与本修复无关。

### 门禁结果

| 门禁 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib test integration_test` | 190 文件，0 改变，exit 0 |
| `flutter analyze` | No issues found |
| `flutter test test/t15b_speedtest_test.dart` | 7 passed |
| `flutter test test/fix10_dedup_test.dart` | 4 passed |
| 真实窗口 `run` | +1 All tests passed（5/5 check，failures=[]） |
| 真实窗口 `reopen` | +1 All tests passed（2/2 check，failures=[]） |
| `cargo fmt -p application -p bridge_api -- --check` | 无差异 |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p application -p bridge_api --locked` | 全绿（application 150 + 各集成；bridge_api 43；fix10_speedtest_result 4） |
| `flutter build windows --release` | 未跑（根代理统一跑，本卡未越界） |

### 证据哈希（SHA256）

```
d7603f7a7f03db683e5e1281979ba17a410a1f4427c2d682cacd764e8a9be938  01-results.png
37880839e40316cab3874a451a84777ade41f52395709c264ad3f646241bfc92  02-after-remove-invalid.png
68fef3eaa7de5eb4b76f764f6f4180c15cf810d77a2ddbc32c973a85ff147a1b  observations.json
e1f48b2717acdddcc857f32a870c771c75ccb2acab3b5bd3f5d15bfe7c002d74  reopen-observations.json
c52a4c25ac78a62dcdff11efa1590e9f3749c8705acca19fa63cc2c407d6fc4d  reopen-window.png
```

## 5. 限制与未验证

- 未做原版实机双窗口、同 DPI 逐事件像素对照；状态为 `implemented`，不写 `verified`。
- 排序重开持久化（PR-15）受桥接函数缺口限制：`ProfileDto` 无 `Sort` 字段、无排序 FRB 函数，UI 侧仍未接入写 `Sort`。Rust 用例与 `ProfileExItem.Sort` 落库已就绪并有单测。
- PR-11 的 Rust 端“按组一次删除”函数（`AppEngine::remove_invalid_profiles`）已实现并单测，但桥接 `speedtest_remove_invalid()` 保持无 `subid` 的结果行清理语义；真实删除由 UI 经 `deleteProfiles` 完成。
- UDP（PR-28）未恢复适用实现，保持诚实禁用。
- 6 类测速本轮完成 Tcping 主链；TCPXray/延迟/速度/UDP/混合的实机结果矩阵按本卡“其余类型按序补测”留待后续。
- 真实窗口仅 Windows；macOS/Linux/ARM64/Avalonia 未验证。
