# SP-20 连接表列和右键 — 完整卡证据（implemented）

状态：**implemented**（2026-10-07 按唯一用户流程实现完成；verified 待真实核心
端到端与 SP-34 完整门禁）。基线 `3635392`。不 commit。`work/`、`outputs/` 只读
（仅读原版对照，未动）；`compat/` 未动；engine.rs、lib.rs、runtime_bridge /
runtime_controller（A08）、status_bar（A08）、IPC/stable DTO、BridgePort、FRB、
Cargo 锁均未动。合成数据（`*.example.invalid`），无 socket 直连外网、无代理/
10808 操作；测试端口仅 Rust stub 回环监听（首个空闲 ≥11808，断言≠10808）。

唯一用户流程：用户排列连接列、右键关闭后重开，布局保持且真实连接管理有效。

## 0. 原版对照（v2rayN 7.25.4 / 7d6a967，只读 `work/` 实读）

- `v2rayN/Views/ClashConnectionsView.xaml`：列 `ExName` 即稳定列键，默认宽
  Host=300 / Chain=500 / Network=80 / Type=160 / ProcessPath=100 / Elapsed=100；
  右键菜单 `menuConnectionClose` + `menuConnectionCloseAll`；另有关闭全部按钮、
  列宽自适应按钮、自动刷新开关。
- `ClashConnectionsView.xaml.cs`：`RestoreUI` 按
  `ClashUIItem.ConnectionsColumnItem.OrderBy(Index)` 恢复（仅 `Width > 0` 覆盖，
  `DisplayIndex` 重排）；`StorageUI` 退出时按 Name/ActualWidth/DisplayIndex 回写。
- `ServiceLib/ViewModels/ClashConnectionsViewModel.cs`：`ConnectionCloseCmd`
  以 `canEditRemove`（`SelectedSource.Id` 非空）门控；`ClashConnectionClose(all)`
  冻结选中 id（空 id 直接返回），关闭全部走独立空 id 路径，关闭后重查连接；
  刷新门控 `AutoRefresh && ShowInTaskbar && IsRunningCore(sing_box)` 且
  `ConnectionsRefreshInterval > 0` 按 tick 取模。
- 关键安全语义：单条关闭空 id 绝不发出（否则 upstream `CloseConnection("")`
  即 `DELETE /connections/` 会误关全部）——本卡在 controller 层守住。

## 1. 改动文件（写锁内）

Dart（`apps/desktop/lib/features/monitor/`）：
- `connections_columns.dart`（+`moveConnectionColumn` 纯函数：移动并重编
  0..n-1，宽度随列，越界钳制）。
- `clash_ui_config.dart`（+`ClashUiConfig.connectionsColumns` 原始
  `ConnectionsColumnItem` 行解析 + `asColumnRows`；缺失回退空即上游默认）。
- `monitor_controller.dart`（`closeConnection` 请求时冻结 id+generation：
  空 id 永不发桥（`false`）；会话切换后迟到回包不认成功、不刷新新会话列表；
  `closeAllConnections` 同加 generation 守卫；失败不重查不丢行）。
- `connections_view.dart`（打开时 `resolveVisibleColumns` 恢复列序/列宽并按宽
  渲染；表头拖拽排列（`DragTarget`+`moveConnectionColumn`）每次变更同步
  `saveGroup('ClashUIItem', ConnectionsColumnItem)`；列宽自适应按钮恢复上游
  默认并落盘；每行右键菜单“关闭连接”（空 id 禁用）/“关闭全部”，目标 id 在
  菜单打开时冻结，关闭失败 SnackBar 明示；退出时无需额外回写——内存列与落盘
  行永不偏离，见 §4 决议）。

Rust（`crates/application/src/monitor.rs`，仅本文件内测试追加，生产逻辑不动——
真实 endpoint 路径已由 SP-17/R4-23 落地，本卡钉住不许退化）：
- `sp20_list_and_close_use_real_endpoint_with_frozen_id`：std-only stub
  controller（floor 端口 ≥11808，≠10808），真实 `ClashApiService` 列表解析
  （upload/download/id/host/chains）+ 冻结 id 关闭命中
  `DELETE /connections/c1` + 关闭全部走独立 `DELETE /connections/`。
- `sp20_close_failure_is_surfaced_not_fake_success`：stub 500 → `Err(500)`
  且请求仍到线（无静默跳过、无假成功）。
- `sp20_column_layout_survives_real_json_roundtrip`：normalize → 真实 serde
  `domain::ClashUiItem` → 反序列化 → normalize，独立重开同序同宽。

测试：`apps/desktop/test/repair/sp_20_connections_full_test.dart`（新增，10 用例：
纯重排+配置解析 2、controller 冻结/失败/迟到 3、widget 恢复/右键单关/
右键全关/autofit/独立重开 5）；`sp_20_connections_prep_test.dart`（头注更新，
9 用例保持回归钉）。

持久化通道复用既有（`domain::ClashUiItem.connections_column_item` ↔
`persistence::clash_columns` ↔ `ClashUiItemDto` ↔ settings document），未新增 DTO。

## 2. 先红后修

- 红：新 full 测试实现前编译失败（`moveConnectionColumn` /
  `connectionsColumns` 缺符号），exit **1**；controller 空 id/迟到回包旧行为
  与新断言冲突（旧 `closeConnection('')` 返回真并触桥）。
- 绿：实现后 `sp_20_*` 19/19；Rust 3 新测一次写对即绿（钉住既有真实路径，
  无生产变更，见 `commands.log` R11-R13）。

## 3. 定向检查（实际运行）

- `cargo fmt --all -- --check`：exit **0**（两处新测格式按 rustfmt 提示收敛）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：exit **0**。
- `cargo test -p application --locked --lib`：**330 passed / 0 failed**；
  monitor 过滤 **27 passed / 0 failed**（含 3 新测）。
- `dart format`（本卡 6 文件）：0 改动。
- `flutter analyze`：本卡文件零 issue；全应用 2 条 error 均在他卡文件
  （`test/repair/sp_19_settings_entry_test.dart:45-46`，本卡未动，基线即坏）。
- `flutter test test/repair/sp_20_*`：**19/19**（10 新 + 9 prep）；合成 1k/10k
  唯一 id、全 `.example.invalid`；Rust stub 仅回环 floor 端口（≥11808）。
- 受影响既有 monitor 测试：fix16c / fix11 / fix11b / fix11c / t15a /
  r4_23 **36/36** 通过（sp_20 整文件批跑中出现 1 次单用例 `did not complete`
  无断言失败，重跑即绿，属已知 flutter_tester 菜单动画竞态 flake，已记录）。
- 未运行（VALIDATION_POLICY.md 定向范围外，留 SP-34）：`flutter build
  windows --release`（无 native/AOT 接线变更）、`cargo test --workspace`、
  `tools/gates/run_all.ps1`。

## 4. 决议与偏离记录

- D1 退出时回写取消：初版在 `dispose` 用 `ref` 回写，Riverpod 禁止且 teardown
  顺序下 provider 可能已销毁（`UnmountedRefException`，测试复现）。改为每次
  变更同步落盘，内存与落盘永不偏离，退出回写无存在必要；`_setAutoRefresh`
  经 `clashUiGroupWith` 保留未知键，不覆盖列行。行为等价于 `StorageUI`。
- D2 右键菜单锚定：`showMenu` 锚于 secondaire 点击点；菜单动画未稳时点选会
  miss，测试侧 `pumpAndSettle` 后点选（ widgets `find.text(...).last` 消歧义）。
- D3 Rust 生产零改动：`ClashApiService` 真实路径已存在且被新测钉住；空 id
  守卫放在 controller（ViewModel 等价层，与上游 `canEditRemove` 同位），传输
  层不引入误导性错误变体。

## 5. 未完成项（implemented 非 verified 的原因）

1. 真实核心端到端（live sing-box/mihomo 下列表/关闭/重开）未验：需已授权
   隔离环境 + 实际核心，由整合者安排。
2. FRB actual 身份直通仍缺（SP-17 遗留）：`target_profile_id` /
   `actual_generation` 等仅在 `ipc_contract::stable` 与 engine 内存，未经
   `bridge_api SnapshotDto` 暴露；本卡关闭绑定的是 monitor 会话 generation
   （D25 迟到驱逐），DTO 落地后直接接线，无需改 UI 结构。
3. 完整 AGENTS 门禁留 SP-34。
