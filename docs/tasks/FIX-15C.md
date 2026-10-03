# FIX-15C — 自启提交时机与系统代理/PAC 数据源补齐

状态：`implemented`（Rust 单 crate fmt/clippy/test 与 Flutter 针对性测试通过；真机 HKCU Run 写读删已复原且无残留，真机系统代理 ForcedChange 写读复原前后一致；四模式/PAC 未在真机逐一执行，故不写 `verified`）。

任务 ID：FIX-15C

来源：`docs/tasks/FIX-15.md` 登记卡（`docs/evidence/parity-review-2026-10-03/repair-queue.md:40`“自启/PAC另卡”）。前置：FIX-15 已提交的托盘/热键/启动隐藏/单实例语义不改。开始应用 HEAD `a940c21`（工作树干净）。

本次唯一用户流程：在设置窗口保存“开机自启(AutoRun)”后，系统 Run 项按上游时机写入/清除；切换到 `Pac` 模式时读取完整 `data/pac.txt`（或自定义 PAC 文件）并交给本地 PAC 服务。两者都只在规定提交点改动系统，且真机测试后逐项复原。

对应 feature / field / action / layout ID：`FLD-CFG-056`（GuiItem.AutoRun）、`FLD-CFG-088..092`（SystemProxyItem）、`ENUM-004`（四模式）、`ACT-TRAY-00x`。

必读上游文件、符号和固定 commit（只读）：
- `v2rayN/ServiceLib/Handler/AutoStartupHandler.cs:9-40/45-78`（`UpdateTask`：先 `ClearTaskWindows` 再按 `AutoRun` `SetTaskWindows`；非管理员写 `exePath.AppendQuotes()`）。
- `v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:400-402`（`SaveConfig` 成功后才 `AutoStartupHandler.UpdateTask`）——自启提交时机。
- `v2rayN/ServiceLib/Handler/SysProxy/SysProxyHandler.cs:8-68/86-117`（`UpdateSysProxy` 四模式；`Pac` 时启动 `PacManager`）。
- `v2rayN/ServiceLib/Manager/PacManager.cs:32-59`（自定义 PAC 存在则用之，否则 `config_dir/pac.txt`；缺失时写入内嵌 `Sample/pac`；再替换 `__PROXY__`）。
- `v2rayN/ServiceLib/ViewModels/StatusBarViewModel.cs:363-378`（托盘切换模式：先改 `SysProxyType` → 应用 → `SaveConfig`）——模式提交边界。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`GuiItem.AutoRun`、`SystemProxyItem.CustomSystemProxyPacPath`、`config_dir`（app 数据目录）。
- 输出：Run 项 `"<exe>"` 写入/删除；PAC 脚本文件路径与文本（仍含 `__PROXY__`）。
- 错误：Run 写失败或 PAC 读失败返回结构化 `ErrorDto`（`E_PLATFORM_BACKEND`/`E_IO`），不伪造成功。
- 取消：设置窗口取消不写 Run 项；切换开关本身不写。
- 权限：仅本机进程；真机写只在显式用户动作且授权路径下。
- 持久化：`GuiItem.AutoRun`、`SystemProxyItem.SysProxyType` 随设置文档落库。
- 生效：Run 项在设置保存成功后生效；PAC 在 `Pac` 模式起 PAC 服务时生效，端口 ≥ 11808。

允许修改的模块（本卡实测）：`crates/platform/**`、`crates/bridge_api/src/api/platform.rs`、`apps/desktop/lib/features/settings/{platform_controller.dart,platform_bridge.dart,proxy_settings_view.dart,settings_actions.dart}`、`apps/desktop/test/**`、`compat/features.yaml`（仅追加）、`docs/evidence/UX-PARITY-FIX-15C/**`、本卡。

禁止改变：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/settings/{option_setting_window.dart,settings_controller.dart,settings_fields.dart,settings_defaults.dart,hotkeys.dart,global_hotkey_window.dart}`、`features/profiles|subs|runtime|monitor|update|backup|routing/**`、`crates/application/src/engine.rs`、`crates/updater/**`。不删入口、不降分母、不改 FIX-15/15B 语义。

本次必须通过的命令/真实场景与实际结果：
- `cargo fmt -p platform -p bridge_api -- --check` → 0；`cargo clippy -p platform -p bridge_api --all-targets --locked -- -D warnings` → 0。
- `cargo test -p platform -p bridge_api --locked` → 全绿（新增 `pac::tests` 5/5、`bridge_api::pac_resolve_script_seeds_and_reads_default` 1/1）。
- 真机 `cargo test -p platform --test real_windows -- --ignored --nocapture`：Run 项写读删无残留；系统代理 ForcedChange 应用/复原前后一致。
- `dart format --output=none --set-exit-if-changed <changed>` → 0 changed；`flutter analyze lib test` → No issues。
- `flutter test test/fix15c_pac_resolve_test.dart` → 8/8；`test/fix15c_autostart_timing_test.dart` → 3/3。
- 回归：`fix15_tray_pac_test` 8/8、`fix08_option_cancel_test` 1/1、`fix08_option_apply_test` 1/1、`t13_platform_models_test` 10/10、`t13_statusbar_test` 2/2（exit 79 重试后）、`fix15b_startup_test` 3/3。
- 未运行：`flutter build windows --release`、全量 `flutter test`、全量 Rust workspace 门禁。

证据文件位置：`docs/evidence/UX-PARITY-FIX-15C/`（`README.md`、`observations.json`、`real-pre-state.json`、`real-post-state.json`、`real-autostart.log`、`real-sysproxy.log`）。

完成条件：自启只在设置保存成功后且值变化时写 Run 项，真机写读删可逆；PAC 读取 `data/pac.txt`/自定义脚本并缺失时回退内置模板；四模式/PAC 状态机与退出 ownership 恢复由既有测试覆盖；门禁通过。四模式未在真机逐一执行，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`crates/bridge_api` 新增的 `pac_resolve_script` 因 `lib/bridge/api/**` 与两处 `frb_generated` 属禁改文件、本卡未重生成 FRB，暂不可被 Dart 调用；App 目前走 Dart 侧 `selectPacFile`/`startPacFromConfig`，`data/pac.txt` 缺失时以最小模板播种。待 FRB 重生成后应改为调用 Rust `pac_resolve_script` 以使用完整上游 `Sample/pac` 模板。
- 接口缺口（登记）：`desktop_integration.dart`/`status_bar_view.dart` 的 PAC 调用点不在本卡允许修改清单内，尚未切到 `startPacFromConfig`；已具备可调用入口，接线属后续卡。
- 接口缺口（登记）：UP 的提权计划任务路径（`AutoStartTaskService`）仍未实现，仅非提权 Run 项路径。
- 真机只验证了 `ForcedChange` 的写读复原；`ForcedClear`/`Unchanged`/`Pac` 在真机尚未逐一执行（`Pac` 会绑定端口），四模式状态机由内存 backend 覆盖。

## 本轮实际结果

- `crates/platform/src/pac.rs`：新增 `DEFAULT_PAC_TEMPLATE`（内嵌上游 `Sample/pac`）、`resolve_pac_path`、`resolve_pac_script`/`ResolvedPac`（缺失时播种默认并读取），含 5 个隔离临时目录测试。
- `crates/platform/src/lib.rs`：导出上述符号。
- `crates/bridge_api/src/api/platform.rs`：新增 `PacScriptDto` 与 `pac_resolve_script`（frb sync）及 1 个测试。
- `apps/desktop/lib/features/settings/proxy_settings_view.dart`：新增 `defaultPacScriptTemplate`、`PacFileSelection`、`selectPacFile`（上游 `PacManager.InitText` 选择规则）。
- `apps/desktop/lib/features/settings/platform_controller.dart`：新增 `startPacFromConfig`（解析/播种 pac.txt 后复用 `pacStartFromFile`，失败回结构化错误）。
- `apps/desktop/test/support/fake_platform_bridge.dart`：记录 `pacStartFromFile` 路径与文件文本。
- `apps/desktop/test/fix15c_pac_resolve_test.dart`（8）、`apps/desktop/test/fix15c_autostart_timing_test.dart`（3）。
- `crates/platform/src/assets/pac.txt`：自上游 `Sample/pac`（192429 字节）复制，仅作默认模板。
