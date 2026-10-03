# UX-SPACE-01 — 恢复原版节点区入口与表头

日期：2026-10-03。开始基线 HEAD=`25c907e`（真实工作树，未 commit）。冻结上游 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本轮改动文件：

- `apps/desktop/lib/features/profiles/profiles_page.dart`（顶部 WrapPanel 入口、移除左栏）
- `apps/desktop/lib/features/profiles/profiles_models.dart`（展示标签本地化，key 仍为 ExName）
- `apps/desktop/lib/shared/theme/app_theme.dart`（新增 `AppTokens` 工具栏几何 token）
- 测试：`test/ux_space01_group_flow_test.dart`、`test/ux_space01_entries_test.dart`、`test/ux_space01_column_persistence_test.dart`；集成证据 `integration_test/ux_space01_layout_evidence_test.dart`
- 既有测试按展示标签更新：`profiles_models_test.dart`、`profiles_pointer_test.dart`、`t05_profiles_ui_test.dart`、`t10_groups_panel_test.dart`

未触碰：`profiles_table.dart`（菜单事件归 UX-CTX-01/02）、`profile_editor_dialog.dart`。本轮期间其他代理并发修改了 `profiles_table.dart`、`context_menu.dart`、`profiles_controller.dart`、`profile_editor_dialog.dart`；未回退任何他人改动。

## 1. 结构恢复对照（LAY-PROFILES-001）

上游 `ProfilesView.xaml:24..99` 的 `WrapPanel` 顺序，与当前实现逐项对照：

| # | 上游控件（冻结源码） | 上游文案（ResUI.zh-Hans） | 当前实现 | key |
|---|---|---|---|---|
| 1 | `lstGroup` ListBox（ItemContainerStyle=MyChipListBoxItem，MaxHeight=200） | `menuSubscription`=订阅分组；“全部”=AllGroupServers | 顶部 `ChoiceChip` 列表，“全部”+每个订阅分组；长中文名软换行（maxWidth 360，不裁剪） | `group-filter-all` / `group-filter-<subId>` |
| 2 | `btnEditSub` Button 30×30 Kind=Edit | `menuSubEdit`=编辑 | 图标按钮 30×30，打开订阅设置窗口 | `toolbar-sub-edit` |
| 3 | `btnAddSub` Button 30×30 Kind=Plus | `menuSubAdd`=添加 | 图标按钮 30×30，打开订阅设置窗口 | `toolbar-sub-add` |
| 4 | `txtServerFilter` TextBox Width=200 | `MsgServerTitle`=过滤器，按回车执行 | 过滤框 200×30，Enter 刷新 | `filter-field` |
| 5 | `btnAutofitColumnWidth` 30×30 Kind=ArrowSplitVertical | `menuProfileAutofitColumnWidth`=自动调整列宽 | 图标按钮 30×30 | `toolbar-自动列宽` |
| 6 | `btnFastRealPing` 30×30 Kind=LightningBolt | `menuFastRealPing`=一键测试真连接延迟 | 图标按钮 30×30 | `toolbar-快速真延迟` |
| 7 | `menuMixedTestServer` 30×30 Kind=Speedometer | `menuMixedTestServer`=一键多线程测试延迟和速度 (Ctrl+E) | 图标按钮 30×30 | `toolbar-混合` |
| 8 | （下方）`DataGrid lstProfiles` | — | 表格占满整宽，左缘 x=0；无左侧分组栏 | `ProfilesTable` |

额外保留项（原版 24..99 无此槽位，但为“不得删功能”而保留，登记如下）：

- `toolbar-备注`（编辑备注 / `renameSelectedProfile`）：当前 app 的唯一入口，原版右键菜单与快捷键均无对应项 → 需菜单侧补“重命名备注”入口后才可移出。
- `column-settings-button`（显示列设置）：同时有菜单入口 设置→主界面→显示列设置（`UI-COLUMNS`）。
- `toolbar-停止测试`：仅测速运行时出现（Esc 亦可停止）。

被移除的左侧“分组”面板（`groups-title`、`生成全部组`、`生成地区组`）恢复为顶部 chip；“生成策略组”动作仍在右键“一键生成策略组”子菜单（ACT-PROF-007/008）。

## 2. 尺寸实测（真实窗口）

证据：`real-window/observations.json`（`setSurfaceSize` 逻辑像素，pixelRatio=1；合成节点数据；OS 窗口为真实运行窗口）。

| 场景 | 工具栏矩形 | 折行数 | 过滤框 | 表头高 |
|---|---|---|---|---|
| 800×600 垂直 浅色 | 800×72 @(0,41) | 2 | 200×30 @y=45 | 30 |
| 1200×800 垂直 浅色 | 1200×38 @(0,41) | 1 | 200×30 @y=45 | 30 |
| 1920×1080 垂直 浅色 | 1920×38 @(0,41) | 1 | 200×30 @y=45 | 30 |
| 1200×800 水平 浅色（面板 597 宽） | 597×72 @(0,41) | 2 | 200×30 @y=79 | 30 |
| 1200×800 标签 浅色（内容 1079 宽） | 1079×38 @(121,41) | 1 | 200×30 @y=45 | 30 |
| 1200×800 垂直 深色 | 1200×38 @(0,41) | 1 | 200×30 @y=45 | 30 |
| 800×600 垂直 深色 | 800×72 @(0,41) | 2 | 200×30 @y=45 | 30 |

结论：宽屏/1200 为单行 38 高（30 图标 + 上下 4 padding）；窄窗/水平布局按 Wrap 规则受控折为 2 行 72 高，无裁剪、无重叠。原“两行额外文字条”造成的 84 高偏离消除（旧证据见 `context-menu-review-2026-10-03/run-02`：工具栏 84、表格左缘 x=171；现表格左缘 x=0）。行高仍取 24/26/28，表头 30，字号 12–12.5，未抬高行高。

## 3. 列展示标签与持久化（LAY-PROFILES-002）

`profiles_models.dart` 的 `title` 改为 `ResUI.zh-Hans.resx` 本地化文案，`key` 保持 ExName：

| key (ExName) | 资源键 | title |
|---|---|---|
| ConfigType | LvServiceType | 类型 |
| Remarks | LvRemarks | 别名 |
| Address | LvAddress | 地址 |
| Port | LvPort | 端口 |
| Network | LvTransportProtocol | 传输协议 |
| StreamSecurity | LvTLS | TLS |
| SubRemarks | LvSubscription | 订阅分组 |
| DelayVal | LvTestDelay | 延迟 (ms) |
| SpeedVal | LvTestSpeed | 速度 (MB/s) |
| TodayUp | LvTodayUploadDataAmount | 今日上传 |
| IpInfo | LvTestIpInfo | IP 信息 |
| TodayDown | LvTodayDownloadDataAmount | 今日下载 |
| TotalUp | LvTotalUploadDataAmount | 总上传 |
| TotalDown | LvTotalDownloadDataAmount | 总下载 |

持久化结论：`ui_state.json` 的 `column_layout.order/widths/visible` 全部以 ExName 为键（`profiles_controller._persistColumns`），与 title 无关。`test/ux_space01_column_persistence_test.dart` 实测：改宽（Remarks +40）、隐藏（StreamSecurity）、换序（Port 后移）后，用同一 store 重建 controller，宽度/显隐/顺序全部按 key 还原，且 `widths` 中不含中文标签键。故重开列偏好不受改标题影响。

## 4. 入口 / action ID 可达表

被移出顶部文字条的动作用户流程不变，逐项核对原版/现存可达入口（本卡不改菜单结构，未实现项只登记）：

| 原工具栏文字动作 | 现可达入口 | action / ID | 状态 |
|---|---|---|---|
| 添加（节点） | 菜单 配置项→添加 [VMess/VLESS/…]（11 项） | ACT-MAIN-001..011 | 可达 |
| 添加自定义/策略组/链式/出站 | 菜单 配置项 | ACT-MAIN-012..015 | 可达 |
| 编辑节点 | 右键“编辑” / Ctrl+D / 双击（DoubleClick2Activate=false） | ACT-PROF-001 | 可达 |
| 删除 | 右键“移除所选 (多选)” / Backspace、Delete | ACT-PROF-002 | 可达 |
| 复制 | 右键“克隆所选” / Ctrl+C | ACT-PROF-004 | 可达 |
| 启用/停用 | 右键“设为活动” / Enter | ACT-PROF-005 | 可达 |
| TCPing | 右键“测试延迟 Tcping (多选)” / Ctrl+O | ACT-PROF-016 | 可达 |
| 真延迟 | 右键“测试真连接延迟 (多选)” / Ctrl+R | ACT-PROF-017 | 可达 |
| 测速 | 右键“测试速度 (多选)” / Ctrl+T | ACT-PROF-019 | 可达 |
| 混合 | 顶部 `toolbar-混合` / 右键“混合测试” / Ctrl+E | ACT-PROF-015 | 可达 |
| 快速真延迟 | 顶部 `toolbar-快速真延迟` / 右键“快速真延迟” | ACT-PROF-014 | 可达 |
| 停止测试 | 顶部 `toolbar-停止测试`（仅运行中）/ Esc | ACT-PROF-038 | 可达 |
| 移除无效 | 右键“按测试结果移除无效” | ACT-PROF-021 | 可达 |
| 自动列宽 | 顶部 `toolbar-自动列宽` | menuProfileAutofitColumnWidth | 可达 |
| 双击激活 | 菜单 设置→主界面→切换双击激活（UI-DBLCLICK）；参数设置“显示”页 DoubleClick2Activate | UI-DBLCLICK / FLD | 可达 |
| 备注（快捷改名） | **仅**顶部 `toolbar-备注` | 无原版菜单项 | **需菜单侧配合**：右键菜单现无重命名项，建议在 UX-CTX 菜单下补“编辑备注”，之后本工具栏图标方可移除 |
| 显示列设置 | 顶部 `column-settings-button` + 菜单 设置→主界面→显示列设置 | UI-COLUMNS | 可达 |

## 5. 截图路径

- 前后对照：`before/old-left-panel-two-row-toolbar-run03.png`、`before/old-layout-run04.png`（来自 `context-menu-review-2026-10-03` 的修复前基线，窗口 1184×761）
- 修复后真实窗口（集成，合成数据）：`real-window/01-800x600-vertical-light.png`、`02-1200x800-vertical-light.png`、`03-1920x1080-vertical-light.png`、`04-1200x800-horizontal-light.png`、`05-1200x800-tab-light.png`、`06-1200x800-vertical-dark.png`、`07-800x600-vertical-dark.png`
- 修复后 release 真实窗口（独立 exe，PID 管理，无交互）：`real-window/08-release-1200x800-vertical-light.png`；记录 `real-window/release-captures.json`（pid=43876，1200×800，DPI=96→100%，用时 3861ms ≤120s，仅结束自身 PID）。脚本：`capture_release.ps1`。
- 测量原始数据：`real-window/observations.json`

## 6. 门禁结果

- `dart format --output=none --set-exit-if-changed lib test`：通过（0 changed）。
- `flutter analyze`：No issues found。
- `flutter test` 逐文件（`tools/flutter_test_retry.ps1 -PerFile -MaxAttempts 8`）：PER-FILE PASS（全部文件绿色；个别次尝试命中已知 `flutter_tester` 原生崩溃 exit 79，重试通过）。
- `flutter build windows --release`：成功（最终为 `flutter clean` → `pub get` → release，产物可直接启动；此前的 release 产物被中间 debug 集成构建污染，导致 AOT 识别失败，已重清重建）。
- release exe 启动验证：`capture_release.ps1` 启动 Release exe，DPI=96（100%），3861ms 内完成窗口截图并仅结束自身 PID；启动后工具栏为单行、表头中文化，见 `08-release-...png`。
- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --locked -- -D warnings` / `cargo test --workspace --locked`：全部通过。
- 真窗口集成证据 `ux_space01_layout_evidence_test.dart`：3 次尝试后 `recordingComplete=true`（前两次 engine exit 79，未完成记录被覆盖重跑）。

## 7. 未验证与遗留

- **DPI**：仅在当前系统 DPI（100%）下截取；125% / 150% **未运行**。
- **大字号**：仅默认字号；`UIItem.CurrentFontSize` 放大组合未测。
- **满交叉矩阵**：截图覆盖 3 尺寸 × 3 布局 × 浅/深（各因子至少一次），未做 3×3×2 全交叉。
- **业务流程前置未完成**：`frontend-journey-2026-10-02/README.md` 记录的“新增/更新后丢分组、隐藏选择、切组后操作对象”等行为缺口**由其他任务负责**，本卡未修复也**未宣称 verified**；本卡只保证顶部入口与表头结构、尺寸、可达性与列持久化。
- **备注入口**：见 §4 最后两行，需菜单侧配合，当前登记为阻塞项。
- 集成截图为 `setSurfaceSize` 设定的逻辑表面（真实窗口内渲染，pixelRatio=1），未对 OS 窗口边框做物理缩放；与 `context-menu-review` 的方法一致。
- 本卡未 commit（按任务要求）。
