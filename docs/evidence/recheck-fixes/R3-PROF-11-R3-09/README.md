# R3-PROF-11 + R3-09 修复证据

日期：2026-10-04。起始 HEAD `cd141f5`（工作树干净）。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（UP）。本卡只读生产源码/复核报告，未改 `work/`、`outputs/`；未 commit。所有测试为合成数据（RFC 5737 文档地址、假 UUID、临时 SQLite/内存 bridge），未占用 127.0.0.1:10808，未改系统代理/注册表/路由/TUN，未杀外部进程，未跑全量 workspace，未跑 flutter build windows。

## 改动范围

### R3-PROF-11（Reality 空指纹固化）

- `crates/application/src/custom.rs`：新增 `apply_reality_fingerprint_default(profile, def_fingerprint)`，仅当 `stream_security == "reality"` 且指纹为空时写入当时默认；新增 4 个单元测试。
- `crates/application/src/engine.rs`：新增私有 `default_reality_fingerprint()`（读 settings 的 `core_basic_item.def_fingerprint`，在 profile revisions 锁之外读取）；`save_profile` 与 `save_imported_profile` 的普通协议分支在 `normalize_server` 后应用固化。
- `crates/application/tests/r3_prof_11_reality_fingerprint.rs`：集成测试 4 项。

### R3-09（托盘图标 + 今日语义）

- `apps/desktop/lib/app/shell/tray_menu_model.dart`：新增 `trayIconResourceName`（四状态→四个 `.ico`）与 `trayIconFallbackName`。
- `apps/desktop/lib/app/shell/desktop_integration.dart`：新增 `_trayAssetsDir`/`_trayIconPathFor`，`iconPathFor` 改为按状态解析并回退；`_trayIconPath` 复用同一目录。
- `apps/desktop/lib/features/monitor/monitor_controller.dart`：`MonitorState` 新增 `todayUp`/`todayDown`/`hasTodayNodes` 与 `PlatformInt64`→`BigInt` 归一 helper。
- `apps/desktop/lib/app/shell/status_bar_view.dart`：状态栏「今日」改用节点 today 聚合；无节点行显示 `--`。
- `apps/desktop/test/support/fake_monitor_bridge.dart`：支持注入/发射节点 `NodeTrafficDto`。
- `apps/desktop/test/r3_09_tray_icon_today_test.dart`：新增 5 项。

未改：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/{subs,runtime,settings,update,backup,routing}/**`、`crates/application/src/{subs,backup_service,speedtest,groups,monitor}.rs`、`crates/{subscriptions,updater}/**`、`services/**`。未改 FIX-11/11B/11C 统计轮询/Clash/日志语义与 FIX-15 托盘委派结构。

## 实际命令与结果

Rust（工作目录仓库根，`C:\Users\Colby\.cargo\bin\cargo.exe`）：

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -- --check` | 干净（首次有 diff，`cargo fmt -p application` 后复检无输出） |
| `cargo test -p application --locked --lib custom::tests` | exit 0，**9 passed**（含新增 4 项） |
| `cargo test -p application --locked --test r3_prof_11_reality_fingerprint` | exit 0，**4 passed** |

Flutter（工作目录 `apps/desktop`，`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`）：

| 命令 | 结果 |
|---|---|
| `dart format`（改动文件） | 6 文件，1 changed（新测试），复检 0 changed |
| `flutter analyze` | `No issues found!` |
| `flutter test test/r3_09_tray_icon_today_test.dart` | exit 0，**5/5 passed** |
| `flutter test test/t15a_statusbar_test.dart test/t13_statusbar_test.dart test/recheck_rr08_tray_sync_test.dart test/fix15_tray_pac_test.dart test/reprof09_node_stats_test.dart` | exit 0，**24/24 passed** |

## 上游对照结论

- R3-PROF-11：UP `ConfigHandler.AddServerCommon:1205-1218` 在 `StreamSecurity` 为 reality 且 `Fingerprint.IsNullOrEmpty()` 时写入 `config.CoreBasicItem.DefFingerprint`。现实现时机与方法一致：`normalize_server` 负责 `AddServerCommon` 的 StreamSecurity/Network 尾部，随后 `apply_reality_fingerprint_default` 固化默认。CODEGEN 的运行时 fallback（`xray/outbound.rs:696-701`、`singbox/outbound.rs:843-852`）现在只在节点真的没有固化值时才触发。
- R3-09a：UP 托盘图标状态存在，本轮以 `trayIconResourceName` 显式区分四状态并解析资源；因仓库未打包 `.ico`，实际回退到 `app_icon.ico`（登记缺口）。
- R3-09b：UP `StatisticsManager` 单独维护 `TodayUp/TodayDown` 并跨日清零（`:94-95,156-161`），节点表列绑定 `ServerStatItem.TodayUp/TodayDown`。现实现从 `monitor.statsSnapshot().nodes` 聚合 today，与「今日」语义一致，不再用 session 累计冒充。

## 未完成 / 接口缺口

- R3-PROF-11：未做真实 Windows 窗口 + FRB + 真 SQLite 重开端到端；保持 `implemented`。CODEGEN 侧 `fingerprint` 为 `String`，无法区分显式空串与未设置——若配置默认本身为空串则固化不生效（上游默认为非空指纹）。其它保存入口（非 `save_profile`/`save_imported_profile`）未接线。
- R3-09a：仓库未打包任何 `assets/tray_*.ico`，`pubspec.yaml` 未声明 assets，四状态当前都回退到同一图标；真实托盘视觉区分需补四个图标资源与 assets 声明（不在本卡允许范围）。故「真实托盘视觉」未验证。
- R3-09b：`MonitorState` 未记录 `StatsSnapshotDto.enabled`；「禁用」以「无节点行→`--`」呈现，未区分「禁用」与「启用但尚无流量」的文案。

## 下一步前置

- 在隔离 Windows 窗口用合成 Reality 节点复跑「设默认 A→保存空指纹→改默认 B→重开/导出」，并补真 SQLite；若要覆盖空默认固化，需在 CODEGEN 引入显式哨兵。
- 补齐 `apps/desktop/assets/tray_*.ico` 与 `pubspec.yaml` assets 声明后，逐状态做真实托盘截图验收。
