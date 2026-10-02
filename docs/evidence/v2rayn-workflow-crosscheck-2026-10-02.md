# v2rayN 前端跨元素工作流核对

审查基线：本回合实际可读 checkout `d7fbe80`。冻结上游：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，根目录 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。记录文件沿用父任务指定日期；续核对时环境日期为 2026-10-03。

状态：本文全部发现均为 `identified`。方法为只读源码、台账及现有测试核对；应用、内核、平台与真实用户流程均**未验证**，新测试**未运行**。没有修改应用源码、任务状态或 compat 既有条目。未读取用户配置、订阅内容、运行日志或其他秘密；未启动程序、占用端口、修改系统代理或停止进程。

本报告补充 `docs/evidence/frontend-behavior-review-2026-10-02.md`，不覆盖其 UX-001..009。已读根 AGENTS.md、实际可见任务卡 `docs/tasks/T01.md`，以及四份台账中的相关入口、字段、布局和功能。T01 只验证承载能力，并明确不验收后续业务。当前可见 `docs/tasks/` 仅有 T01；有关导入、运行、设置和测速的后续任务前置需要由后续执行回合恢复/定位，不能据 T01 给这些业务记 verified。

## 工作流覆盖与断点

| 用户操作链 | 静态断点 | 关联发现 | 实际验收 |
|---|---|---|---|
| 在 G2 中导入/手工新增 → 查找 → 刷新 → 重开 | 当前分组不进入新增/导入参数，刷新还绕过组过滤 | WF-001、原 UX-004 | 未验证 |
| 表格切 G2 → 更新当前组；或在订阅窗口编辑/更新 → 返回表格 | 当前组取自另一窗口；订阅窗口写入不通知表格模型 | WF-002、WF-003 | 未验证 |
| 选择 A → 设默认 → 再设 A；运行 A → 改默认 B | 默认命令被当成 toggle；默认 ID 没有运行切换请求/desired bump | 原 UX-001、UX-002 | 未验证 |
| 修改节点/设置 → 保存 → 立即应用 | 后端 desired 已增长，运行控制器仍用旧 snapshot revision | WF-004 | 未验证 |
| 设置草稿 → 保存 → 再编辑 → 应用；或改自启 → 取消 | 最后一轮草稿不提交；取消不能撤销已执行平台写入 | 原 UX-006、UX-007、UX-009 | 未验证 |
| 保存测速设置 → G2 内过滤 → 真延迟/混合/快速 → 停止 → 刷新 | 固定参数覆盖已保存值；空 ID 语义扩大目标；取消立即显示停止而不确认完成 | WF-005、WF-006、WF-009、原 UX-004 | 未验证 |
| 标签布局过滤 → 切信息页 → 回配置项；或切布局 | 控件被销毁，输入框空白但过滤状态仍存在 | WF-007 | 未验证 |
| 快捷切主题/布局/双击激活 → 开参数设置 → 取消 → 重开 | UI 独立存储/内存与 canonical 设置不一致，load 覆盖快捷修改 | WF-008 | 未验证 |
| 更新订阅 → 请求取消 | 更新结束后才拿到 job ID；现有测试取消的是已完成任务 | WF-010 | 未验证 |
| 最小化到托盘 → 恢复 → 重开 | 本轮只读了桌面集成链，未做平台验收；托盘状态、关闭策略和恢复仍需专项验证 | 待真实流程 | 未验证 |

## 新增跨流程发现

### WF-001：在当前分组里添加/导入的节点没有当前组归属

- 状态：`identified`。
- 源码链：`profiles_page.dart:284` 设置 `ProfilesState.groupSubId`；`subs_actions.dart:27`/`:82` 调 `importFromText` 时没传 `subid`；`bridge_port.dart:431` 默认 `subid=null`；Rust `crates/bridge_api/src/api/subs.rs:390`、`:429` 因此使用空组。`import_persistence.dart:38` 原样保存 DTO，未补当前组。手工新增同样在 `profiles_controller.dart:212` 创建 draft，`profile_draft.dart:13` 的 `subid` 默认空，`profile_actions.dart:21` 没补当前组。
- 合成复现：建立 G1/G2，在表格选 G2，导入一条新的合成分享 URI；再在 G2 手工新增 B。检查保存后的 `subid`、G2 节点数；随后刷新、重开、重新选 G2。
- 可推导结果：节点存入空组；因原 UX-004 的 reload 绕过组过滤，操作后可能暂时看起来已导入 G2，再次点 G2 又消失。单独修复刷新会直接暴露导入后当前组找不到节点。
- 真实预期：ACT-MAIN-016 的 scope 为当前 `_config.SubIndexId`（`compat/actions.yaml:566`）；冻结 `MainWindowViewModel.AddServerViaClipboardAsync:497` 传该值，`AddServerAsync:453` 同样赋值 `Subid`。
- 元素规则：当前组的权威 ID 必须同时驱动分组高亮、导入、手工新增、更新当前组；写入成功反馈包含真正保存数，写入后派生视图仍采用同一组。
- 修复注意：不能只给现有 `importFromText` 补当前 subid 而不核对后端行为。Rust `subs.rs:437-438` 对非空组调用 `replace_sub_profiles`，需确认其追加/替换语义，防止为修归属而替换现有节点。
- 测试要求：真实 Rust bridge + 独立数据目录，检查 G1 不变、G2 旧节点保留、新节点归 G2、刷新/重开后仍存在；覆盖导入部分保存失败，不以导入解析数代替实际保存数。

### WF-002：“更新当前订阅组”更新的是订阅设置窗口最后选中项

- 状态：`identified`。
- 源码：`subs_actions.dart:333-341` 读取 `SubsState.selected` 并将 `selected.id` 传给 update；表格分组由 `profiles_page.dart:284` → `profiles_controller.dart:362` 修改另一状态。`sub_setting_window.dart:64` 才修改 SubsState 的选择。
- 合成复现：订阅设置窗口点 G1 后关闭，表格点 G2，然后主菜单“更新当前订阅（直连）”；捕获本轮请求目标集合。在从未打开订阅设置时，表格即使已选 G2 也会提示先到设置窗口选订阅。
- 可推导结果：目标为 G1，或没有目标；表格所显示的当前组不能决定该命令。
- 真实预期：ACT-MAIN-022/023（`actions.yaml:737`、`:763`）及冻结 `MainWindowViewModel:188`、`:192` 使用 `_config.SubIndexId`；`ProfilesViewModel.SubSelectedChangedAsync:336` 把表格选组写入这个值。
- 元素规则：命名为“当前组”的命令从表格当前组取得目标；若当前为“全部”，依冻结行为明确处理空组，不把另一窗口的历史选择作为 fallback。反馈标明本轮真正目标。
- 测试要求：G1/G2 更新前后版本不同，真实 bridge/公开合成订阅服务验证仅 G2 更新；覆盖未打开订阅窗口、全部组、删掉当前组、窗口选择与表格选择不同。

### WF-003：订阅窗口成功写入后，主窗口分组/节点仍读旧模型

- 状态：`identified`。
- 源码链：`SubsController.save/delete/update`（`subs_controller.dart:104`、`:123`、`:152`）只 reload 自己的 items；`sub_setting_window.dart:160` 的右键更新直接调该控制器。主表格 `GroupsPanel`（`profiles_page.dart:248-252`）只 watch `profilesControllerProvider`，且节点数量来自其旧 `state.profiles`。`openSubSettings`（`subs_actions.dart:297-300`）关闭窗口后未刷新 profiles；profiles 控制器没有订阅 `subscriptions_updated`。菜单更新 wrapper 才在 `subs_actions.dart:311` 补一次表格 reload，因此两个入口结果不同。
- 合成复现：记录主表格 G1 数量，在订阅设置内新增 G2/改名 G1/右键更新 G1，待成功后关闭；检查分组名称、数量和节点表，再手动刷新比较。删除当前 G1 后检查仍存在的 `groupSubId`。
- 可推导结果：数据库和订阅窗口已有结果，主窗口保留旧分组/旧节点，直到无关 profiles 变化触发重建或 reload；删除后的当前组也未协调。
- 真实预期：冻结 `MainWindowViewModel.SubSettingAsync:568-570` 在提交成功后 `RefreshSubscriptions`；导入成功 `:500-501` 刷新订阅及节点。一个持久化事实应映射到各入口一致的视图结果。
- 元素规则：订阅提交/更新产生统一失效信号，分组列表、计数、当前组、可见节点、选择集合一起从后端重新派生；关闭窗口不需要用户额外刷新才看到成功结果。
- 测试要求：真实 bridge 完成各操作后立即断言主窗口模型及可见元素，与重开数据库结果一致；删除当前组必须进入明确 fallback，不能留下无高亮但仍在过滤的幽灵当前组。

### WF-004：保存成功后“应用”仍提交保存前的 desired revision

- 状态：`identified`，强化原报告“应用读取旧 revision”的风险；没有真实复验。
- 源码链：`RuntimeController.applyActive`（`runtime_controller.dart:101`）只取自身最近 snapshot；`_onEvent:43-49` 仅接受四种 runtime 事件。节点保存在 `crates/application/src/engine.rs:294` 增长 desired；设置保存在 `:898` 增长 desired。`crates/bridge_api/src/api/engine.rs:493-506` 和 `settings.rs:1294` 保存链没有触发运行控制器 refresh 的相应事件。Flutter `profiles_controller.dart:249` 的 reload 只更新 profiles，`settings_controller.dart:124` 只应用 UI。实际应用最终在 `application/src/engine.rs:1040` 校验 expected revision。
- 合成复现：启动后取得 runtime snapshot rev R，编辑活动节点并保存，或改合成测试端口保存；不做手动 snapshot 刷新，立即点主窗口或设置窗口的“应用”。
- 可推导结果：保存后后端 desired 为 R+1，应用仍使用 R，在有效配置下会走 stale revision 拒绝；界面可能尚未显示“未应用”。
- 真实预期：desired 与 applied 分离且由 Rust 拥有；保存产生的新 desired 必须可被 UI 观察。应用以已确认的最新存储版本为前置，不应要求用户通过无关操作更新 snapshot。
- 元素规则：保存成功→更新 desired 状态→显示待应用→对当前存储 revision 提交应用→显示后端结果。保留并发校验；stale 应引导重新读取而不自动吞掉。
- 测试要求：真实 bridge 验证编辑/导入/设置/路由/DNS→立即应用，检查生成配置与最终 desired/applied 配对；合成故障注入 stale 时草稿和可重试路径保留。不要用测试里预先手动 refresh 代替用户链。

### WF-005：保存的测速设置被每次启动测试的固定常量覆盖

- 状态：`identified`。
- 源码：设置页 `_speedTestTab`（`option_setting_window.dart:635-670`）编辑 `SpeedTestItem` 的超时、两个 URL、并发、IPAPI、UDP 目标；`ProfilesController.startSpeedTest`（`profiles_controller.dart:612-621`）却每次调用 configure：pageSize=1000、并发=10、超时=10、两个固定公开 URL、IPAPI=null、UDP=null、delay=1。Rust `speedtest_configure`（`crates/bridge_api/src/api/speedtest.rs:298-307`）直接替换当前运行配置。
- 合成复现：将 timeout 改 20、mixedConcurrency 改 15、URL 改测试服务地址保存；启动真延迟/混合/下载测速，检查后端有效参数和测试服务请求。
- 可推导结果：保存/重开能看到新设置，但本轮测试继续使用旧常量，IP 目标等也被清空。
- 真实预期：FLD-CFG-106 等测速字段运行期读配置（`fields.yaml:3930-3961`）；冻结 `SpeedtestService:12-13` 读取分页/间隔，`:521-522` 读取 URL/超时。
- 元素规则：设置控件保存的值就是下一轮测试的有效参数，只有缺省/null 才采用冻结 fallback；UI 不重复拥有测速默认规则。
- 测试要求：真实 Rust 配置读取与有效参数断言，必要时用公开合成 HTTP 服务验证 URL/超时/并发生效；覆盖保存→重开→测试，不能只断言 settings JSON roundtrip。

### WF-006：分组/过滤内测速扩大到全库；未选普通测速也扩大到全库

- 状态：`identified`。
- 源码：`profiles_controller.dart:625` 对 Mixed/Fast 一律传空 ids；普通测试未选中也传空。Rust `speedtest_start`（`speedtest.rs:350`、`:367-381`）查询全库并把空 ids 解释成全部 stored nodes。UI 函数注释 `profiles_controller.dart:607` 则声称 Mixed/Fast 使用全部 visible。
- 合成复现：G1/G2 各有两节点；选 G2，过滤为只显示 B1，点混合/快速；再清空选择点 TCPing/真延迟。检查实际 test_nodes 集合。
- 可推导结果：G1 与隐藏 B2 被提交；普通测速无选择也测试全库。后续 150ms reload 还触发原 UX-004，造成组高亮/可见集进一步不一致。
- 真实预期：冻结 `ProfilesViewModel.ServerSpeedtest:715` 对 Mixed/Fast 使用当前 `ProfileItems`，该集合由 `RefreshServersBiz:363` 按组/过滤产生；普通测试使用选中项 `:719`，空集合直接返回 `:722-724`。台账 ACT-PROF-014/015 写“全部节点”，必须连同上述当前集合上下文解释，不应把它扩大到整个数据库。
- 元素规则：当前分组/过滤下的“全部测试”为明确可见集合；“选中测试”为明确选择集合，空选择禁止提交。前后端不以含糊的空集合同时表示“无目标”和“全库”。
- 测试要求：真实 bridge 记录本轮请求的准确 ID 集合，覆盖各组、过滤、空集合、隐藏选择；`t15b_speedtest_test.dart:51-57` 目前只断言空 ids，未验证后端实际目标，不能沿用该断言作为兼容性证明。

### WF-007：跨页后过滤框空白，表格却继续按不可见过滤条件显示

- 状态：`identified`。
- 源码：`SideTabs.build`（`side_tabs.dart:51-54`）按当前 tab ID 替换 content；回到配置项会创建新的 `ProfilesPage`。`profiles_page.dart:26` 的 TextEditingController 默认空，`:56` 在 dispose 时销毁，`:106` 没从 `ProfilesState.filter` 恢复。profiles provider 是普通 NotifierProvider（`profiles_controller.dart:22`），跨页仍保留 filter/selected/group 状态。
- 合成复现：使用标签布局，输入能命中少量行的过滤字，选其中一行并滚动；切信息/代理页再切回配置项。也覆盖通过主菜单切布局导致 ProfilesPage 替换。
- 可推导结果：输入框为空且滚动位置重置；表格仍经过旧过滤，选择状态可能继续存在。再次 Enter 读取的是 state.filter（`profiles_controller.dart:404`），空白输入框不等于已清空条件。
- 真实预期：冻结过滤绑定是同一 ViewModel 的 ServerFilter（`ProfilesView.xaml.cs:45`）且查询由 `ProfilesViewModel.RefreshServersBiz:363` 使用同一 `_serverFilter`。至少要保证可见文本与实际筛选条件一致；精确滚动/焦点保留仍需冻结真实窗口对照。
- 元素规则：控件生命周期不能产生不可见查询条件；页面重建时恢复已生效的文本，或同时清空模型与展示并遵守确定策略。选择、焦点和滚动保持/重置需要明确，不靠 widget 偶然生命周期。
- 测试要求：标签往返、三布局往返后同时检查过滤文本、实际可见 ID、当前组和 selected 集合；做真实 UI 输入/跨页，不能仅 controller.setFilter。

### WF-008：快捷主题/布局/双击行为与参数设置的存储源冲突

- 状态：`identified`。
- 源码链：`UiShellController.setLayout`（`ui_shell_controller.dart:285-287`）保存独立 UI layout section；toggleTheme（`:308-313`）保存独立 theme section。`ProfilesController.toggleDoubleClick2Activate`（`profiles_controller.dart:491-496`）只改内存。打开参数设置会 `settings.load`（`option_setting_window.dart:52`），随后 `_applyImmediate`（`settings_controller.dart:165-174`）把 canonical UiItem 应用到 shell 和 profiles；`ui_shell_controller.dart:222-233` 重设 layout/theme。重启 bootstrap 同样加载 canonical（`app.dart:80`）。
- 合成复现：通过主窗口切为深色/标签布局，打开参数设置后取消；或工具栏打开双击激活，再打开参数设置后取消；最后重开。
- 可推导结果：快捷切换没有写 canonical 设置，打开窗口的 load 即覆盖快捷值；双击行为突然改变，重开也恢复旧值。UI 独立 store 即便有保存，也可能被随后 canonical load 覆盖。
- 真实预期：冻结 `OptionSettingViewModel:361`、`:371` 写 `UiItem.DoubleClick2Activate`、`MainGirdOrientation`；FLD-CFG-077（`fields.yaml:2845-2875`）有 canonical 存储合同。新增快捷入口若承诺修改相同偏好，应同步该源或明确仅本次窗口状态。
- 元素规则：同一偏好的各入口显示同一已提交值；打开/取消其他窗口不撤销此前已提交的快捷选择；重开恢复同一源。不能将一次用户动作保存在一个文件、从另一个源恢复。
- 测试要求：真实 bridge + 独立 UI store 完成快捷修改→开设置→取消→重开，断言同一 preference；覆盖分别从参数页/主题页/工具栏修改，不能单独测试每个 provider persistence。

### WF-009：测速可多次并发启动，停止只取消最后一个且立即宣告停止

- 状态：`identified`。
- 源码：`profiles_page.dart:125-144` 测试按钮始终有 onPressed，没有 `speedTestRunning` 门禁；`startSpeedTest`（`profiles_controller.dart:609-633`）每次覆盖唯一 speedTestJobId。`cancelSpeedTest:651` 忽略取消返回值，立即取消 poller、设置 running=false/`SpeedtestingStop`（`:652-657`）；Rust `speedtest.rs:466-469` 只取消传入 ID 对应任务。轮询结束判断 `profiles_controller.dart:196` 又是全局 activeJobs==0，没有逐轮对应。
- 合成复现：启动长测试 J1，在其运行时再启动 J2，随后按“停止测试”或 Esc；检查全部本应用任务的状态和结果流，及 UI 提示。
- 可推导结果：唯一记录变为 J2；停止入口只给 J2 的 ID，J1 可能继续。界面即刻停止轮询并隐藏运行指示，即便 J1 或 J2 的取消尚未确认。旧任务可继续写结果而 UI称停止。
- 真实预期：冻结 `SpeedtestService.ExitLoop:34-37` 快照全部 run token，随后取消全部；用户的“停止测试”针对其当前测试流，不能有未呈现且继续运行的任务。
- 元素规则：启动/排队/替换必须是明确策略，当前任务不可被新点击无声覆盖；停止应显示“正在停止”，等待后端确认目标任务完成/取消，再转停止。失败留可操作状态。
- 测试要求：真实 job registry 覆盖连续双击启动、不同测试类型并发、Esc、停止失败、取消期间新结果和迟到完成事件；检查所有目标均结束，不能只断言 mock 收到 cancel 或本地 running=false。

### WF-010：订阅更新的取消接口拿不到运行中的本轮 job ID

- 状态：`identified`；当前窗口没有可见取消按钮，因此这是任务合同缺口，不声称当前用户能从按钮复现。
- 源码：`SubsController.update`（`subs_controller.dart:164-167`）await 返回后才记录 lastJobId；Rust `update_subscriptions`（`subs.rs:315`）先创建 job、`:323` 等更新结束、`:324` finish 后才在 `:330` 返回 job_id。cancel（`subs_controller.dart:183-185`）只读 lastJobId。`sub_setting_window.dart:75-97` 没有取消元素。
- 合成复现：通过真实 bridge 提交一个等待合成服务响应的更新，在运行期间调用该取消入口；首次无 job，第二轮可能拿上一轮 ID。随后检验本轮 job 是否仍运行。
- 可推导结果：首次 cancel 无操作；后续 cancel 针对上一轮；待 await 结束拿到 ID 时任务已 finish。`t09_sub_setting_test.dart:87-96` 等 update 完成后再 cancel，仅能验证转发，无法证明运行中取消。
- 真实预期：冻结 ACT-MAIN-022 等并无显式取消入口（`actions.yaml:740-743`），不能新增假取消并将其算作已完成能力。若当前设计保留 cancellable job，job acceptance/ID 必须先于运行完成可观察。
- 元素规则：取消按钮只对后端确认的当前可取消 job 可用；提示精确区分 requested、cancelled、已完成、过安全点；异步异常要保证 busy 清理和错误可见。
- 测试要求：真实延迟订阅服务，在请求运行中取消并确认没有提交新节点、旧节点保留；覆盖首次/重复/第二轮取消、越过安全点；mock 仅注入异常，不能当作取消成功证据。

## 执行记录与后续前置

- 实际命令：`Get-Content`（上述源码/现有证据/任务卡）、`rg --files`（限定 lib/test/docs/tasks/compat/冻结源目录）、`rg -n`（限定具体源码/台账）、`Get-Location`、`Get-ChildItem docs/tasks -Name`、`git rev-parse --short HEAD`、`git status --short`、`Test-Path`。成功定位上述静态链；针对猜测测试文件名的首次 rg 有文件不存在提示，已改为枚举实际 test 文件。未扫描 target/build，未运行测试或构建。
- 改动文件：仅新增本报告；既有 `frontend-behavior-review-2026-10-02.md` 未修改。
- 未完成：冻结窗口的实际行为对照、所有真实 UI/Rust/持久化/重开/配置效果链；平台最小化/托盘/退出/恢复；性能测量；所有修复。
- 下一步前置：恢复/定位相应业务任务卡；建立独立合成数据目录及公开合成订阅/测速服务；真实内核测试前探测 ≥11808 端口；不得触碰 10808 或宿主系统代理。每一批以真实目标集合、数据库重开结果、有效配置、job/runtime确认状态为断言，保留失败路径和并发约束。
- 完成度：本轮未给出用户流程总体比例。源码存在、UI 可点击、动作日志、静态分析或局部单测均不能换算为真实全流程通过。
