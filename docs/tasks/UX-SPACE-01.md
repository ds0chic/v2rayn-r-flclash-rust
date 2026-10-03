# UX-SPACE-01 — 恢复原版节点区入口和表头

状态：`implemented`（结构/尺寸/表头/列持久化已验证；业务流程前置缺口未修复，相关流程未 verified）。

任务 ID：UX-SPACE-01

本次唯一用户流程：用户在原版节点区顶部选择订阅分组→过滤节点→读取节点列表→使用原有快速测试入口；入口位置、展示顺序与文字间距保持原版结构，控制条不能挤占两行额外文字操作。

前置任务及已验证证据：
- 冻结上游commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，当前审查应用 `d7fbe80`。
- 专项run-02/04/08测到工具栏高84，表格左缘x=171；原版LAY-PROFILES-001明确顶部分组和少量图标操作。节点表行高26是实测，不能靠把它抬到48改善拥挤。
- 当前新增/更新后丢分组、隐藏选择问题在 `frontend-journey-2026-10-02/README.md` 有实测。视觉修复不能顺便掩盖这些行为缺口；依赖修复未完成则验收对应流程保持未验证。
- 开始前记录真实HEAD/工作树；同一页面和共享theme不与菜单事件修改并行落地。

对应 feature / field / action / layout ID：
- `LAY-PROFILES-001`：顶部工具栏/分组容器。
- `LAY-PROFILES-002`：列定义、默认顺序与展示标签。
- `ACT-PROF-014/015` 的快速/混合测试保留原版入口；本任务不重新实现测速后端。

必读上游文件、符号和固定 commit：
- 方案 §7/§19、AGENTS.md、相关台账。
- 冻结 `ProfilesView.xaml:24..99`、列定义与ResUI.zh-Hans.resx；Windows以冻结WPF布局为本卡依据，其他平台不得无说明互套。
- 当前 `profiles_page.dart::_Toolbar/GroupsPanel`、`profiles_models.dart` 的column key/title、`profiles_table.dart::_headerCell`、`AdaptiveToolbar`、`AppTokens`。
- `docs/evidence/context-menu-review-2026-10-03/README.md` 的尺寸和排版约束。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入为已有订阅、当前group ID、过滤文字、窗口宽度、列偏好和字号；输出为对齐原版的可达入口与清楚表头。
- 保持原版顶部分组→编辑/新增订阅图标→200宽过滤框→自动列宽/快速/混合图标的层级和顺序；低宽度按原版Wrap处理，不额外复制一长串文字命令。
- 从新增工具栏移走的动作必须仍在原版菜单/右键/快捷键入口可达，不能删功能。
- 展示标签用上游本地化资源，内部ExName/column key继续用于持久化、排序与FRB字段；不能把中文标题当稳定存储键。
- 布局调整和取消不写节点、默认节点、订阅或运行状态。列偏好与窗口布局仍按原有持久化语义保存，重开不得恢复出错布局。
- 无系统权限或平台网络副作用；测试不得触碰10808、宿主代理、自启或TUN。

允许修改的模块：`profiles_page.dart`、列展示模型、`profiles_table.dart` 的表头呈现、必要的共享工具栏/theme；相关布局回归与证据。组过滤后端语义、测速逻辑和Rust不在本卡范围。

禁止改变的已有行为：默认列顺序/宽度键、原版菜单动作、选择和过滤语义、三种主窗口布局、快捷键、原版功能分母；不增加新的主页导航，不以隐藏大量动作声称颜值完成。

测试夹具和原版预期：两组合成订阅、中文长组名、长备注和IPv6节点、已有列偏好；冻结source提供结构和默认资源值。参考图必须标明原版实机/源码结构/当前实测，不能拿概念图当原版截图。

本次必须通过的命令/真实场景：
- Flutter格式检查、analyze、相关布局测试、Windows release构建及仓库门禁。
- 800×600、1200×800和宽屏；三布局；中英文资源；浅深色；100%/125%/150%DPI，未运行组合明确登记。
- 顶部选择A→过滤→清空过滤→选择B→回全部；入口操作目标正确、行与表头不重叠，菜单里的全部原版动作仍可达。
- 记录控制条实际矩形与折行数量，不能只核对toolbarHeight token。重开验证列偏好不因改标题丢失。

证据文件位置：`docs/evidence/UX-SPACE-01/`，保留原版结构映射、前后相同窗口尺寸截图、入口/action ID可达表、列持久化与真实场景记录。

完成条件：LAY-PROFILES-001/002的结构与资源映射可核对；当前额外侧栏/文字条造成的布局偏离消除；原有功能和列偏好保留；实际用户路径稳定，视觉没有遮挡/贴边/误读。业务流程前置未完成时不得给完整verified。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

## 执行记录（2026-10-03，HEAD 25c907e，未 commit）

- 实施：profiles_page.dart 恢复顶部 WrapPanel（订阅分组 chips → 编辑/新增订阅图标 → 200 宽过滤框 → 自动列宽/快速真延迟/混合图标），移除左侧分组栏与两行文字条；profiles_models.dart 展示标签本地化（key 仍 ExName）；app_theme.dart 新增工具栏几何 token。
- 尺寸实测（真实窗口，real-window/observations.json）：宽屏单行 38 高、800/水平布局按 Wrap 折为 2 行 72 高；表头 30；表格左缘 x=0；过滤框 200。
- 列持久化：ux_space01_column_persistence_test 证明 width/visible/order 按 ExName 存储并跨重开还原，改中文 title 不影响。
- 门禁：dart format / flutter analyze / 逐文件 flutter test / build windows --release / cargo fmt+clippy+test 全通过。
- 遗留：DPI 125%/150%、大字号、3×3×2 全交叉未测；备注入口仅本工具栏，需菜单侧配合；新增/更新丢分组、隐藏选择等行为由其他任务负责，未 verified。
- 证据：docs/evidence/UX-SPACE-01/（README.md、before/、real-window/）。

