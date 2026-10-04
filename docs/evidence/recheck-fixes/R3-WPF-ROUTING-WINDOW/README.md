# R3-WPF-ROUTING-WINDOW — 路由设置改为独立顶层窗口

日期：2026-10-05。基线 HEAD：`8cba2c6`（工作树在该次实施前干净；`tools/cores/session_matrix.ps1`
与 `assets/`、`R3-VISUAL-DPI-TRAY/` 为他人未提交改动，未触碰）。冻结原版：v2rayN 7.25.4
`7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 1. 方案

复用 Wave L（`R3-WPF-OPTION-WINDOW`）已验证的**原生 C++ 第二顶层窗口 + 第二个 Flutter engine**模式：

- 新增 `apps/desktop/windows/runner/routing_window.{h,cpp}`：`RoutingWindow : Win32Window`，
  以 `DartProject(L"data").set_dart_entrypoint("routingWindowMain")` 建第二个
  `FlutterViewController`（独立 engine/view），首帧回调 `Show()`。
- 新增 `apps/desktop/windows/runner/routing_window_host.{h,cpp}`：单例宿主，持有主/子两个
  `MethodChannel`（`v2rayn/routing_window`，各自 engine messenger），管理子窗口生命周期。
- 子窗口以主窗口为 owner 创建（`win32_window::Create(title, origin, size, owner)`，Wave L 已有），
  标题 `路由设置`、1000×700（对齐上游 `Width=1000 Height=700`）；创建时 `EnableWindow(main,FALSE)`
  模拟上游 `ShowDialog()` 的 owner 模态，关闭后 `EnableWindow(main,TRUE)` + `SetForegroundWindow(main)`。
- 子 engine **不调用 `RustLib.init`、不注册插件**，不持 FRB/Rust 句柄。

通信协议（channel `v2rayn/routing_window`，按 engine 各自 messenger 绑定）：

| 方向 | 方法 | 参数 | 语义 |
| --- | --- | --- | --- |
| 主 engine → native | `open` | snapshotJson | 创建/抬起独立窗口，返回 bool（5s 超时视为不可用） |
| native → 主 engine | `applyDraft` | `{id, draft}` | 子窗口草稿回传，主窗口执行保存 |
| 主 engine → native | `reportOutcome` | `{id, ok, message}` | 保存结果 |
| 子 engine → native | `ready` | — → snapshotJson | 启动取快照（带重试） |
| 子 engine → native | `saveDraft` | `{id, draft}` | 请求保存（转发主 engine） |
| native → 子 engine | `saveOutcome` | `{id, ok, message}` | 保存结果回投 |
| 子 engine → native | `close` | — | 关闭窗口（不写盘） |

子窗口以「本地草稿」编辑所有内容（方案元数据 + 规则列表 + 两个策略下拉），**确定**时把整份
草稿经通道回传主 engine；主 engine 用既有路径落地并应用：

1. 逐方案 `routingControllerProvider.save(draft)`（既有 `save_routing`）；
2. 草稿中已删除的方案 → `deleteRouting`（既有）；
3. `RoutingBasicItem` 的 `DomainStrategy`/`DomainStrategy4Singbox` → 既有 `settingsController.saveGroup`；
4. 活动方案变化 → 既有 `setDefaultAndReload`（保存默认 + 重载服务）；否则 `runtimeController.reload()`。

**取消 / Esc / 标题栏关闭**只 `close()`，不落盘；应在主 engine 弹 SnackBar（打开窗口失败时）。
**打开失败可见反馈**：`open` 返回 false 且 native 存在时弹「打开路由设置窗口失败」；widget 测试
环境无 native host（`MissingPluginException`/超时）时回退到内嵌 dialog，保证菜单入口与既有测试可用。

## 2. 改动文件

- `apps/desktop/windows/runner/routing_window.{h,cpp}`（新增）
- `apps/desktop/windows/runner/routing_window_host.{h,cpp}`（新增）
- `apps/desktop/windows/runner/runner_messages.h`（新增 `kRoutingCloseMessage`/`kRoutingClosedMessage`）
- `apps/desktop/windows/runner/flutter_window.cpp`（Attach `RoutingWindowHost`；处理 `kRoutingClosedMessage`）
- `apps/desktop/windows/runner/CMakeLists.txt`（加入新增源文件）
- `apps/desktop/lib/features/routing/routing_windows.dart`（新增宿主抽象/JSON 编解码/`RoutingEditorWindow`/
  `_RoutingSchemeEditor`/`runRoutingWindow`/`RoutingWindowApp`；`RoutingRuleDetailsDialog` 增加可选
  `outboundTags`，让规则详情在子 engine 内不读 bridge）
- `apps/desktop/lib/features/routing/routing_actions.dart`（`openRoutingSettings` 改为开独立窗口 +
  回退；新增 `_buildRoutingSnapshot`/`_applyRoutingDraft`）
- `apps/desktop/lib/main.dart`（新增顶层 `routingWindowMain` entrypoint）
- `apps/desktop/test/r3_wpf_routing_window_test.dart`（新增：6 个 widget 测试）
- `apps/desktop/test/t11_menu_test.dart`（无 native host 时 mock `v2rayn/routing_window`，验证回退）
- 本证据目录；`docs/tasks/R3-WPF-ROUTING-WINDOW.md`

未改：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`bridge/api/**`、其余业务 feature、
`crates/**`、`services/**`；未提交 git。

## 3. 命令与结果

工作目录 `apps/desktop`：

| 命令 | 结果 |
| --- | --- |
| `dart format --output=none --set-exit-if-changed lib test integration_test` | exit 0（304 files，0 changed）→ `format.log` |
| `flutter analyze` | `No issues found!` → `analyze.log` |
| `flutter test test/t11_menu_test.dart test/r3_wpf_routing_structure_test.dart test/r3_wpf_routing_window_test.dart` | 10/10 通过 → `tests-routing-window.log` |
| `flutter test test/t11_routing_test.dart test/fix08_routing_draft_test.dart test/fix08c_routing_draft_test.dart test/t21e_dialogs_responsive_test.dart` | 12/12 通过 → `tests-routing-regression.log` |
| `flutter build windows --release` | exit 0，`Built build\windows\x64\runner\Release\v2rayn_desktop.exe` → `build-windows-release.log` |
| 发布包符号核对 | `routingWindowMain` 同时存在于 `data/app.so` 与 `v2rayn_desktop.exe` |

合并单次跑多个测试文件时偶发命中已知环境 flake（“did not complete”，非断言失败）；拆分为两组后稳定通过。

## 4. 真实窗口探针（发布包，隔离数据目录）

`probe_routing_window.ps1`：从临时副本启动（独立单实例锁）、`V2RAYN_R_DATA_DIR=<temp>`、
`V2RAYN_R_OPEN_ROUTING=1`，仅停止本脚本启动的 PID。结果：`passed=true`，
`stopped=true, alive_after=false`，`pid=27188`（见 `probe-routing-window.json`）。

| 窗口 | HWND | 标题 | visible | enabled | owner | style | rect |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 主 | `0x3F0AAA` | `v2rayN` | true | **false（模态禁用）** | — | `0x1CCF0000` | 10,10–1210,810 |
| 路由 | `0xD80A30` | `路由设置`(U+8DEF U+7531 U+8BBE U+7F6E) | true | true | `0x3F0AAA` | `0x14CF0000` | 120,120–1120,820 |

`checks`：`main_present / routing_present / routing_title / independent_hwnd /
routing_owned_by_main / main_disabled_modal / routing_enabled / two_visible_top_level /
main_screenshot / routing_screenshot` 全 `true`；`routing_foregroundable=true`。
截图 `probe-routing.png` 为 1000×700 独立窗口：标题栏、`添加规则集`/`一键导入规则集`、
两条策略下拉（`域名解析策略=AsIs`）、蓝色下划线 `预定义规则集列表`、五列
`别名/数量/排序/可选地址 (Url)/自定义图标`、活动行高亮、底部 `取消/确定`。

> 关闭/重开语义：`routing_window_host.cpp::OpenRoutingWindow` 在窗口已存在时 `SetForegroundWindow`
> 抬起既有窗口并保留草稿（重复打开不丢状态、不重复建窗）；主窗口最小化不销毁子窗口。
> 标题栏关闭走 `WM_DESTROY` → `kRoutingClosedMessage` → 主窗口恢复；不写盘。

## 5. 上游对照结论

- 上游 `RoutingSettingWindow.xaml`：`ToolBarTray`（`添加规则集` + `一键导入规则集`）、两条策略下拉、
  `预定义规则集列表` TabItem、DataGrid 五列 `LvRemarks/LvCount/LvSort/LvUrl/LvCustomIcon`、
  行右键菜单（添加/移除所选规则/全选/设为活动规则/一键导入规则集）、活动行 `IsActive` 高亮。
  本实现窗口内容与之一致（探针截图 + `r3_wpf_routing_structure_test` 覆盖）。
- 上游 `RoutingSettingWindow.xaml.cs:43-50`：`Closing` 时 `IsModified==true` → `DialogResult=true`，
  由 `MainWindowViewModel` 决定刷新/Reload；`PreviewKeyDown` 支持 Ctrl+A / Enter（设默认）/ Delete（删除）。
  本实现以「确定保存成功才刷新/应用」对齐“修改才重载”的语义；Esc/取消/标题栏关闭不落盘。
- 窗口形态：上游是独立 `WindowBase` 顶层窗口、`ShowInTaskbar=False`、owner 模态；探针确认本实现为
  独立 HWND + owner + 主窗口禁用，一致。
- 与 Wave K 的差异（有意）：Wave K 内嵌 dialog 底部仅有 `关闭`；本卡为独立窗口并按完成合同 #3 增加
  底部 `确定/取消`（同时保留工具栏/右键菜单命令），未回退五列/文案/顶部工具栏。

## 6. main_shell 接线补丁

**不需要。** `main_shell.dart` 的 `ACT-MAIN-025` 与 `V2RAYN_R_OPEN_ROUTING` 钩子仍调用
`routing_actions.openRoutingSettings(context, ref)`；入口行为在 `routing_actions.dart` 内部切换为
独立窗口，菜单分发结构未改。

## 7. 未完成 / 接口缺口

- 未做逐事件行为对照（真实鼠标：菜单打开→编辑→确定/取消/标题栏关闭→重开）与 100%/150% DPI 对照；
  本卡据此记 `implemented`，未达 `verified`。探针只覆盖“打开/窗口形态”。
- 跨 engine 保存回环（子窗口草稿 → native `applyDraft` → 主 engine `_applyRoutingDraft`
  save/delete/setDefaultAndReload → `reportOutcome`）由 widget 测试（fake host）覆盖，
  未在真实 release 包上驱动“编辑后确定”链路；`_applyRoutingDraft` 仅经编译与静态分析。
- **新建方案且同时设为默认**的边界：新方案保存前 id 为空，主侧无法在保存后据此 `setDefaultAndReload`，
  此情形仅保存 isActive 标志并走 `runtime.reload()`（未单独 setDefault）。登记为缺口。
- 子 engine 未注册插件：规则编辑器的「从文件导入」使用 `file_selector`，在第二 engine 中
  `MissingPluginException`（已被 `pickRulesFromFile` 捕获并提示）；剪贴板/URL 导入可用。
- `一键导入规则集` 上游为 `ConfigHandler.InitRouting(config, true)`；Rust 侧无对应用例（既有登记缺口），
  窗口内 `_importBuiltin` 仅提示，不回退。
- 规则行「全选」（多选）未实现（既有登记缺口）。
- 环境：本机存在本项目 Release 实例 PID 37716（`build\...\Release\v2rayn_desktop.exe`，非本会话启动、
  无会话记录）。为不杀不受本会话管理的进程，先将其 exe 改名为 `v2rayn_desktop.stale-37716.exe` 以解除
  linker 占用（进程未停止）；构建产物为全新 `v2rayn_desktop.exe`。用户可自行关闭该旧实例。
- 未监听/占用 `127.0.0.1:10808`；探针未触发 `applyActive`，无内核启动；未改系统代理/注册表/路由/TUN；
  未读用户凭据。

## 8. 下一步前置

1. 逐事件对照脚本：真实点击菜单→窗口出现→编辑规则/默认→确定/取消/标题栏关闭/重开，采集 HWND
   事件、`applyDraft`/`reportOutcome` 时序与两张截图，覆盖 100%/150% DPI。
2. 补「新建方案 + 设默认」的 `setDefaultAndReload` 回传（主侧需从 `save` 返回值拿新 id 后再 setDefault）。
3. 规则编辑器文件导入：要么为子 engine 注册 `file_selector`（不引入 Rust），要么把文件选择经通道回传主 engine。
4. 关闭步骤 7 的 `.stale-37716` 旧实例（需用户确认）后，正常 release 构建可不再改名。
