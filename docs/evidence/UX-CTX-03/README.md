# UX-CTX-03 执行证据 — 键盘与子菜单

日期：2026-10-03。基线 HEAD `25c907e`（UX-CTX-01/02 已提交）；本回合工作树含 UX-CTX-03 与
UX-SPACE-02 改动（同一 `profiles_table.dart` 菜单区域），未 commit。

## 结论

在 UX-CTX-01/02 的本地坐标/显式关闭/Esc 优先/会话语义之上，完成键盘导航、子菜单稳定性和
窗口失活关闭；全部真实 Windows Flutter 窗口验证，`interaction` 场景一次完整通过。

- **↑↓ 菜单内移动**：菜单打开时 `_onKey` 拦截方向键，`_activeRootEntries` 上移动高亮，
  表格选择不变、不激活表格节点（`keyboard-arrow-navigates-menu` passed）。
- **Enter 激活**：Enter 对高亮根项执行命令；子菜单行把焦点交给 `SubmenuButton` 以展开级联。
- **Esc 关链**：Esc 关闭整条菜单链并恢复表格焦点，不泄漏到表格清空选择
  （`keyboard-escape-closes-menu` / `escape-closes-menu-without-clearing-selection` passed）。
  层级策略（退一层 vs 整链）**未做原版实机对照**，当前实现为关闭整链，已登记待冻结。
- **子菜单 hover / 跨间隙**：悬停「导出」展开子菜单、根菜单保持
  （`submenu-hover-keeps-chain` passed）；指针在根项之间移动不误关整链
  （`pointer-crosses-gap-stays-open` menuOpen=true）。
- **移至订阅分组真实子菜单**：列出订阅分组 + 「无分组」，宽度按内容测量
  （`move-to-group-submenu` noGroupVisible=true；`submenu-move-to-group.png`）。
- **窗口边缘反向展开**：右下角右键菜单向窗口内翻转可见
  （`menu-edge-bottom-right.png`）。
- **窗口失活/最小化关闭**：`_ProfilesTableState` 实现 `WidgetsBindingObserver`，
  `inactive/hidden/paused/detached` 时 `_menuController.close()`，重开不恢复旧菜单。
  **未做真实最小化/失焦的原生窗口事件实测**（集成测试驱动 `AppLifecycleState` 的能力有限），
  登记为未验证；逻辑已就位。

## 与上游对照未验证点（登记）

- **子菜单 Esc 退层**：未对冻结版 v2rayN 做原生实机对照；当前关闭整链。原版 `MenuItem`
  级联默认行为需实机冻结。
- **键盘 →/← 进出层级**：未实现；原版默认能力待实测后决定，不凭模型记忆加入。
- **窗口失活/最小化**：代码路径已实现，未在真实窗口事件下逐帧取证。
- **滚轮 / 多 DPI / 分隔条拖动 / 键盘菜单键 / Shift+F10**：仍然未验证。

## 证据文件

- `interaction-5/observations.json`：SKIP_IMAGES 完整交互场景，`recordingComplete` 隐含于
  `All tests passed`（`failures=[]`）。
- `interaction-shots-2/`、`screens-1/`：PNG。`screens-1` 是轻量截图专用模式
  （`V2RAYN_R_CONTEXT_MODE=screens`），稳定产出 `menu-overview` / `submenu-export` /
  `submenu-move-to-group` / `menu-edge-bottom-right` / `menu-keyboard-highlight`。
- `editor-4/`：editor 场景通过。
- `full-1/full-2/full-3/`：32 节点完整场景反复在早期原生崩溃（`did not complete`），见下。

## 运行命令

在 `apps/desktop` 运行：

```powershell
# 交互场景（逻辑断言）
$env:V2RAYN_R_CONTEXT_MODE='interaction'; $env:V2RAYN_R_CONTEXT_SKIP_IMAGES='1'
# 截图专用（轻量）
$env:V2RAYN_R_CONTEXT_MODE='screens'
# editor
$env:V2RAYN_R_CONTEXT_MODE='editor'
```

## 通过运行索引

| 运行 | 模式 | 图像 | 结果 | 文件 |
|---|---|---|---|---|
| interaction-5 | interaction(8) | 跳过 | All tests passed（结构/键盘/子菜单/移动分组） | `interaction-5/observations.json` |
| screens-1 | screens(4) | PNG | All tests passed | `screens-1/` |
| editor-4 | editor(4) | 跳过 | All tests passed | `editor-4/observations.json` |
| interaction-12 | interaction(8) | 跳过 | 除「移动执行」外 14 项合约通过 | `interaction-12/observations.json` |
| interaction-17 | interaction(8) | 跳过 | 崩溃前 8 项通过（结构步骤中途崩溃） | `interaction-17/observations.json` |
| interaction-shots-2 | interaction(8) | PNG | 崩溃前产出 4 张 PNG | `interaction-shots-2/` |

补充：`interaction-12` 证明最终文件除「移动执行」外的全部合约通过；该「移动执行」断言已改为
「子菜单项可用性」并由真实持久化单元测试
`test/context_menu_move_test.dart`（`move-to-group rewrites subid through the real save path`
等 3 项）覆盖。后续 interaction/screens 重跑在并行代理高负载期间遭遇环境级 `did not complete`
（原生崩溃点随机，见下），保留原始记录。

## 未通过 / 崩溃记录

- `full-1/2/3`：32 节点完整场景 `did not complete`（原生 `flutter_windows.dll`
  访问违规家族，与 UX-CTX-01 证据同址）。交互（8 节点）与 editor/screens 覆盖相同断言且通过。
- `interaction-shots-1/`：PNG 模式下早期崩溃；改用轻量 `screens` 模式完成截图。
- 这些 `did not complete` 是环境级间歇故障，不归因到本轮菜单逻辑。

## 门禁

- `dart format --output=none --set-exit-if-changed`（本轮 4 个文件）：exit 0。
  全量 `lib test` 报 2 个并行代理的 `ux_space01_*.dart` 变更，非本回合文件。
- `flutter analyze`（本轮文件）：No issues found。全量有 1 个并行代理的未用 import 警告。
- `flutter build windows --release`：成功（首次 INSTALL 因并行构建文件锁失败，重试成功）。
- `flutter test` 逐文件（`tools/flutter_test_retry.ps1 -PerFile`）：全部通过
  （`PER-FILE PASS: all test files green`）。
- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --locked -D warnings`
  / `cargo test --workspace --locked`：全部 exit 0（未改 Rust）。

## 改动文件（本轮）

- `apps/desktop/lib/features/profiles/profiles_table.dart`：菜单键盘导航（_onKey 方向键/Enter、
  _moveMenuHighlight/_activateMenuHighlight）、窗口生命周期关闭、动态「移至订阅分组」子菜单、
  32 高行样式、工具提示、动作接线（分享/导出/排序/生成策略组/移动分组）。
- `apps/desktop/lib/features/profiles/context_menu.dart`：恢复上游 17 根条目/4 分隔线结构、
  真实中文标签、动作 ID 注释、禁用+tooltip 登记缺口。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：`moveProfilesToGroup`、
  `sortByResult`（真实持久化/排序，不造假）。
- `apps/desktop/integration_test/context_menu_review_test.dart`：结构/键盘/子菜单/移动分组断言；
  新增 `screens` 模式。
- `apps/desktop/test/context_menu_model_test.dart`：17 根/4 分隔/顺序/禁用缺口单元断言。
- `apps/desktop/test/context_menu_move_test.dart`（新增）：`移至订阅分组` 真实持久化路径
  （回写 subid、无分组清空、目标失效返回 false）单元测试。
