# T13 接线决策记录（系统代理 / 托盘 / 热键 / 自启 / 退出恢复）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §13（系统代理恢复逐字段所有权/冲突检测）、
  §15（自定义脚本保留）、§08（关闭/托盘语义）。
- 关联平台决策：`docs/decisions/T13-platform.md`（D1..D6，四语义与所有权原语）。
- 关联台账：`compat/features.yaml` F-SYSPROXY-001..004、F-DESKTOP-001..007；
  `compat/actions.yaml` tray 15 条、hotkey 9 条、main-menu 剩余项；
  `compat/layouts.yaml` LAY-STATUSBAR-001、LAY-MAIN-004。
- 上游只读来源：`ServiceLib/Handler/SysProxy/*`、`ServiceLib/Manager/{PacManager,AppManager}.cs`、
  `ServiceLib/Handler/AutoStartupHandler.cs`、`ServiceLib/ViewModels/StatusBarViewModel.cs`、
  `ServiceLib/ViewModels/GlobalHotkeySettingViewModel.cs`、`v2rayN/Manager/HotkeyManager.cs`、
  `v2rayN/Views/StatusBarView.xaml(.cs)`、`ServiceLib/Common/ProcUtils.cs`。

## 接线 D1 —— 平台编排只经 trait，真实后端显式启用

新增 `crates/application/src/platform_service.rs`：`PlatformService` 持有
`Arc<dyn SystemProxyBackend>` 与 `Arc<dyn AutoStartBackend>`，所有写操作都经 trait。
生产接线在 release 启动时调用 `init_platform_backend("windows")` 切换到真实 WinINET/Run 键后端；
**默认（测试、证据、`cargo test`、widget test）始终使用内存 fake**。这样任何自动化路径都不可能
修改宿主系统代理或注册表（AGENTS.md 硬约束）。

真实后端本身在 `crates/platform` 中仅编译；写入只发生在用户显式点击时，不来自环境变量。

## 接线 D2 —— 逐字段所有权账本跨多次应用保留“会话原点”

`apply_proxy` 的账本合并规则：

- 应用前先用 `restore_if_owned` 对照当前状态分类已有条目：被用户改过的字段从账本移除（不再属于本应用）；
  仍是本应用值的字段**保留其最初的 `before`**（真正的会话原点）。
- 新写入的字段以 `(最早的 before, 最新的 after)` 合并进账本。

修正了初版“每次应用都重置 before”导致的退出恢复无法回到会话原点的问题。
退出恢复 (`restore_on_exit`) 统一走逐字段 `restore_proxy`，**不再无条件 force-clear**，
从而满足方案 §13“用户或其他软件改过的字段不能被无条件覆盖”。上游 `forceDisable`
的无条件清除语义被逐字段所有权收紧（记录在本决策）。

## 接线 D3 —— PAC 服务端口严格 ≥11808，生命周期幂等

`pac_start` 拒绝低于 `DEFAULT_PAC_PORT_BASE`(11808) 的显式端口；`port=0` 自动选空闲端口。
运行中重复 `pac_start` 只刷新内容、保持监听端口（对齐上游 `PacManager.StartAsync`）。
`pac_stop` 幂等。桥接层 `pac_start`/`pac_start_from_file`/`pac_stop`/`pac_state` 直接映射。

## 接线 D4 —— 桥接 API 与 DTO

`crates/bridge_api/src/api/platform.rs` 暴露：

- `set_system_proxy(mode,server,bypass,auto_config_url)`、`get_system_proxy_state(desired_mode)`；
- `restore_system_proxy()`、`restore_system_proxy_on_exit(desired_mode)`；
- `pac_start`/`pac_start_from_file`/`pac_stop`/`pac_state`；
- `get_autostart`/`set_autostart`/`autostart_value_name`；
- `validate_custom_proxy_script`（仅存在性校验，**从不执行脚本**）；
- `resolve_uwp_loopback_tool`（仅解析 + 校验 `bin/EnableLoopback.exe`，启动由桌面运行时在用户确认后执行）；
- 辅助：`hotkey_proxy_mode`、`hotkey_list`、`sysproxy_mode_value`、`sysproxy_type_valid`、
  `local_port_for_protocol`、`pac_port_base`、`init_platform_backend`。

导出 `applied`（所有权凭证）与 `conflicts`（外部修改冲突），状态栏据此显示，不伪造成功。

## 接线 D5 —— 托盘/热键插件选型

- 托盘：`system_tray 0.1.1`（MethodChannel，已稳定），而非 `tray_manager 0.7.0`——
  后者依赖仍在演进的 `nativeapi` 低层 API，会给 release 构建引入额外 FFI 风险。
- 热键：`hotkey_manager 0.2.3`；窗口：`window_manager 0.5.2`。
- 托盘菜单**顺序与开关状态**由 `lib/app/shell/tray_menu_model.dart` 纯模型表达（可单测），
  与 `StatusBarView.xaml` 的 `ContextMenu` 一一对应（ACT-TRAY-002..013）；节点子菜单受
  `TrayMenuServersLimit` 限制（超限隐藏，对齐上游 `BlServers=false`）。
- 热键注册经 `HotkeyRegistrar` 抽象注入；`PluginHotkeyRegistrar` 仅映射保守的虚拟键子集，
  未映射的键报告为冲突而非乱注册。`EGlobalHotkey` 1..4 映射到 `SysProxyType` 0..3（上游 `(int)e-1`）。

## 接线 D6 —— 桌面集成仅在真实运行时构建

`lib/app/shell/desktop_integration.dart` 只在 `V2rayNRApp` 的 bootstrap 中实例化（release 应用），
widget 测试从不构造它；其所有插件调用（tray/window/hotkey）因此只发生在真实运行、且由用户动作触发。
关闭按钮语义读 `UiItem.Hide2TrayWhenClose`（隐藏到托盘）与 `AutoHideStartup`；
退出走 `restoreOnExit` 后销毁窗口。

## 接线 D7 —— 测试隔离

- 共享 widget 测试 harness (`test/support/profiles_harness.dart`) 注入 `FakePlatformBridge`，
  否则状态栏读取平台控制器会触达未加载的 native 库（本轮修复的崩溃根因）。
- `test/support/fake_platform_bridge.dart` 是内存实现，记录 `appliedModes` 并可注入失败/冲突。
- 自动化测试不触碰真实系统代理/注册表/端口；PAC 测试绑定 ≥11808 回环端口。

## 接线 D8 —— Flutter 测试逐文件复现

`tools/flutter_test_retry.ps1` 新增 `-PerFile` 模式：每个 `*_test.dart` 独立进程 + 重试。
这是应对锁定 Flutter 3.47.5 `flutter_tester` 段错误的确定性方法（T01 已记录；T12a 亦采用）。
整包单进程运行会随用例数增长而近乎必然崩溃（约 95–99 用例处），与本轮改动无关（已验证 HEAD 基线同样崩溃）。
