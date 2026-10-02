# T17 视觉 token 与交互组件规范（浅/深两套）

- 日期：2026-10-02
- 范围：`apps/desktop/lib/**`（界面层），不改 Rust/FRB 合同。
- 依据：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §7（保留结构 + 视觉升级限定）、§8（事件级交互）、§21；`compat/layouts.yaml` 窗口/布局/列/尺寸；`compat/actions.yaml` main-menu/tray/statusbar。

## 决策

1. **单一组件系统**：继续使用 Material 3 + `VisualDensity.compact`，不引入第二套组件库；所有外观集中到 `lib/shared/theme/app_theme.dart`，避免焦点/菜单/字体不一致（§7）。
2. **语义 token 用 ThemeExtension**：新增 `AppSemanticColors`（success/warning/info/hover/selectedRow/disabledForeground/zebraStripe/focusOutline/gridLine + 行高/等宽字体/斑马开关），浅深各一套；`BuildContext.semantics` 提供亮度正确的回退，保证裸 `MaterialApp` 的 widget 测试也能渲染。
3. **排版层级固定**：标题 14 / 副标题 13 / 正文 12.5 / 小字 11.5 / 微字 11；日志与 JSON 用等宽（`AppTokens.monoFontFamily`）。字号 `UIItem.CurrentFontSize` 与主题联动，行高按字号落 24/26/28（`rowHeightFor`），保持紧凑桌面密度，不变成触屏大行高。
4. **图标族统一**：`AppTokens.icon(semantic)` 把上游语义映射到 Material outlined 图标，工具栏/菜单/状态栏共用同一族，避免线宽与光学尺寸漂移。
5. **窗口台账**：`AppWindowMetrics` 固定标题 `v2rayN`、默认 1200x800、最小宽 800（LAY-MAIN-002 / INV-WPF-002）。标题与最小尺寸由 `DesktopIntegration` 在真实运行时设置；持久化尺寸仍由 Win32 runner 拥有。
6. **斑马纹可选**：默认关闭，`UIItem` 之外以 UI 偏好 `zebra` 存 `ui_state`，菜单“主界面/表格斑马纹”切换；行高与信息密度不变。
7. **对话框锚点/按钮顺序**：共享 `showAppConfirmDialog` 固定“取消在左、确定在右”，主按钮 `autofocus` 使 Enter 生效，Esc 走 `WidgetsApp` 默认 `DismissIntent`；确认按钮在破坏性场景使用 error 色。
8. **有序反馈、无假进度**：进度只显示真实阶段文本（`StageIndicator` 仅转圈 + 阶段名），不产生百分比；空态统一 `EmptyState`（图标 + 文案 + 可选副文案）。

## 受影响的不变量

- 三布局与分栏比例持久化沿用 T05 的 `ui_state`（`layout` section），未改变字段。
- 主题持久化新增 `zebra` 键；`toggleTheme` 现在走 `_persistTheme()`，避免只写 `mode` 覆盖其它键。
- `desktopIntegrationProvider` 由 `Provider<null>` 改为持有实例的 `Provider<DesktopIntegrationHolder>`，供菜单“关闭”在真实运行时最小化到托盘，测试环境保持 null 走诚实状态消息。

## 未决 / 不声称

- 本轮未做 100%/125%/150%/200% DPI 与多显示器的像素级比对，属于 T18/T19 的平台验证范围。
- 未做原生 macOS/Linux 视觉验证。
- 视觉 token 的对比度仅为设计选择，未跑自动化 WCAG 计算。
