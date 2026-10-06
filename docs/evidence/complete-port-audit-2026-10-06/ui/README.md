# 完整稳定移植复审：用户流程与界面（2026-10-06）

状态：`identified`。当前复审基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。不以旧审计的问题数、当前测试总数或“implemented”状态计算完成率。

本文 ROOT 为 `C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn/`，UP 为 ROOT 下 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。下面每个源码路径和行号均是当前基线读取结果，完整绝对路径由 ROOT/UP 前缀解析；本目录的 `findings.json` 提供机器可读绝对定位。

本代理没有改生产源码、冻结源码、台账分母或原有测试，没有启动原生程序、内核，未操作系统代理、路由、TUN、自启/全局热键、剪贴板或真实用户数据。新增测试均为内存 bridge、假 host 或 MethodChannel 故障注入。测试执行由根代理顺序安排，7 项正确预期均明确断言失败；首轮加载异常和夹具错误不计产品失败，重跑日志已保存。

## 结论与证据层级

顶部启动目标、选择延迟、右键目标/菜单关闭、有组手工导入重复提交、订阅来源保留、DNS 草稿碰撞、路由多选、热键暂停派发门等已有实际代码修复。旧审计中这些问题不能原样重复报告。All/无组批导入和Custom纯预览仍有独立缺口，见 UI-11。

但是当前仍有数据危险的路由提交合同、丢回复永久等待、当前组不能恢复、主状态栏可视范围和运行节点身份缺失，以及若干原版入口/列行为/子窗口呈现未迁移。目标“完整稳定移植”尚未满足。

## 确认问题与具体修复合同

### UI-01 / P1：路由规则读取失败仍成为可提交的空规则，快照校验不完整

状态 `identified`。源码确认；原生 SQLite 故障链未验证；合成快照拒绝测试见 `native_reply_loss_test.dart`。

- 当前 ROOT `apps/desktop/lib/features/routing/routing_actions.dart:53-64`：对每个路由方案调用 listRoutingRules；`page.ok=false` 转为 `rules=[]`，并正常打开窗口。ROOT `apps/desktop/lib/features/routing/routing_windows.dart:1539-1549` 从 rules 重建 ruleSet；`routing_actions.dart:115-125` 全窗确定逐方案 save，会把该方案真实规则覆盖为空。窗口拒绝 `{}` 的新守卫并未阻断这种非空但不完整的快照。
- 当前 `routing_windows.dart:1570-1585` 对非法/未知 schema 默认空 map→空方案，`1743-1749` 仅检查空字符串/`{}`，并不验证 schemes 类型/必要字段/每方案读取成功。
- 原版 UP `ServiceLib/ViewModels/RoutingSettingViewModel.cs:84-105` 从实际模型刷新列表，`:119-139` 单方案读取后再打开子编辑器；原版主窗没有“无法读取规则便将其作为删除全部规则保存”的入口合同。
- 复现：合成方案 A 含规则→规则读取返回存储错误，或独立窗口 ready 返回 `{"unexpectedSchema":true}`→窗口允许编辑/确定。期待：明确读取失败、禁止写入并可重试；当前：可以生成正常空草稿。
- 最小修复：版本化 snapshot envelope 带 revision、所有方案与规则的读取结果；任何读取失败不能构造可写草稿，schema 错误不能降为空库。主提交只接受合法、完整、与 opening revision 一致的命令；已有真正空库仍允许添加。
- 验收：规则读取失败、总列表读取失败、ready 丢失/非法 JSON/未知 schema、合法空库逐项测；所有失败路径持久化 no-diff，重开保持原规则。

### UI-02 / P1：增量路由操作后，旧全量草稿可以复活已删方案或撤销已提交编辑

状态 `identified`。源码确认 + 合成用户流程故障注入，测试见 `routing_partial_commit_test.dart`；真实 FRB/SQLite 未验证。

- 当前 ROOT `apps/desktop/lib/features/routing/routing_windows.dart:2100-2115` 多选删除逐个 commit，第二项失败立即 return，第一项已成功提交但窗口仍保留所有旧行。`:1931-1954` 保存成功但运行 apply 失败也统一 `ok=false`，窗口不重读已提交状态。
- 当前 `routing_windows.dart:2190-2199` 最后“确定”仍发送全量 `_schemes`；`routing_actions.dart:112-132` 对旧草稿每一行再 save，并删除不在草稿的当前方案，能把先前删除的旧行再写回，或删除“保存成功/应用失败”时新建但 UI 未纳入的新方案。
- 原版 UP `ServiceLib/ViewModels/RoutingSettingViewModel.cs:153-163` 删除后 RefreshRoutingItems，`:110-115` 主窗 SaveSettingsAsync 仅写域名策略；方案在各子编辑器提交后刷新，不是最终关闭时全量重写所有方案。
- 复现：选 A/B→删除→A 已删、B 存储失败→错误提示→用户按确定关闭。期待：准确显示 A 已删，关闭不复活 A；当前合成 host 可观察旧 A 仍在窗口，确定又携带 A。
- 最小修复：native commit host 的主“确定/关闭”不再次提交全量方案；拆分 persisted 与 applied 结果，操作后以真实 backend 新快照校正窗口。批量操作做 Rust 用例事务，或返回逐项已提交集合并即时同步，不能一律假装全部未改。
- 验收：第 1/2/n 项失败，保存成功/apply 失败，重载失败，后台并发新增/删除，确定/取消/关闭/重开；均无旧行复活、新方案误删或已提交字段反向覆盖。

### UI-03 / P1：独立参数/路由窗口丢失回复时永久等待

状态 `identified`。源码确认 + MethodChannel 丢回包故障注入，测试见 `native_reply_loss_test.dart`；真实 HWND/engine 故障未验证。

- 当前 ROOT `apps/desktop/lib/features/settings/settings_window_host.dart:81-94`、`apps/desktop/lib/features/routing/routing_windows.dart:1685-1700` 对主 engine 保存回调未 try/finally 转失败结果；回调抛异常便不 reportOutcome。
- 当前 `settings_window_host.dart:157-170`、`routing_windows.dart:1753-1786` 保存/commit 的 completer 无完成期限和结果查询；saveDraft 方法成功返回后若 saveOutcome 丢失，Future 永远不完成。`windows/runner/option_window_host.cpp:71-77` 与 `routing_window_host.cpp:72-78` 即使 main_channel 不存在仍回 Success。
- 原版同 engine 对话框无需本项目新引入的跨 engine 回包；新架构必须补完请求生命周期，不能让这个架构差异成为用户永久卡住的原因。
- 复现：ready 正常，saveDraft 传输 ACK 正常，主保存回复不发送；等待 31 秒仍 pending。期待：进入“结果未知、正在校对/可重试”并终止忙碌状态；当前：一直等。
- 最小修复：request ID + window generation + bounded response + operation status 查询。主回调异常统一返回结构化结果；channel 缺失立即失败；超时先 reconcile，不能自动重放已可能提交的写入。窗口关闭清理 pending，旧回复不能污染重开窗口。
- 验收：主回调异常、主 channel 不在、响应丢失/延迟、双击确定、保存途中关闭/主退出，均无永久 busy、重复写、旧回复串窗。

### UI-04 / P2：当前订阅分组没有进入持久化配置，也没有重开恢复

状态 `identified`。源码确认 + 当前组持久化合同测试，见 `group_reopen_test.dart`；测试在 canonical SubIndexId=null 的失败断言停止，后续控制器 invalidate 恢复断言未执行，构建不读 SubIndexId 由源码确认；真实关闭/重开未验证。

- 当前 ROOT `apps/desktop/lib/features/profiles/profiles_controller.dart:365-384` build 不读取 Config.SubIndexId；`:866-874` setGroupSubId 只更新临时 state，不保存 canonical settings。全部代码搜索未发现另一个把此选择写回 SubIndexId 的生产消费者。
- 原版 UP `ServiceLib/ViewModels/ProfilesViewModel.cs:334-338` 更新 `_config.SubIndexId`，其后刷新/导入/当前订阅操作共用该对象，RefreshSubscriptions 恢复该配置。
- 复现：选组 A→关闭/重建控制器→重新打开。期待：组 A，导入/当前更新仍以该组为目标；当前：All。
- 最小修复：application 提供 current group 读写用例，保存 Config.SubIndexId，失败反馈；构建时从持久化配置恢复，组被删除时回退 All；临时 selected/primary 不存为 active。
- 验收：A/B 同名组、空 URL 普通组、删除当前组、写失败、原版配置迁移、保存→重开→导入/更新目标一致。

### UI-05 / P2：底部控件已有改善，但正常最小宽度仍依赖横向滚动

状态 `identified`。源码确认 + widget 几何检查，见 `status_viewport_test.dart`；真实高 DPI 截图未验证。

- 当前 ROOT `apps/desktop/lib/app/shell/status_bar_view.dart:163-171` 中部并非弹性区域；`:298-316` 全部控件+服务摘要+速率在 SingleChildScrollView 中顺序排列。实际代理模式完整长文字、额外规则模式、TUN desired/actual 同列，占宽未限，右侧速率可能在初始视口外。此前“无 overflow、ensureVisible 后可点”测试不能证明同时可见。
- 原版 UP `v2rayN/Views/MainWindow.xaml:14-16` 默认宽 1200、最小宽 800；`StatusBarView.xaml:22-103` DockPanel 右停靠速率、左停靠两行端口/TUN/160 宽选择器，中部服务信息占剩余区域。
- 最小修复：有限宽左右分区 + Expanded 中部摘要，选择器固定/受限宽度，超长节点/路由名省略+tooltip，诊断和二级反馈不扩主行。800px/100–200% DPI 下同时看到主控件与两行速率，不能靠拖横向条找速率。
- 验收：800/1200/1280px、100/125/150/200% DPI、长中文合成标签、错误状态、亮暗主题；初始视口内同时可见，不只检查 ensureVisible 后的结果。
- 严格边界：横向滚动是当前有意设计；本项属于与原版“左右同时显示”的布局和用户体验差异，不能宣称控件功能丢失。新增测试先记录初始越界控件，再逐项 ensureVisible 检查滚动后可达性，分别保存结果。

### UI-06 / P2：底部无法确认实际运行哪个节点，原版运行信息测试入口缺失

状态 `identified`。源码确认；真实切换节点/失败保留旧会话未验证。

- 当前 ROOT `apps/desktop/lib/features/runtime/runtime_bridge.dart:40-77` 读模型没有 applied profile identity/摘要，`:142-149` statusLabel 只返回“运行中 PID/端口”；`status_bar_view.dart:163-170` 因而不能展示实际 A/B。不能临时从 profiles.activeId 推导，因为 B 保存成功/应用失败时旧 A 可能仍在运行。
- 同一中部仅普通 Text/Column，无运行信息 MouseDown/点击测试派发；`ACT-STAT-004` 有台账但当前该入口无接线。
- 原版 UP `ServiceLib/ViewModels/StatusBarViewModel.cs:270-271` 使用 running.GetSummary；`v2rayN/Views/StatusBarView.xaml.cs:15-16,94-96` 为两行运行信息派发 TestServerAvailability（源码绑定 PreviewMouseDown；函数命名为 DoubleClick，但本段没有 ClickCount 判断，执行模型应按实际源固定点击语义）。
- 最小修复：runtime snapshot 带 applied profile ID 和脱敏摘要，由真正会话创建/commit 写入。恢复原版运行信息测试入口，共享安全后台 use case。
- 验收：A 运行→B 成功，A 运行→B 失败保留 A，改备注未应用、无 active、停止/恢复/重开；底部/托盘/主表可分清选择对象与实际会话，测试结果有明确反馈。

### UI-07 / P2：节点页“编辑当前订阅”和“新增订阅”仍打开同一个总列表

状态 `identified`。ROOT `apps/desktop/lib/features/profiles/profiles_page.dart:118-128` 两按钮同调用 openSubSettings。原版 UP `ServiceLib/ViewModels/ProfilesViewModel.cs:862-881` 分别读取当前 `_config.SubIndexId` 或新建 SubItem，直接打开 SubEdit。

复现：组 A 已选→编辑当前订阅，应直接编辑 A；新增应是空白新增表单。当前两者均先进入总列表，当前目标与用户意图脱节。最小修复：明确 editCurrentGroup/createGroup 入口，使用稳定 ID 和原版保存/取消合同，All 下编辑按原版无操作/门控。验收含 A 页/B 总列表旧选择、All、普通组、新建取消与保存后当前组变化。

### UI-08 / P2：独立设置/路由窗口没有继承主题、字体和语言

状态 `identified`。ROOT `apps/desktop/lib/features/settings/option_setting_window_entry.dart:19-23` 与 `features/routing/routing_windows.dart:1844-1848` 两个新 engine 固定 buildAppTheme(Brightness.light)，无父窗口 presentation 配置。原版窗口共用主题资源/字体与文化设置（UP `v2rayN/Views/RoutingSettingWindow.xaml.cs`、`OptionSettingWindow.xaml`）。

复现：主窗暗色/指定字体/English→开两子窗。当前呈现默认亮色，产品字段也大面积硬编码中文（主窗部分菜单/状态栏已经本地化，不应宣称完全没本地化）。最小修复：只读 presentation envelope，共享主题/font/locale 和产品资源 key；不启动第二套 Rust engine。验收亮暗、accent、font family/size、适用语言、跨 DPI、关闭重开。

### UI-09 / P2：Clash 连接表的列行为、右键和绘制虚拟化没有完整迁移

状态 `identified`。ROOT `apps/desktop/lib/features/monitor/connections_view.dart:228-274` 为嵌套滚动 DataTable，全量遍历 rows；列为常量、无 onSort/拖列/列宽保存恢复、无原版右键菜单和自动列宽入口。现有过滤、关闭选中/全部和自动刷新已有调用链，不能说整个连接功能不可用。

原版 UP `v2rayN/Views/ClashConnectionsView.xaml:64-79` 启用行虚拟化和右键关闭；`ClashConnectionsView.xaml.cs:19,43-55,70-125` 自动列宽、按 ConnectionsColumnItem 恢复宽度/序号并退出保存。对应 FLD-CFG-136、LAY-CLASHCN-001/002。

最小修复：统一虚拟表格/列布局模型接 ConnectionsColumnItem，恢复右键与自动列宽，列排序与焦点/选择按冻结源核对。大量连接全行构建是结构性风险，本轮没有测出真实时延，不能写成已测得某毫秒。验收字段排序/列拖宽/序号→重开，右键已选/未选/空白，10k 合成连接计时与截图。

### UI-10 / P2：旧平台提示持续遮盖后来导入/订阅结果

状态 `identified`。ROOT `apps/desktop/lib/app/shell/status_bar_view.dart:276-287` 只要 platform.message 非空，shell.message 不显示；platform clearMessage 无普通业务消费者。切过一次代理模式后，随后导入/更新写 shell.message 会被旧平台结果盖住。原版 Notice/运行信息以当前操作反馈刷新，没有该跨域长期遮盖合同。

最小修复：消息带时间/来源/严重性，最新操作结果可见；持久故障单独标记并可查看，不占据所有成功反馈。验收用 FakePlatform 产生旧消息→导入/订阅成功与失败→看到最新结果，无宿主代理写入。

### UI-11 / P1：All/无组批导入仍逐条同步写入；新事务/纯预览接口已生成但未接线

状态 `identified`。源码确认 + 合成第二条存储故障注入，见 `ungrouped_import_failure_test.dart`；真实 SQLite All 入口故障未验证。

- 当前 ROOT `apps/desktop/lib/features/subs/import_persistence.dart:88-90` 预览调用旧 importFromText(subid:null)，`:100-128` All/无组提交仍走 persistImportedProfiles；`:47-57` 为每行同步 saveImportedProfile + 每行读取 revision。用户默认 All 导入大量节点时会在 UI isolate 串行写库，后段存储失败留下前段记录，UI虽然明确报告 saved/failed，但“一次确认批提交”的稳定性/性能合同未满足。
- 当前 ROOT `apps/desktop/lib/bridge/bridge_port.dart:596-600` 旧 seam 直调旧 Rust import_from_text；`crates/bridge_api/src/api/subs.rs:602-603` materialize=true，`:438-466` Custom/Outbound 预览会创建/写 config 文件。文件按内容命名避免重复，却不等于取消/失败预览零持久化。
- 现成 ROOT `apps/desktop/lib/bridge/api/subs.dart:107,136` 和 Rust generated 已含 previewImportText/commitImportText；`api/subs.rs:580-590,640-678` 新纯预览/单事务（含无组）已经实现。Dart“等待下一次FRB生成”的注释已过期，新 seam 没被普通 UI 消费。
- 原版 UP `ServiceLib/Handler/ConfigHandler.cs:1697-1702` 合法候选一次 InsertAllAsync，再 SaveConfig；本项目原方案亦要求后台解析和批量事务。不能拿有组 batch 测试代替无组最常见入口。
- 最小修复：BridgePort 分清 parse-only preview / commit parsed profiles；所有目标组含空组统一新 commitImportText，避免第二次解析、新 ID 漂移和逐行同步写；Custom 文件材料化与 DB 事务用暂存+成功提交/失败清理合同，不让预览/取消留下文件。FRB 已生成，先接线与回归，不能继续以生成前置为阻塞。
- 验收：All/普通组/订阅组、1/10k节点、Custom配置、取消、第二条存储失败、commit失败、重开，均一批持久化、无未提交文件/半批数据；实际 FRB release 比较正常入口事件/帧耗时。旧 UI 返回 saved/failed 的诚实反馈仍保留。

## 稳定性候选与尚未证明的部分

1. DNS 确定依次 saveSimple→save Xray→save sing-box（ROOT dns_window.dart:785,811,838），后段 I/O 失败会保留前段写入。所有 JSON 先校验的修复有效；这个剩余问题是存储故障边界而非“非法后段文本导致半写”。原版也分步提交，不宜直接称冻结差异；完整稳定版仍应提供单事务/明确逐段已保存事实。真实故障注入未运行。
2. 节点绘制虚拟化已有，但 ROOT bridge_port.dart:433-447,467-485 仍同步全量读取完整 ProfileDto，profiles_controller.dart:365-369,551-568 构建/重载全列表，实时 overlay 每 150ms 对全 base 计算并比较。SQLite 分页查询确实存在，不能称数据库没有分页；问题是 UI 没有只加载可视数据窗口。历史 R4-09 Rust 只覆盖 10k、100k为 fake；本轮根代理补当前 release DLL+真实 SQLite，100k 已落库，独立 Dart VM 同500行同步分页循环读回耗时2981.659ms（见 `../frb-native-100000-observations.json`）。Flutter tester大读取异常退出不能证明发行 GUI 必崩；该单次耗时不等于启动总耗时/p95/GUI帧率。按生产UI isolate同步调用关系可推断该路径会阻塞其交互；真实桌面启动/切组/筛选/overlay与原版对照仍未验证。
3. 连接/代理组刷新 coalescing 和 session generation 已有保护；真实内核连接大量刷新、关闭失败/再试、会话切换时延迟回复仍需实际合成内核验收。
4. 节点右键 submenu 的 Escape 当前关闭整链（profiles_table.dart:387-399），当前源码自身注明 WPF 子菜单逐级 Esc 未真机对照。主菜单生命周期的 fake 覆盖充分，不足以称原生焦点/跨 DPI/子菜单键盘已全部对齐。
5. 全局热键编辑期间 dispatch 检查 `_paused` 已在 hotkeys.dart:545-550 实现，旧“只注销不设派发门”的问题已过时。同组合多动作的 OS 派发、冲突/取消实际注册仍未实测，本代理没有注册宿主快捷键。

## 已复核修复，不再重复列为现存缺陷

| 流程 | 当前实现证据 | 限制 |
|---|---|---|
| 顶部启动捕获所选目标，F5 使用 persisted active | profiles_controller.prepareStartTarget；main_shell runtime toolbar；r4_02_contract | 真正普通未武装包按钮→FRB→核心另需验 |
| sameactive 默认动作幂等且不伪称 running | profile_actions.dart:572-596，noop=true/applied=false | 显式启动/重试仍应真实验 |
| 选择即时提交，Ctrl/Shift 在 pointer 时捕获 | profiles_table pointer-down；r4_08_contract | 合成 widget，非真实鼠标/DPI |
| 右键已选保留多选，冻结主行/命令目标，滚动/失焦关闭 | profiles_table；r4_07_contract | 此次r4_07测试进程异常未完成，不能视为当前已验证；子菜单焦点/Esc 原生待验 |
| 有组手工导入一次 batch，来源 IsSub 区分 | subs_actions/import_persistence；Rust 订阅事务 | All/无组/Custom预览仍见 UI-11；真实 GUI 重开需验 |
| 当前订阅更新目标是节点页组，All→合法全部 | subs_actions.dart:448-459；r4_17_contract | 当前组持久化另见 UI-04 |
| DNS 文本以自身 controller baseline 保留脏草稿 | dns_window.dart:72-79 | 后段存储失败事务边界仍待验 |
| DNS 普通保存也重载 | dns_window.dart:858-866 | 错误结果需完整会话核对 |
| 路由 Ctrl+A/Delete/Enter 与单项即时提交 | routing_windows native editor；r4_14_contract | 增量 + 旧全量确定冲突见 UI-02 |
| 热键暂停有同步 dispatch gate | hotkeys.dart:545-550 | 真实 OS 注册/派发未验 |
| 底部选择器有边框箭头、两行端口/速率、诊断进详情 | status_bar_view.dart | 800px 同时可见仍见 UI-05 |

## 覆盖与验收范围

读取了 AGENTS、原方案布局/节点交互/性能/验收/任务格式章节，R4-01/02/06/07/08/09/11/12/14/15/16/17/19/20/21/22 任务合同，以及 compat 的 feature/action/layout/相关 field 定位。检查重点是用户入口与跨窗故障合同；没有声称逐个验完 158 个实体字段或每种远程协议。

本目录 `coverage.json` 枚举本次审查涉及的台账 ID（feature/action/layout/settings field），状态仍为 `identified`，集合只是 referenced_scope；选定 7 项新合成合同明确失败，不表示整个引用集合已逐项运行或验收，不抬升台账既有状态。全量 inventory 分母/字段效果矩阵由根代理和设置专项汇总。

完整稳定移植验收还需要：原版同合成数据逐事件真实 UI 对照；普通发布包的启动/缺核安装/重试/停止；FRB→SQLite 保存→重开→运行配置/实际平台效果；原生窗口请求异常；所有 applicable 设置消费者；真实 TUN 与 helper 生命周期；100k+ 节点/10k 连接 release 操作时延；高 DPI/主题/语言/托盘视觉；每项平台适用性与包版本哈希证据。未运行项目写未运行，未实测写未验证。

## 本轮命令与实际观察

根代理从 `apps/desktop` 顺序运行锁定 Flutter 的 `test --no-pub ../../docs/evidence/complete-port-audit-2026-10-06/ui/<文件> --reporter expanded`，避免与全量门禁并发。原始日志在 `../checks/audit-ui-*`，单项重跑日志也保留。以下不把未加载、夹具错误算作产品失败。

| 审计测试 | 当前实际观察 | 能证明/不能证明 |
|---|---|---|
| routing_partial_commit_test.dart | 明确合同断言失败；a删后仍渲染，按确定后假持久化集合 `{b,a}`，whole-draft save=1 | UI真实widget的旧草稿再次提交+源码真实写入路径；不能证明真实用户数据库已受此影响 |
| status_viewport_test.dart | 明确同屏断言失败；800px初始 routing-selector x=754.3..1038.8，速率 x=1231.0..1348.5；滚动后各项都在视口内 | 当前布局与原版左右同时显示不同；已证明可滚动到达，不是功能丢失 |
| native_reply_loss_test.dart | 首轮加载异常不计；重跑3项明确FAIL：两次保存31秒仍pending，未知快照schema被正常返回 | MethodChannel传输ACK/丢回复边界和解析器真逻辑；不是实际HWND故障 |
| group_reopen_test.dart | 首轮空URL夹具错误不计；修后明确FAIL：期望syn-sub-0，canonical实际null | 真控制器未存当前组；恢复断言未执行，源码未读SubIndexId确认；真实重开未验 |
| ungrouped_import_failure_test.dart | 明确FAIL：saved=1/failed=1，per-row writes=2，合成仓储before=0/after=1 | 同步逐行提交能半批成功；生产SQLite调用链确认，但没有真实SQLite注入本次I/O失败 |

本代理实际另执行：读取 AGENTS/规格/台账/上游与现有卡；`rg` 搜索与当前行号复核；`python .../ui/assemble_scope.py` 成功生成 11 项 findings 和 referenced scope；`git diff --name-only` 无生产改动，`git status --short` 仅本次新审计目录。未运行原生GUI、真实FRB库或平台效果；根代理的全量门禁及真实FRB/SQLite探针请以总报告为准。
