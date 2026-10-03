# UX-SPACE-03-VLESS 执行证据 — VLESS 节点编辑表单留白 / 字号 / 错误空间

日期：2026-10-03。冻结上游 v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
应用基线 HEAD `25c907e`。状态：`implemented`（100% DPI 真窗口实测；其余组合未验证，见文末）。

本轮只改 VLESS 编辑弹窗的**呈现层**与共享表单/theme 的最小样式：
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
- `apps/desktop/lib/shared/theme/app_theme.dart`（新增 `AppForm` 度量与文本样式）
- 新增测试 `apps/desktop/test/ux_space03_vless_editor_test.dart`、
  `apps/desktop/integration_test/ux_space03_vless_editor_test.dart`、
  `apps/desktop/integration_test/ux_space03_before_probe_test.dart`

未改字段含义 / 顺序 / 分组 / 默认值 / 敏感字段遮挡 / 保存取消边界；未删字段、未折叠字段。

## 门禁环境说明

执行期间共享工作树被其他并行任务改到 `profiles_table.dart`/`context_menu.dart` 等文件且处于
**无法编译**的中间态。为保证本轮证据只反映自己的改动，所有门禁与真窗口验证在 `git worktree`
（detached HEAD `25c907e`）中复制本轮 4 个文件后运行，不触碰、不回滚其他代理的文件。
该 worktree 已清理。共享树当前仍在其他任务编辑。

## 1. 实测尺寸表（前后，逻辑像素）

真窗口内容区 1184×761，`devicePixelRatio=1.0`（100% DPI）。默认字体 = 12.5（`CurrentFontSize` 未设置时）。

| 度量 | 前（HEAD 25c907e） | 后（默认 12.5） | 后（大字号 20） | 目标 | 结论 |
|---|---|---|---|---|---|
| 备注输入框高 | **22** | **34** | 45 | 34–36 | 达标；大字号随字号增长 |
| 标签列宽 | 无独立标签列（浮动标签） | 148 | 148 | 长标签可换行 | 达标 |
| 标签↔控件净距 | —— | 12 | 12 | 12–16 | 达标 |
| 相邻字段净距 | 约 8（上下各 4） | 10 | 10 | 8–12 | 达标 |
| 分组间净距（协议末字段→传输标题） | —— | 16 | 16 | 16 | 达标 |
| 标签字号 | 12 | 12.5 | 20 | 随用户字号 | 达标 |
| 内容字号 | 12（硬编码） | 12.5 | 20 | 随用户字号 | 达标 |
| 错误字号 | 12（硬编码） | 11.5（bodySmall） | 19 | 随用户字号 | 达标 |
| 保存按钮底边 | —— | 706 ≤ 761 | 706 ≤ 761 | 不跑出弹窗 | 达标 |
| 取消按钮底边 | —— | 706 ≤ 761 | 706 ≤ 761 | 不跑出弹窗 | 达标 |

- 前值来源：`before/observations.json`（baseline 实测备注框高 22，与专项 run-07 一致）。
- 后值来源：`observations.json`（真窗口 `getRect`，非仅断言 Text 存在）。
- 端口错误矩形：`{x:456, y:308, w:95.7, h:15}`；备注报错后其下一字段（地址）净距仍为 10，
  错误不漏盖下一字段，见 `02-editor-error-light.png`。
- 大字号下输入框高 45 > 36：登记为规格缺口——控件高度必须随 `CurrentFontSize` 增长而不是
  截断文字；未缩小设置范围、未裁剪文字。

原始 JSON：`observations.json`（真窗口）、`before/observations.json`（基线）。

## 2. 字号传播实现

`app_theme.dart` 新增 `AppForm`，所有样式从 `ThemeData.textTheme` 派生：

- `AppForm.contentStyle(theme)` = `theme.textTheme.bodyMedium`（= `buildAppTheme(fontSize: shell.fontSize)`
  里的 `baseSize`，即 `UiItem.CurrentFontSize` / FLD-CFG-075）。
- `AppForm.labelStyle` = contentStyle + `w500`；`AppForm.sectionTitleStyle` = contentStyle + `w600`；
  `AppForm.errorStyle` = `bodySmall`（baseSize−1）+ `colorScheme.error`。
- 编辑器删除了全部局部 `const TextStyle(fontSize: 12)`（标签/内容/错误/下拉项/错误横幅/标题）；
  下拉 `style: contentStyle`，多行内容 `contentStyle.copyWith(fontFamily: monospace)`。
- 依据：`buildAppTheme` 把 `fontSize` 写入 `textTheme.bodyMedium`；`shell.fontSize` 来自
  `ui_shell_controller` 对 `UiItem.CurrentFontSize` 的读取，故为真实传播，不是只改 Theme 后仍锁 12。

真窗口记录：默认 `baseFontSize=12.5 / label=12.5 / content=12.5`；大字号 `base=20 / label=20 /
content=20`，错误 `19`。

## 3. 取消 / 错误场景结果

真窗口 + widget 测试一致：

1. 打开 VLESS 节点 → 改备注「真实窗口改后备注」、端口 `99999` → 点保存。
2. 保存被本地校验拒绝（`onSave` 未调用，节点数不变），弹窗保持，错误「端口需在 1-65535」可见，
   草稿保留（备注/端口仍为改动值）。→ `02-editor-error-light.png`。
3. 点取消 → 弹窗关闭，节点数/选择不变。
4. 重新右键同一节点→编辑 → 读出原值：备注 `节点1`、端口 `11980`（未保存草稿被丢弃）。
   → `03-editor-reopen-light.png`。

同一流程在 `test/ux_space03_vless_editor_test.dart` 以合成 VLESS 节点断言（`saveCalls==0`、
错误可见、草稿保留、取消后重开原值）。

## 4. 其他协议抽查结论

共享 `_buildField/_buildControl` 被所有协议编辑共用。抽查 VMess（`field-alterId`）与
Trojan（`field-password`）：字段渲染完整、控件高 34–37、无溢出/裁剪、保存/取消在窗内
（`shared form does not clip VMess or Trojan fields` 通过）。未逐一提测其余 9 个协议与
自定义/分组编辑弹窗——登记为未验证。

## 5. 上游映射

`AddServerWindow.xaml` `gridVLESS` 可见字段：`txtId5`(TbId5 用户 UUID)、`cmbFlow5`(TbFlow5 流控)、
`txtSecurity5`(TbSecurity5 加密)、`togmuxEnabled5`(Mux)；顶部行含 CoreType/备注/地址/端口，与当前
分组 `基础 / 协议 / 传输 / TLS·Reality` 的字段集合一致。本轮只把标签改为左侧固定列、统一间距与
字号来源，字段集合、顺序、分组、默认值、遮挡语义均未改变。

## 6. 截图路径

- 前基线（浅色）：`before/00-editor-before.png`（浮动标签、备注框高 22）
- 后（浅色默认）：`01-editor-default-light.png`
- 后（错误态）：`02-editor-error-light.png`
- 后（取消重开）：`03-editor-reopen-light.png`
- 后（深色默认）：`04-editor-default-dark.png`
- 后（大字号 20）：`05-editor-large-font.png`
- 收尾：`final-state.png`

## 7. 门禁结果（worktree HEAD 25c907e + 本轮文件）

| 检查 | 命令 | 结果 |
|---|---|---|
| 格式 | `dart format --output=none --set-exit-if-changed lib test` | 0（150 文件，0 changed） |
| 分析 | `flutter analyze` | No issues found |
| Widget 测试 | `tools/flutter_test_retry.ps1 -PerFile` | 全部通过（引擎间歇崩溃按重试通过；`t21e_import_forms_test` 需原生 `bridge_api.dll`，worktree 构建后通过） |
| 真窗口 | `flutter test integration_test/ux_space03_vless_editor_test.dart -d windows` | All tests passed（第2次尝试通过；第1次为本项目已知 `flutter_windows.dll` 间歇原生崩溃） |
| 前基线 | `flutter test integration_test/ux_space03_before_probe_test.dart -d windows`（还原 HEAD 两个 lib 文件） | All tests passed |
| Release | `flutter build windows --release` | 成功 |
| Rust 格式 | `cargo fmt --all -- --check` | 0 |
| Rust clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 |
| Rust 测试 | `cargo test --workspace --locked` | 0（补入 `tools/cores` 后；未补 cores 时仅 `t10_real_core_matrix` 因缺 xray/sing-box 可执行文件失败，属检出环境缺失，非本轮回归） |

未改 Rust/FRB，未运行内核、未监听端口、未改系统代理/TUN，未触碰 10808。全程无 commit。

## 8. 未验证与遗留

- **DPI**：只实测 100%（`devicePixelRatio=1.0`）。125% / 150% 未测。
- **窗口**：只测默认窗口（内容区 1184×761）。缩小到 800×600 最小窗口未做真窗口缩放实测。
- **布局/主题**：仅默认布局；浅/深色已各测一次；水平/标签布局未测。
- **大字号 + 紧凑行高冲突**：输入框高 45 超出 34–36 目标，登记规格缺口；未裁文字、未缩设置值域。
- **其他协议/窗口**：VMess、Trojan 抽查；其余协议、参数设置、自定义/分组编辑弹窗未逐一验收。
- **原版逐像素对照**：无原生 computer-use，未对冻结版做原生鼠标/像素对照；原版尺寸来自冻结源码。
- **原生崩溃**：真窗口 Debug 运行偶发 `flutter_windows.dll` 间歇崩溃（本项目已知环境问题），
  本轮以重试拿到完整通过；未定位根因。
