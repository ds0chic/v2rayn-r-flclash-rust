# SP-20 连接表列和右键 — 准备范围证据（非完整卡完成）

状态：**identified**（本次只做可独立部分；完整验收依赖 SP-17/A08 在途，不伪造完成）。
基线：`788adf6`。不 commit。`work/`、`outputs/` 只读未动；`compat/` 未动；
engine.rs、lib.rs、runtime_bridge/runtime_controller、IPC/stable DTO、BridgePort、
FRB、Cargo 锁均未动。合成数据，不动宿主网络（无 socket/HTTP/代理/10808 操作）。

唯一用户流程（完整卡）：用户排列连接列、右键关闭后重开，布局保持且真实连接管理有效。
本次准备范围：原版列/右键/会话 generation 对照、合成连接数据夹具、列布局持久化
合同与实现（纯函数层；未接入正式 `ConnectionsView`/FRB/运行时）。

## 0. 原版对照（v2rayN 7.25.4 / 7d6a967，只读 `work/`）

- 默认列/宽（`v2rayN/Views/ClashConnectionsView.xaml`，`ExName` 为稳定列键）：
  Host=300 / Chain=500 / Network=80 / Type=160 / ProcessPath=100 / Elapsed=100。
- `ClashConnectionsView.xaml.cs`：`RestoreUI` 按
  `ClashUIItem.ConnectionsColumnItem.OrderBy(Index)` 恢复（仅 `Width > 0` 覆盖）；
  `StorageUI` 在退出时按 Name/ActualWidth/DisplayIndex 回写（FLD-CFG-136 系列）。
- `ServiceLib/ViewModels/ClashConnectionsViewModel.cs`：右键关闭冻结
  `SelectedSource.Id`（空 Id 不可执行，`canEditRemove`），关闭全部走独立空 id
  路径（`ClashConnectionClose(all: true)` → `CloseConnection("")`），关闭后重查；
  刷新门控：`AutoRefresh && ShowInTaskbar && IsRunningCore(sing_box)` 且
  `ConnectionsRefreshInterval > 0` 按 tick 取模。
- 缺口登记：真实连接管理的 endpoint+generation 绑定与 FRB/运行时接线需
  SP-17（A08 独占 runtime_bridge/runtime_controller），本次未实现，只保留
  请求冻结/过期丢弃纯合同（见 §2 等待接口）。

## 1. 改动文件（写锁内）

- `apps/desktop/lib/features/monitor/connections_columns.dart`（新增）：
  `defaultConnectionColumns`（上游默认宽/序）、`resolveVisibleColumns`（按 Index
  排序、丢未知列、Width<=0 回退默认、缺列按上游顺序补齐、重编 0..n-1）、
  `connectionColumnsToStorage`（Name/Width/Index 回写形状，供调用方写入
  `ClashUIItem.ConnectionsColumnItem` 后独立重开读回）、`freezeConnectionClose`/
  `closeResponseIsCurrent`/`isCloseAllRequest`（关闭目标冻结与过期丢弃）、
  `syntheticConnections`（纯内存 `*.example.invalid` 合成行，无网络）。
- `crates/application/src/monitor.rs`（同文件追加 SP-20 prep 节 + 4 个单测）：
  `DEFAULT_CONNECTION_COLUMNS`/`default_connection_columns`/
  `normalize_connection_columns`（与 Dart 同语义，`domain::ColumnDefinition` 行）、
  `ConnectionCloseRequest`/`freeze_close_request`/`close_response_is_current`。
  未动 engine.rs/lib.rs/DTO/FRB。
- `apps/desktop/test/repair/sp_20_connections_prep_test.dart`（新增，9 用例）。
- 本证据目录（`docs/evidence/stable-port/SP-20/`）。

## 2. 先红后修

- 红：新测试在实现前运行，`connections_columns.dart` 缺失导致编译失败，
  `flutter test test/repair/sp_20_connections_prep_test.dart` exit **1**
 （见 `commands.log` R1）。
- 绿：实现后同命令 9/9 通过；受影响既有 monitor 测试
 （fix16c_clash_ui_config、fix11_monitor_session、fix11b_clash_panel、
  fix11c_logs）同批 32/32 通过（fix11b 首次批跑 2 项 `did not complete`，
  无断言失败，重跑即全绿；本文件未动其代码路径，见 `commands.log` R5-R6）。

## 3. 定向检查（实际运行）

- `cargo fmt -p application -- --check`：exit **0**（仅本包；改动前一次格式化，
  后复核 0）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：exit **0**
  （本次改动在场时通过；之后并行他卡在途改动致编译阻塞，见下）。
- `cargo test -p application --locked --lib`：本次改动在场时 **315 passed / 0 failed**；
  当前重跑被并行在途改动阻塞（`crates/application/src/dns.rs:664` 花括号 mismatch、
  `crates/config_codegen/src/xray/dns.rs:225` 类型错，均在他卡写锁内，本卡未动，
  不代修）。替代验证：把 `monitor.rs` 内 SP-20 函数/测试体逐字抽出（仅
  `domain::ColumnDefinition`→同形本地 stub 机械替换）到
  `TEMP/opencode/sp20_monitor_prep.rs`，`rustc --edition 2021 --test` 编译 0 警告，
  4/4 通过。workspace 级重跑待他卡修复后由整合者汇总联动检查。
- `dart format --set-exit-if-changed`（本卡两文件）：exit **0**。
- `flutter analyze`：本卡文件零 issue；全应用 7 条 info 均在他卡文件
 （`status_bar_view.dart` 等），本卡未动。
- `flutter test test/repair/sp_20_*`：9/9 通过；合成 1k/10k 行唯一 id、全
  `.example.invalid` host；无端口、无 socket（测试端口规则不适用，无网络调用）。
- 未运行：`flutter build windows --release`（本次无 native/AOT 接线变更）、
  AGENTS 完整门禁（发布候选才跑；留给 SP-34）。

## 4. 未完成项与等待接口（完整卡仍 identified）

1. `ConnectionsView` 正式接入：列宽/列序应用与拖拽回写、右键菜单
   （关闭/关闭全部）绑定冻结 id、重开恢复——待 SP-17 会话 generation 落定后接。
2. 真实连接管理：close 绑定 endpoint+generation、关闭失败/换 session 迟到不误关
   的端到端验证——提供方 SP-17（A08：runtime_bridge/runtime_controller）。
3. 持久化通道已存在（`domain::ClashUiItem.connections_column_item` ↔
   `persistence::clash_columns` ↔ `ClashUiItemDto`），本卡回写形状与其兼容，
   接线由 SP-00 整合者确认。
4. Rust workspace 重跑：待并行 `dns.rs`（application）与 `xray/dns.rs`
   （config_codegen）在途改动修复编译后，整合者汇总重跑受影响包。
