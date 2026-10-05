# R4-10 异步桥与慢 I/O — 证据

状态：implemented（Dart 契约与 Rust 单元测试通过；未在真实慢盘/GUI 上实测，故未 verified）。

HEAD/基线：`3426a1a`；应用基线 `77c74ed`；上游冻结 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；armed=false。未占用端口（`ports_used: []`），未碰 10808、宿主系统代理/注册表/路由/TUN/自启，未读取用户凭据，未终止任何非本项目进程。

## 本次唯一用户流程

备份/磁盘/数据库慢时仍可滚动窗口并看到反馈，且取消后不提交结果。

## 覆盖缺陷与方法

- D10：`api/t16.rs:112/184/271` 等本地备份/校验/恢复/导入/识别是同步 FRB，含文件复制、哈希、数据库生命周期。→ 移除这 6 个函数的 `#[frb(sync)]`，改异步 FRB；Dart seam 改 `Future`，`backup_controller` 走 `await` + busy + 取消代际。
- D25：`bridge_api/src/api/monitor.rs:1396-1399` 在 hub 共享锁内 `flush_store`（逐行 SQLite upsert）。→ 锁内只 `apply` + 克隆行，释放锁后经 `persist_rows` 落盘；落盘失败经 `stats_snapshot().error` 可读并在下一 tick 重试。
- D08：`bridge_port.dart` 全量同步读取 / `profiles_controller.dart` 150ms 重读。属 R4-09/R4-22 的摘要/分页范围，本卡不重复拥有；已登记接口缺口。

## 实现摘要

### Rust

- `crates/application/src/monitor.rs`
  - `StatsService.store` 由 `Box<dyn TrafficStore>` 改为 `Arc<Mutex<Box<dyn TrafficStore>>>`，store 自带互斥，磁盘 I/O 不再需要持有 monitor hub 锁。
  - 新增 `snapshot_rows()`（纯内存克隆）、`store_handle()`（可克隆句柄）。
  - 新增自由函数 `persist_rows(handle, rows)`：在 hub 锁之外顺序 upsert，返回错误供调用方登记；单 poller 调用，有界、无新线程。
  - `load/flush_store/clear_all/clear_node` 改为经 store 互斥锁访问。
  - 新增单测 `persist_rows_writes_through_store_handle_without_the_service`（drop 掉 service 后仍能经句柄落盘，证明不需要 hub/service 锁）；`clear_all_empties_memory_and_store` 适配锁访问。
- `crates/bridge_api/src/api/monitor.rs`
  - poll 循环：hub 锁内只 `apply` 并克隆 `(store_handle, rows)`，释放锁后 `persist_rows`；失败写 `store_error`（`ErrorDto::from`）。
- `crates/bridge_api/src/api/t16.rs`
  - `t16_backup_local/list/verify/restore/recognize/import_upstream` 移除 `#[frb(sync)]`（异步 FRB）。函数签名不变，未改 `frb_generated`。

### Dart

- `apps/desktop/lib/bridge/bridge_port.dart`
  - 6 个 `t16Backup*` 抽象方法返回 `Future<...>`；`FrbBridgePort` 用 `Future<T>.sync(() => t16.t16BackupX(...))`（regen 前后都可编译：regen 前包裹同步返回值，regen 后展平异步 Future）；`SyntheticBridgePort` 相应改为 `async`。
- `apps/desktop/lib/features/backup/backup_controller.dart`
  - `reloadBundles/localBackup/recognize` 改 `Future<void>` 并 `await`；本地备份/恢复/导入加 `busy` 与代际 `_generation`；新增 `cancel()`，取消后到达的旧结果按代际丢弃，不提交、不假报成功。
- 测试：新增 `test/r4_10_contract_test.dart`（4 项）；更新 3 个备份测试的桥覆写为 `Future`。

## 必过场景覆盖

| 场景 | 覆盖 |
|---|---|
| I/O/DB 批处理/哈希/IPC 移出 UI | t16 备份/校验/识别/恢复/导入改异步 FRB；monitor 落盘移出 hub 锁（rust 单测） |
| sync 仅保留极短内存方法 | 保留的 t16 sync 仅 cleanup/open/versions；profiles 大读仍 sync，登记为 R4-09/R4-22 缺口 |
| monitor 磁盘 flush 移出共享读锁 | poll 锁外 `persist_rows`；`persist_rows_writes_through_store_handle_without_the_service` |
| 慢任务中帧可渲染、取消可用 | widget 测试：gated 慢桥下滚动可用、取消后代际丢弃结果 |
| 有界 worker、失败可读可重试 | 单 poller 顺序落盘，无新增线程/任务；失败写 `store_error`，错误后可重试（widget 测试 4） |
| FRB 二次生成 no-diff | 待整合者重生成（见下） |

## 实际命令与结果

- `flutter analyze`（apps/desktop）：No issues found!
- `flutter test test/r4_10_contract_test.dart`：All tests passed（4）
- `flutter test test/fix14b_backup_restore_test.dart test/sr02_04_backup_import_test.dart test/sr03_restore_lifecycle_test.dart test/fix14_backup_activate_test.dart test/fix14c_webdav_backup_test.dart test/t16_backup_test.dart test/r3_set_restore_test.dart`：All tests passed（24）
- `dart format --output=none --set-exit-if-changed <改动文件>`：0 changed
- `cargo fmt --all -- --check`：clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：Finished，无警告
- `cargo test --workspace --locked`：全部 test result: ok（EXIT=0，含新增 monitor 单测）
- `flutter build windows --release`：Built `build\windows\x64\runner\Release\v2rayn_desktop.exe`

## FRB 重生成需求

本卡将 6 个 `t16_backup_*` 源函数由 `#[frb(sync)]` 改为异步。按卡约定未改任何生成文件。整合者需在其唯一写入窗口对 `crates/bridge_api` 运行一次 `flutter_rust_bridge_codegen generate`，生成物将更新：`crates/bridge_api/src/frb_generated.rs`、`apps/desktop/lib/bridge/frb_generated.dart|.io.dart|.web.dart`、`apps/desktop/lib/bridge/api/t16.dart`（6 个函数变为返回 `Future<...>`）。Dart seam 已按“regen 后返回 Future”写好，重生成后无需再改业务代码；再次生成应为 no-diff。重生成前运行时仍是同步调用（生成物未变），重生成后才真正把磁盘/哈希工作移出 UI isolate。

## 未完成 / 未验证

- 真实慢盘/大量 ServerStatItem 下的 GUI 滚动与取消端到端未实测（本轮为合成 widget 夹具）。
- 未在真实 SQLite/文件系统上执行一次受门禁的慢备份延迟测量。
- profiles 摘要/分页大读仍为同步（D08 的读路径），由 R4-09/R4-22 拥有。
- monitor 首次绑定 `monitor_start_polling`→`rebind_store`→`stats.load()` 全表读取仍在 hub 锁内、且该 FRB 为 sync；由 R4-23/R4-10 后续处理。

## 接口缺口（登记，不自行削减）

- FRB 重生成：提供方 = bridge_api 源；调用方 = frb_generated 与 `lib/bridge/api/*`；输入/输出 = 6 个 `t16_backup_*` 的 async 签名；生效点 = 生成后运行时异步。整合者统一处理。
- 监控落盘错误目前复用 `stats_snapshot().error`（`store_error`），未区分“绑定失败 / 落盘失败”码；由整合者决定是否需要独立字段。
- D08 大读异步与 R4-09 分页接口由整合者固定唯一写入者。

## 环境与清理记录

- Windows PowerShell 7；Flutter 3.47.5；Rust 1.98.1；测试/构建未监听任何端口（≥11808 也未使用）。
- `Get-Process v2rayn_desktop,net_host`：仅 `net_host` PID 32340，路径 `dist\v2rayN-R-1.0.0+1-windows-x..`，与本轮 `build\` 输出无关，未启动、未终止。构建前无 `v2rayn_desktop` 实例锁。
- 期间工作树内有并发代理（R4-03）未提交的 `crates/runtime/src/adapter.rs`、`services/net_host/src/session.rs`、`features/update/*` 与 `test/r4_03_contract_test.dart`；工作区级门禁失败一度由其跨 crate 影响引起，待其收敛后全门禁通过（见命令日志）。未修改其文件。
