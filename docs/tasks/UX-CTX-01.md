# UX-CTX-01 — 节点右键定位、关闭和焦点

状态：`identified`。这是后续执行模型的修复任务卡，本轮只准备任务与证据，没有修改应用实现。

任务 ID：UX-CTX-01

本次唯一用户流程：用户选中节点A→右键打开菜单→左键点击另一行或按Esc退出→继续操作节点表；菜单必须在正确位置出现，退出后不能留下旧菜单或丢失本应保留的选择。

前置任务及已验证证据：
- 应用源码基线 `d7fbe80`，冻结上游 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始实施前记录实际HEAD和工作树差异，不能沿用旧基线冒充当前。
- `docs/evidence/context-menu-review-2026-10-03/README.md` 与 run-02/run-04/run-08：右偏171逻辑像素、点击另一行不退出、Esc清空选择却不关菜单已实测；完整专项未通过。
- run-07：独立的右键→编辑→取消→再编辑另一节点流程通过。修复不能破坏这一条。
- 原版菜单事件时机、原生窗口失活与子菜单Esc退层还没有实机证据，需分别补测。默认框架行为推断不能作为已验证原版事实。

对应 feature / field / action / layout ID：
- `LAY-PROFILES-004`：节点右键菜单。
- `ACT-PROF-038`：无菜单时表格Esc停止测试的原版语义。
- `ACT-PROF-001`：编辑作为菜单命令退出的正向控制场景。

必读上游文件、符号和固定 commit：
- 方案 §19、仓库AGENTS.md、四份compat台账的相关项。
- 冻结commit中的 `v2rayN/v2rayN/Views/ProfilesView.xaml`（DataGrid.ContextMenu）、`ProfilesView.xaml.cs::LstProfiles_PreviewKeyDown`、`ServiceLib/ViewModels/ProfilesViewModel.cs` 的选择和动作绑定。
- 当前 `profiles_table.dart::_showContextMenu/_onKey/_buildContextMenu`，`profiles_controller.dart::handleRightTap/handleKeyEvent/clearSelection`，`table_actions.dart::actionForKey`。
- 锁定Flutter SDK：`widgets/raw_menu_anchor.dart` 的open坐标说明与TapRegion；`material/menu_anchor.dart` 的定位、焦点及MenuItemButton关闭行为。按本机3.47.5读取，不凭其他版本文档设计。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入为鼠标坐标、按钮/修饰键、当前可见行ID和选择集合；传入位置必须明确属于global还是anchor local。
- 输出为唯一菜单的打开/关闭状态、正确命中位置和焦点；此任务不生成配置、不启动内核、不改变默认节点。
- 不存在行、数据刷新导致目标失效时关闭菜单并保留可解释的选择；不能用另一个行下标替代目标。
- Esc在菜单打开时优先处理退出；此次按键不得流到表格停止/清空选择。菜单关闭时保留原版快捷键。子菜单退层策略先补原版证据，不由模型猜测。
- 鼠标离开菜单不等同于取消；进入子菜单不能误关整链。点击菜单外的表格区域必须结束旧菜单；该次点击是否继续选中另一行依据冻结原版实机记录。
- 菜单位置/开关状态不持久化；退出后不能在重开窗口时自动恢复旧菜单。业务数据与运行状态本任务不修改。
- 无系统权限申请，无宿主系统代理/TUN/自启写入，不碰10808，不停止外部进程。

允许修改的模块：
- `apps/desktop/lib/features/profiles/profiles_table.dart`；必要时新增专用共享菜单组件和最小UI菜单状态。
- `profiles_controller.dart` 仅限菜单与选择/焦点协作，不能顺手改业务动作；相关行为测试与本任务证据。

禁止改变的已有行为：
- 不改变原版菜单条目、顺序、层级、批量动作和快捷键；菜单结构恢复/分组移动另按专项报告后续任务做。
- 多选内右键保留选择这一已工作的行为不能退化；编辑取消不改节点、默认节点或分组。
- 不改Rust/FRB接口，不增加另一套IPC，不以删除菜单或全局吞Esc解决问题。
- 不改冻结source、outputs原件或台账分母；不写魔法延迟规避开关竞争。

测试夹具和原版预期：
- 使用本轮4/32条合成loopback节点与真实Windows Flutter窗口；UI持久化位置隔离，不用个人节点/订阅。
- 菜单位置、选择ID与焦点同时记录；覆盖未选中行、多选内行、另一行、行号、空白和窗口边界。
- 实测尺寸与源码合同分开。原版尚未验证的事件细节补实机证据后冻结；无菜单Esc必须对照原版已有实现。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test integration_test/context_menu_review_test.dart`、`flutter analyze`。
- 有意义的菜单焦点/命中回归测试；真实窗口运行本轮完整context场景及editor子场景。
- 点A右键→点B左键；点A右键→Esc；多选右键→Esc；重复右键；边角弹出；右键→编辑→取消→另一节点右键编辑。
- 先排查完整场景中的Debug原生访问违规，保存原始失败和根因验证；仅editor模式通过不能放行整个任务。发布门禁按AGENTS要求执行，性能不能用Debug成绩替代。

证据文件位置：`docs/evidence/UX-CTX-01/`；必须包含基线、运行命令/环境、观察JSON、实际截图、原版对照、失败与恢复记录。

完成条件：源码状态可审查；本任务全部实际场景完成且通过；菜单期间键盘不误触表格；坐标不依赖固定侧栏偏移；多选和编辑正向控制仍通过。未跑原生窗口失活等场景时必须明确未验证，不写成完整verified。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
