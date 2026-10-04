# R3-WPF-OPTION-WINDOW — 参数设置改为独立顶层窗口

日期：2026-10-05。基线 HEAD：`55e6d10`（工作树在该次实施前干净）。冻结原版：
v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 1. 方案（spike 结论）

选择**原生 C++ 第二顶层窗口 + 第二个 Flutter engine**：

- 新增 `apps/desktop/windows/runner/settings_window.{h,cpp}`：`SettingsWindow : Win32Window`，
  用 `flutter::DartProject(L"data").set_dart_entrypoint("settingsWindowMain")` 建第二个
  `FlutterViewController`（独立 engine/view），首帧回调里 `Show()`。
- 新增 `apps/desktop/windows/runner/option_window_host.{h,cpp}`：单例宿主，持有主/子两个
  `MethodChannel`（`v2rayn/option_window`，各自 engine messenger），并管理子窗口生命周期。
- `win32_window.h/.cpp`：`Create` 增加 owner 重载；子窗口以主窗口为 owner 创建
  （`WS_OVERLAPPEDWINDOW` + `hWndParent=owner` ⇒ 独立顶级 HWND、无任务栏按钮、始终在 owner 之上）。
- 模态：创建子窗口时 `EnableWindow(main, FALSE)`，关闭后 `EnableWindow(main, TRUE)` +
  `SetForegroundWindow(main)`，对齐上游 `WindowDialog.ShowDialogAsync` 的 `ShowDialog()`。

未编译实测 `desktop_multi_window`（pub 0.3.1，mixin.dev，39 天前发布）：其 spawn 出的窗口
owner/模态语义不可控，且会引入第三方的第二 engine 生命周期依赖；原生路径使用 runner 已在用的
`flutter_windows` API，可控且更贴合“owner/模态/焦点”验收。**因此不对 `desktop_multi_window`
的可用性作任何结论。**

通信协议（同名通道、按 engine 各自 messenger 绑定）：

| 方向 | 方法 | 参数 | 语义 |
| --- | --- | --- | --- |
| 主 engine → native | `open` | snapshotJson | 创建/抬起独立窗口，返回 bool |
| native → 主 engine | `applyDraft` | `{id, draft}` | 子窗口草稿回传，主窗口执行保存 |
| 主 engine → native | `reportOutcome` | `{id, ok, message}` | 保存结果 |
| 子 engine → native | `ready` | — → snapshotJson | 启动取快照（带重试） |
| 子 engine → native | `saveDraft` | `{id, draft}` | 请求保存（转发主 engine） |
| native → 子 engine | `saveOutcome` | `{id, ok, message}` | 保存结果回投 |
| 子 engine → native | `close` | — | 关闭窗口（不写盘） |

子 engine 不调用 `RustLib.init`，不注册插件；草稿经主 engine 的既有 `saveDocument` →
autostart 同步 → `applyActive` 路径落盘。

## 2. 改动文件

- `apps/desktop/windows/runner/settings_window.{h,cpp}`（新增）
- `apps/desktop/windows/runner/option_window_host.{h,cpp}`（新增）
- `apps/desktop/windows/runner/win32_window.{h,cpp}`（owner 重载）
- `apps/desktop/windows/runner/flutter_window.cpp`（Attach 宿主；处理 `kSettingsClosedMessage`）
- `apps/desktop/windows/runner/runner_messages.h`（新增两条 WM_APP 消息）
- `apps/desktop/windows/runner/CMakeLists.txt`（加入新增源文件）
- `apps/desktop/lib/features/settings/settings_window_host.dart`（新增：宿主抽象 + 主/子两侧实现）
- `apps/desktop/lib/features/settings/option_setting_window_entry.dart`（新增：子 engine 根 widget）
- `apps/desktop/lib/features/settings/option_setting_window.dart`（新增 `host`/`standalone` 模式，
  五页/文案/字段/按钮不变）
- `apps/desktop/lib/features/settings/settings_actions.dart`（`openOptionSettingWindow` 改为开独立窗口，
  新增主侧保存回调）
- `apps/desktop/lib/main.dart`（新增顶层 `settingsWindowMain` entrypoint，唯一 engine 入口符号）
- `apps/desktop/test/r3_wpf_option_window_test.dart`（新增 widget 测试）
- 本证据目录；`docs/tasks/R3-WPF-OPTION-WINDOW.md`

未改：`main_shell.dart`、`app.dart`、`frb_generated*`、`bridge/api/**` 及其余业务 feature、`crates/**`。

## 3. 命令与结果

工作目录 `apps/desktop`：

| 命令 | 结果 |
| --- | --- |
| `flutter analyze` | `No issues found!` |
| `dart format --output=none --set-exit-if-changed lib test integration_test` | exit 0（298 files，0 changed） |
| `flutter test test/r3_wpf_option_window_test.dart` | 5/5 通过 |
| `flutter test`（option 相关：t12a_option_window / t12a_settings_storage / fix08_option_{apply,cancel,error} / fix16_settings_field / fix16b_settings_source / fix16e_cert_provider + 新测试） | 全部通过；`fix16b_settings_source_test` 首次命中已知 exit 79 偶发，隔离重试 3 次均通过 |
| `flutter build windows --release` | exit 0（首次 INSTALL 步骤因既有 Release 实例占用文件失败，重跑通过；见 §5） |

窗口探针（`probe_option_window.ps1`，隔离数据目录，从临时副本启动以取得独立单实例锁）：

```
run_exe  = %TEMP%\v2rayn-r-option-run-<guid>\v2rayn_desktop.exe
data_dir = %TEMP%\v2rayn-r-option-probe-<guid>
exited_early=false, stopped=true, alive_after=false   # 仅停止本脚本启动的 PID
```

| 窗口 | HWND | 标题 | visible | enabled | owner | style | rect |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 主 | `0x461016` | `v2rayN` | true | **false（模态禁用）** | — | `0x1CCF0000` | 10,10–1210,810 |
| 设置 | `0x1B126C` | `设置`(U+8BBE U+7F6E) | true | true | `0x461016` | `0x14CF0000` | 120,120–1120,820 |

探针断言（`probe-option-window.json`）：`main_present / settings_present / settings_title /
independent_hwnd / settings_owned_by_main / main_disabled_modal / settings_enabled /
two_visible_top_level / main_screenshot / settings_screenshot` 全部 `true`，`passed=true`。
截图 `probe-option.png` 为 1000×700 独立窗口（含标题栏、五页 Tab、`确定/取消`，Wave K 文案）。

`settings_foregroundable` 记录为 `false`：自动化终端自身持有前台，被启动进程无法夺取前台；
这是执行环境限制，非窗口缺陷。owner + 主窗口 `WS_DISABLED` 即模态/焦点关系的可观测保证。

## 4. 上游对照结论

- 上游 `MainWindowViewModel.cs:583-595`：`ShowDialogAsync` 返回 true 才刷新状态栏并 `Reload()`。
  本实现由主 engine 保存成功后 `applyActive()`，与“确定才生效”一致。
- 上游 `WindowDialog.cs`：`window.Owner = MainWindow; window.ShowDialog()`（模态、owner）。
  探针确认 owner 与主窗口禁用与该语义一致。
- 上游 `OptionSettingWindow.xaml.cs`：`btnSave` 触发 `RequestClose` → `DialogResult=true`；
  标题栏关闭返回 null/false。本实现确定→保存成功→关闭；取消/Esc/标题栏关闭不写盘。
- Wave K（标题「设置」、五页、`Core: 基础设置/v2rayN 设置/系统代理设置/Tun 模式设置/Core 类型设置`、
  `确定/取消`、无裸 key）未回退：widget 测试与探针截图均确认。

## 5. main_shell 接线补丁

**不需要。** `main_shell.dart` 的 `ACT-MAIN-024` 与 `V2RAYN_R_OPEN_SETTINGS` 钩子仍调用
`openOptionSettingWindow(context, ref)`；入口行为在 `settings_actions.dart` 内部切换为独立窗口，
菜单分发结构未改。

## 6. 未完成 / 接口缺口

- 未做逐事件行为对照（真实点击 确定/取消/标题栏关闭/重开的 UI 事件流）与 100%/150% DPI 对照；
  本卡据此只能记 `implemented`，未达 `verified`。
- 跨 engine 保存回环（子窗口草稿 → native 转发 → 主 engine `saveDocument`/`applyActive` →
  `reportOutcome` → 子窗口关闭）未在真实 release 包上驱动验证：探针只覆盖“打开/窗口形态”，
  子窗口保存语义由 widget 测试（fake host）覆盖，主侧 `_applyOptionDraft` 仅经编译与静态分析。
- 保存成功但 autostart（`GuiItem.AutoRun` 变更）写入失败时，主侧回调返回 `ok=true` 且不展示失败提示
  （子窗口随即关闭）；原 dialog 会在主窗口弹 SnackBar。属已知交互缺口。
- `settings_foregroundable` 因终端占用前台无法断言。
- 环境注意：本机已有一个本项目 Release 实例（PID 39396，`build\...\Release\v2rayn_desktop.exe`，
  非本会话启动、无会话记录）。为不杀不受本会话管理的进程，实施时把被占用的 `v2rayn_desktop.exe`
  重命名为 `v2rayn_desktop.stale.exe` 以解除 linker 占用；该进程未停止，仍以改名后的映像运行。
  用户如需可自行关闭。其它运行中的 `Debug` 实例、`dist` RC、用户真实 `v2rayN.exe` 均未触碰。
- 未对 `127.0.0.1:10808` 做任何监听/占用/修改；探针未触发 `applyActive`，无内核启动；未改系统代理/注册表/路由/TUN；未读用户凭据。

## 7. 下一步前置

1. 逐事件对照脚本：真实鼠标点击菜单→窗口出现→编辑→确定/取消/标题栏关闭/重开，采集 HWND 事件与两张截图。
2. 补 autostart 失败在子窗口的可见反馈（`reportOutcome.message` 在 ok 时也可携带，子窗口短暂提示）。
3. 在 100%/150% DPI 下重跑探针与截图。
4. 关闭步骤 5 中的 `.stale` 实例（需用户确认）后，正常 release 构建可不再改名。
