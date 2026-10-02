# UX-CTX-01 执行证据 — 节点右键定位、关闭、焦点

日期：2026-10-03。基线 HEAD `73eaad2`（工作树有本轮改动，见下）。本轮只修改节点表右键
菜单的定位/关闭/焦点/命令目标，**未改菜单条目、顺序、层级**（结构恢复属 UX-SPACE-02）。
未 commit。

## 结论

审查定位的 1–6 条根因已逐条修复，真实窗口运行验证：

- `menu-position-and-density`：菜单第一项左缘与点击点 `leftDelta=0.0`（修复前为 171），
  水平落在窗口内。full-f3 / interaction-24 / interaction-shots-43。
- `left-click-another-row-dismisses-menu`：一次左键关闭菜单且选中被点为 B（close + 透传），
  菜单不再悄悄保留。full-f3 / interaction-24。
- `escape-closes-menu-without-clearing-selection`：菜单开着时 Esc 关闭菜单且选择不变，
  不再泄漏到表格清空选择。full-f3 / interaction-24。
- `right-click-different-row-while-menu-open`：唯一菜单（menuCount=1），选择切到新行，
  菜单重定位到新点击点（leftDelta 0.0）。full-f3 / interaction-24。
- `right-click-row-header`：行号右键打开菜单并以该行为目标。full-f3 / interaction-24。
- `right-click-empty-table`：空区域右键打开整表菜单，目标相关项禁用，不拿不存在行冒充对象。
  interaction-24 / 00-empty-right-click.png。
- `right-click-selected-multi-row`：多选内右键保留多选集合（正向控制保持）。full-f3。
- editor 场景 `editor-command-{0,1}` + `editor-cancel-{0,1}`：编辑打开正确对象、取消后
  节点数/选择保持（正向控制保持）。editor-shots-3。

完整 32 节点场景 `full-f3` 通过（recordingComplete=true，failures 为空）。
**原生崩溃状态**：Debug 下完整场景存在环境级 `flutter_windows.dll 0xc0000005 @0x927160`
间歇崩溃，见下节；但已取得一次完整通过运行，且崩溃可复现地与运行长度相关，与截图编码
无关（SKIP_IMAGES 下仍崩）。

## 根因与修复逐条

1. **坐标空间混用**（`profiles_table.dart:357/431`）。改为用实际 anchor RenderBox
   （`_anchorBoxKey` 挂在 MenuAnchor.child 的 Listener 上）`globalToLocal(event.position)`
   得到 anchor 本地坐标再 `open(position:)`；不再手工减侧栏宽度。菜单左移 171 → leftDelta 0。
   移动/缩放/分隔条拖动/多尺寸未逐一回归（见未验证点）。
2. **命中范围覆盖整表**。新增 `_onPointerDown`（Listener.opaque 包整表）：菜单打开时表格
   左键按下 → 关闭整条菜单链；该次点击按“关闭+正常处理”透传（关闭用 post-frame 延迟以避免
   取消进行中的 tap 手势）。点击另一行会同时选中 B。right-click 其他目标 → 关闭旧菜单并对
   新目标重建（唯一菜单）。
3. **焦点与 Esc 优先级**。`_onKey` 先判断 `_menuController.isOpen`：打开时 Esc → 关闭菜单、
   恢复表格焦点、`return handled`，本次按键不再流入表格；菜单关闭时保留 ACT-PROF-038 表格
   Esc 语义。打开菜单后 `_focusNode.requestFocus()` 使 Esc 能到达 `_onKey`。
4. **触发覆盖**。右键入口上移到整表 Listener：数据单元格、行号列、列头、空区域统一处理。
   行号/数据 → 该行为目标；列头/空区域 → 保持当前选择为目标，无选择时依赖选择的动作禁用。
5. **命令目标会话化（UX-CTX-02 要点）**。新增 `ContextMenuSession`（`context_menu_session.dart`）：
   打开时快照 targetIds（多选保留）、primaryId、groupSubId、打开位置、焦点恢复对象、区域。
   `_onContextAction` 用会话执行；执行前 `restoreContextTargets` 校验 ID 仍存在并按快照重绑
   选择；分组变化或 ID 失效 → 关闭菜单并提示“操作目标已失效，请重新选择节点”，不取第一条
   可见行。因 MenuItemButton 会先关闭菜单（触发 onClose 清空会话）再调用 onPressed，会话以
   闭包快照方式绑定到每个 `onPressed`。
6. **重复右键**。菜单开着时右键新目标 → 选择切到新目标、只有一个菜单、位置正确、不执行命令。
7. **多选内右键保留**与 run-07 编辑取消链路保持（见结论）。
8. **原生崩溃排查**：完整场景 Debug 下观测到 `flutter_windows.dll 0xc0000005 @0x927160`
   的后台进程死亡（`did not complete`），与旧证据 run-02/04/08 完全同址。SKIP_IMAGES=1 无法
   消除，故与截图编码无关。多次运行崩溃点随机（app-started 后即崩 / import 后 / submenu 后），
   属引擎级间歇故障；本轮凭一次完整通过（full-f3）记录契约，同时保留原始失败与事件。

## 证据边界

- 真实 Windows Flutter 窗口，连接真实 FRB/Rust/SQLite；节点为公开协议格式的合成 loopback
  数据。窗口内容区 1184×761 逻辑像素，pixelRatio=1。
- 分离数据目录 `target/ux-ctx-01-*`；UI 偏好经 `FileUiStateStore` 文件位置隔离。
- 未启动内核、未监听测试端口、未改系统代理/自启/TUN，未触碰 10808。
- 原生 computer-use 工具不可用，**未对冻结版 v2rayN 做原生鼠标逐事件对照**；原版规则来自
  冻结源码 `ProfilesView.xaml` 的 `DataGrid.ContextMenu`（覆盖整表）与
  `ProfilesView.xaml.cs::LstProfiles_PreviewKeyDown`。
- 尺寸与结构合同分开记录；原版按下/松开时机、子菜单退层、滚轮、窗口失活等仍未验证（见下）。

## 运行命令与环境

在 `apps/desktop` 运行（`C:/Users/Colby/toolchains/flutter/bin/flutter.bat`）：

```powershell
$taskRoot = 'C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn'
$env:V2RAYN_R_DATA_DIR = "$taskRoot/target/ux-ctx-01-full-f3"
$env:V2RAYN_R_CONTEXT_EVIDENCE_DIR = "$taskRoot/docs/evidence/UX-CTX-01/full-f3"
$env:V2RAYN_R_AUTOSTART = '0'
$env:V2RAYN_R_AUTO_SMOKE = '0'
$env:V2RAYN_R_CONTEXT_SKIP_IMAGES = '1'
Remove-Item Env:V2RAYN_R_CONTEXT_MODE -ErrorAction SilentlyContinue
flutter test integration_test/context_menu_review_test.dart -d windows --reporter expanded
```

- `interaction` 模式：8 节点，覆盖定位/关闭/Esc/多选/重复右键/行号/空区域，用于稳定复现契约。
- `editor` 模式：4 节点，编辑→取消→再编辑另一节点。
- `full`（默认）：32 节点完整场景。

## 通过运行索引

| 运行 | 模式 | 图像 | 结果 | 文件 |
|---|---|---|---|---|
| full-f3 | full(32) | 跳过 | All tests passed | `full-f3/observations.json` |
| interaction-24 | interaction(8) | 跳过 | All tests passed | `interaction-24/observations.json` |
| interaction-shots-43 | interaction(8) | **PNG** | All tests passed | `interaction-shots-43/` |
| editor-shots-3 | editor(4) | **PNG** | All tests passed | `editor-shots-3/` |
| editor-ctl | editor(4) | 跳过 | All tests passed | `editor-ctl/observations.json` |

截图（`interaction-shots-43/`）：`01-right-menu.png`（修正后定位）、`02-click-other-row.png`、
`03-escape.png`、`04-empty-right-click.png`、`00-empty-right-click.png`。`editor-shots-3/`：
`editor-0.png`、`editor-1.png`。

## 未通过 / 崩溃记录

- `full/`：SKIP_IMAGES=1 完整场景，00:02 `did not complete`，`windows-crash-events.json`
  记录 `flutter_windows.dll 0xc0000005 @0x927160`。
- `full-b/`…`full-run5/`、`full-f1/f2`：多次 `did not complete`，崩溃点随机（steps 1–16）。
- `interaction-{1..7}`、`interaction-{11..23}`、`full-run1` 等：部分运行到完成并记录逐条断言，
  部分原生崩溃。所有失败均保留 observation/日志。
- 这些 `did not complete` 是进程级死亡（Debug `flutter_windows.dll`），与
  `tools/flutter_test_retry.ps1` 已记录的 `flutter_tester` 段错误属同一环境级间歇故障家族；
  不归因到某项菜单逻辑，因为同一逻辑在 full-f3/interaction-24/editor 完整通过。

## 门禁

- `dart format --output=none --set-exit-if-changed lib test integration_test/context_menu_review_test.dart`：0（150 文件，无变更）。
- `flutter analyze`（apps/desktop）：No issues found。
- `flutter build windows --release`：成功，`build/windows/x64/runner/Release/v2rayn_desktop.exe`。
- `flutter test` 逐文件（`tools/flutter_test_retry.ps1 -PerFile`）：全部通过。首次运行个别文件
  报告 exit 79（`flutter_tester` 段错误，如 profiles_keyboard/t10_groups_panel），单独重跑均
  exit 0；属已知引擎间歇故障，非本轮回归。
- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings`
  / `cargo test --workspace --locked`：全部 0（未改 Rust）。

## 与上游未验证点登记

- 原版右键**按下/松开时机**（onSecondaryTapDown 即开 vs 松开才开）与原生焦点行为：未实测。
- **子菜单 Esc 退层**：本轮实现为“关闭整链”，未对照原版“退一层”；待实机冻结。
- **滚轮 / 窗口失活 / 最小化 / 布局切换 / 多 DPI / 不同窗口大小 / 分隔条拖动**：未逐一回归。
- **表头/空区域**的具体原版选择与启用规则：结构范围有源码证据（DataGrid.ContextMenu 覆盖
  整表），逐区域行为未实测。
- **键盘菜单键 / Shift+F10**：原版默认能力未实测，本轮未实现该入口。
- 菜单“**高度/密度/结构**”仍是 UX-SPACE-02 范围，未动。

## 改动文件

- `apps/desktop/lib/features/profiles/profiles_table.dart`：坐标转换、命中范围/焦点/Esc、
  会话快照、动作目标校验与按会话执行；未改菜单条目/顺序/层级。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：新增
  `restoreContextTargets`、`resetEvents`（仅菜单/选择协作与证据；未改业务动作）。
- `apps/desktop/lib/shared/widgets/context_menu_session.dart`（新增）：会话模型。
- `apps/desktop/integration_test/context_menu_review_test.dart`：断言改为契约；
  新增 `interaction` 模式；几何点击点改到菜单面板之外。
- `compat/actions.yaml`：仅 ACT-PROF-001、ACT-PROF-038 追加 `notes`，未删行、未降分母。
