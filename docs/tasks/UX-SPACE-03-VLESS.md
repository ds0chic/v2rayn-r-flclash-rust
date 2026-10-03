# UX-SPACE-03-VLESS — 节点编辑表单留白

状态：`implemented`（100% DPI 真窗口实测通过；125%/150% DPI、最小窗口、其他协议/弹窗未验证，见执行记录与证据）。先完成一条VLESS编辑路径，其他协议和参数设置各自拆后续卡，不以此卡替代全部表单验收。

任务 ID：UX-SPACE-03-VLESS

本次唯一用户流程：用户右键VLESS节点→编辑→阅读/修改备注和连接字段→触发字段错误→取消→重新打开同一节点；标签、输入、错误与按钮清楚可读，取消后原值保持。

前置任务及已验证证据：
- 冻结上游commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，审查应用基线 `d7fbe80`。
- 专项run-07独立编辑/取消两次通过，实测备注输入框高22；未有该场景本轮PNG，因为暂停图像编码。
- 优先完成UX-CTX-01菜单退出/焦点修复；排查完整真窗口Debug原生崩溃。当前独立流程通过不能覆盖大字号/错误文字的未运行场景。

对应 feature / field / action / layout ID：`F-PROFILE-002`、`ACT-PROF-001`、`LAY-ADDSERVER-001`；字号设置引用 `FLD-CFG-075`，本卡不改其存储值域。

必读上游文件、符号和固定 commit：
- 方案 §7/§19、AGENTS、四份台账中VLESS编辑和字段条目。
- 冻结 `Views/AddServerWindow.xaml` 与ViewModel的VLESS可见字段/原版分组、ResUI标签；不得把通用Flutter表单现有顺序当成上游依据。
- 当前 `profile_editor_dialog.dart::_buildField/_section/_save`、`profile_fields.dart`、`profile_draft.dart`、共享theme的输入框/文本样式。
- 专项报告中的字体、字段/错误/分组间距契约。

输入、输出、错误、取消、权限、持久化及生效语义：
- 使用原始VLESS DTO和本地草稿；本卡改变呈现，不改协议字段含义、DTO或配置生成。
- 默认输入框先按34–36高、字段净间距8–12、分组间16进行实际截图试排；这些是建议起点，必须根据字形和原版结构验收，不当作上游测量事实。
- 明确标签、帮助和错误各自占据的空间；错误出现时下一字段不得被盖住，保存/取消按钮不能跑出弹窗。
- 用户字号真实传播到标签/内容/错误；不要只改Theme仍硬编码12。大字号与紧凑行高限制冲突时登记规格缺口，不缩小设置范围或裁文字。
- 取消和Esc依原版弹窗语义处理，关闭后恢复所选节点、分组与视口；未保存草稿丢弃，重进读出原值。
- 校验失败保留草稿与字段错误，不静默保存；本任务没有实际应用/启动内核流程，也不修改宿主系统代理。

允许修改的模块：VLESS表单的展示层、必要的共享字段/theme样式、相应行为/布局测试与证据。共享组件修改必须核对使用它的其他协议不会裁剪；发现语义问题登记另卡，不顺手重写Rust。

禁止改变的已有行为：上游适用字段、字段顺序/分组、保存和取消边界、默认值、敏感字段遮挡、原版入口、节点ID和revision合同；不把字段删除或折叠藏起当留白完成。

测试夹具和原版预期：合成VLESS节点，长中文备注、IPv6/长地址、错误端口、TLS/Reality字段、默认及大字号；不得用真实节点凭据。原版布局来源固定源码，仍须补相同窗口/DPI下的原版实际截图。

本次必须通过的命令/真实场景：
- Dart格式检查、flutter analyze、有意义的草稿取消与错误呈现测试、Windows真窗口场景和release构建；仓库门禁按实际改动执行。
- 右键节点→编辑→改备注/错误端口→保存被拒绝→错误可见且草稿保留→取消→重新打开，原值不变。
- 默认/大字号、100%/125%/150%DPI、最小窗口/默认窗口、浅深色；测输入盒、字形、错误与下一字段矩形，不能只测Text存在。

证据文件位置：`docs/evidence/UX-SPACE-03-VLESS/`，包含实测尺寸、原版映射、前后截图、取消重进结果、错误/大字号场景、命令及未验证项。

完成条件：该VLESS流程可读、完整且正确，输入高度/标签/错误无拥挤与重叠；取消后原值保持；共享样式没有造成其他表单明显回归。其余协议、参数设置和跨平台必须继续保持各自未验证状态。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

## 执行记录（2026-10-03，deepseek-v4.1-flash）

状态：`implemented`。完整证据见 `docs/evidence/UX-SPACE-03-VLESS/`。

改动文件（仅呈现层与共享表单/theme 最小样式）：
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`：标签改为左侧固定列（宽 148，长标签换行顶对齐）、字段净距 10、分组间 16、标签↔控件 12、`isDense` + 内边距使输入框默认 ~34 高；字号全部改由 theme 派生，删除局部 `const TextStyle(fontSize: 12)`；错误/帮助由 `InputDecoration.errorText/hintText` 独立占位。
- `apps/desktop/lib/shared/theme/app_theme.dart`：新增 `AppForm` 度量（controlHeight/fieldGap/groupGap/labelColumnWidth/labelControlGap/controlPadding）与 `contentStyle/labelStyle/sectionTitleStyle/errorStyle/helperStyle`。
- 新增 `apps/desktop/test/ux_space03_vless_editor_test.dart`（错误端口→保存被拒→草稿保留→取消→重开原值；默认/大字号度量；错误不盖下一字段；VMess/Trojan 抽查）。
- 新增 `apps/desktop/integration_test/ux_space03_vless_editor_test.dart`（真窗口度量 + 错误/取消/重开 + 浅深色 + 大字号）。
- 新增 `apps/desktop/integration_test/ux_space03_before_probe_test.dart`（HEAD 基线截图）。
- `docs/evidence/UX-SPACE-03-VLESS/`（README、observations.json、before/、6 张 PNG）。

实测（真窗口 1184×761，100% DPI）：备注输入框 22→34，标签↔控件 12，字段 10，分组 16，
错误独立占位且下一字段不重叠，保存/取消在窗内；大字号 20 时输入框 45、标签/内容 20、错误 19。
字号传播：`AppForm` 从 `textTheme.bodyMedium`（= `shell.fontSize` = FLD-CFG-075）派生，未锁 12。

门禁：`dart format`/`flutter analyze`/`flutter test`（逐文件）/`flutter build windows --release`/
`cargo fmt/clippy/test --workspace --locked` 全部通过。执行期间共享树被并行任务改到无法编译，
故门禁在 HEAD `25c907e` 的隔离 worktree 中跑本轮文件；worktree 已清理。

未验证/遗留：125%/150% DPI、最小窗口、水平/标签布局、其余 9 协议与参数/自定义/分组弹窗；
大字号输入框 45 > 34–36 目标登记为规格缺口（控件随字号增长，未裁文字、未缩设置值域）；
Debug 真窗口偶发 `flutter_windows.dll` 间歇原生崩溃（重试后完整通过，未定位根因）。未 commit。
