# 用户流程、界面和性能实施细则

状态：identified；本文仅为设计，未修生产源码、未运行新产品测试。应用基线 a7aa0a5；原版7.25.4/7d6a967。事实依据为 [UI审计](../../evidence/complete-port-audit-2026-10-06/ui/README.md) 和冻结源码，未实际对照原生窗口的行为明确留待验收。本文由根整合者完成，UI方案子代理因用量限制中止，没有把其未交付内容当作已审查结果。

## 1. 修改范围和共享写锁

主要文件：`apps/desktop/lib/features/settings/settings_window_host.dart`、`option_setting_window_entry.dart`、`settings_actions.dart`、`settings_controller.dart`；`features/routing/routing_actions.dart`、`routing_windows.dart`、`dns_window.dart`；`features/subs/import_persistence.dart`、`subs_actions.dart`；`features/profiles/profiles_controller.dart`、`profiles_page.dart`、`profiles_table.dart`、`context_menu.dart`、`command_context.dart`、`ui_state_store.dart`；`features/monitor/connections_view.dart`、`monitor_controller.dart`；`app/shell/status_bar_view.dart`；`bridge/bridge_port.dart`；`windows/runner/option_window_host.cpp`、`routing_window_host.cpp`。

共享接口与 `bridge_port.dart`、settings/runtime controller 由整合者单写。UI代理先完成专属窗口/表格模块；真实 Rust API 接口由对应提供方实现再接入。已经生成的 `previewImportText/commitImportText` 不重新发明一套同义实现，先核验签名与当前行为。

上游必读：`ServiceLib/ViewModels/ProfilesViewModel.cs`、`StatusBarViewModel.cs`、`RoutingSettingViewModel.cs`、`ServiceLib/Handler/ConfigHandler.cs`，WPF `Views/MainWindow.xaml`、`StatusBarView.xaml(.cs)`、`ProfilesView.xaml(.cs)`、`ClashConnectionsView.xaml(.cs)`、各对应设置窗口；Avalonia目标读取同入口的Avalonia实现。固定commit不变。

## 2. 独立窗口：传输、保存和生效各自有结果

对应CP-05/09，SP-11/12。

现有 main_channel 不存在也返回 Success、回调异常不 reportOutcome、completer 无限等待、保存闭包冻结旧 revision，是不同层的漏洞，不能仅给 Future 加 timeout。

拟接口沿用主方案的 `WindowRequestEnvelope/WindowSaveOutcome/SettingsSaveReceipt`（当前未实现）；路由操作有对应 domain save receipt。具体实现步骤：

所有保存、查询、重试和回包还携带datasetEpoch，整库恢复后旧epoch作废；保存状态包含commit_unknown/recovery_required，需先恢复对账，不能作为未写盘失败重放草稿。普通mutation不换数据代次，mutationId才逐次新建。

1. 主窗打开子窗时生成 windowId 和递增 windowGeneration，带只读 snapshot/domainRevision/presentation。子窗验证 schemaVersion、必要字段、数据类型、读取结果，真正空库通过明确 `loaded=true/items=[]` 表达。
2. 子窗保存分配 requestId/mutationId，进入 Saving；同一待确认提交不因双击生成两次业务操作。C++验证 main_channel 存在，运输失败返回结构化 `TransportUnavailable`。
3. 主 Dart回调覆盖解析、保存、apply和错误转译，try/catch/finally确保发送 Outcome。不可读草稿不进入业务层。
4. 保存成功更新主窗和子窗当前 revision；core/platform失败保留 SaveSucceeded 的事实。再次确定如有新草稿按 newRevision 保存，无新草稿直接 retryApply。
5. 等待期限由协议配置且测试可控，到期进入 Reconcile，query mutation/operation。已成功则消费 receipt；执行中继续有界观察；明确失败才允许新尝试；查不到也不能悄悄重放非幂等写入。
6. 关闭窗口清理 Dart pending 和native引用；业务操作若已接受继续由Rust管理，结果在主窗可见。关闭不冒充取消已提交保存。旧generation/request的回包不得改变重开的窗。

状态：Ready→Saving→Saved/ApplyFailed/Failed/Reconcile；Reconcile可转Saved/ApplyFailed/Failed，不能永久busy。按钮可用性只来自此状态，不能在 finally 一律设成功。

故障合同：主回调抛错、缺channel、保存落盘失败、保存成功/apply失败、ACK丢失、Outcome丢失、迟到、重复、关窗、主窗退出、重开、两窗并发 stale。现有 `native_reply_loss_test.dart` 和 settings_retry合同先转绿；新增真实HWND生命周期验收，并实际重开SQLite确认仅提交一次。

## 3. 路由：保留原版即时提交，不回写陈旧全集

对应CP-08，SP-13；错误快照属数据安全阻断。

拟 `RoutingSnapshotEnvelope` 带schemaVersion/domainRevision、每方案读取状态和完整规则；`RoutingMutationReceipt` 带mutationId/newRevision、persisted、apply结果和必要的新快照（拟新增）。

1. list方案或任一rule读取失败时，整个相关编辑流程为LoadFailed；显示错误/重试，禁止确定/增删。不能 `page.ok ? rules : []`。
2. 子方案确定/删除/设默认仍在原版对应时机提交。保存成功后刷新权威snapshot和revision，apply失败单独反馈“已保存，未生效”。
3. 多选删除可采用Rust业务批事务以保证一批一致；若维持原版逐项业务合同，则返回成功ID/失败ID并立即同步已成功项，再重查。不能让第一次成功后的UI仍保留旧行。
4. 主窗确定仅保存原版主窗负责的策略字段；关闭/取消按Modified决定reload，不再次把 `_schemes` 全量写入。其它窗口新增的方案不能被删除，已删除方案不能复活。
5. reload失败仍保留新revision；下一次retry只应用已保存内容。schema未知/非法JSON保持只读，不猜成空库。

验收：合法空库、读取失败、畸形schema、第1/2/n项删除失败、子方案保存后reload失败、后台并发增删、确定/取消/Esc/标题栏关闭/重开。`routing_partial_commit_test.dart`正确期望转绿，再真实FRB/SQLite跑部分故障并比对导出与重开。

DNS已修先校验全部JSON的行为保留。后段存储失败的分段提交既有原版背景，不虚报为已证明差异；SP-12/23明确保存阶段或引入应用业务批事务，不能给用户只返回统一false并隐去前段已保存。

## 4. 导入：真正的纯预览与一次提交

对应CP-10，SP-14。

1. `BridgePort`接现成generated `previewImportText`，替换preview旧 `importFromText(subid:null)`路径；pure preview的Rust结果不能材料化Custom文件、节点ID或写DB。
2. 预览固定输入来源/目标group ID/解析结果；确认时整个批次用现成 `commitImportText` 或经核验的parsed-batch事务接口，All和有组共用。不能再每行 `saveImportedProfile`。
3. 如果当前commit重新解析导致ID/结果漂移，先增加preview token/hash校验，确认候选与展示一致；不要凭注释认为新API已经满足这一点。
4. Custom文本先受控staging，DB提交与最终文件发布有epoch/journal关系。预览/取消零持久化；失败清理仅本批owned文件；崩溃重开可恢复，不删其它节点引用的文件。
5. 返回批次saved/skipped/invalid统计和结构化失败。来源IsSub语义按原版入口；不能把有组手工节点全部改成false。含非法行时按冻结原版候选规则一次提交合法集，明确显示被跳过行，不能当数据库失败吞掉。

实际验证：All/普通组/订阅组，1/10k，Custom/Outbound，取消、重复确认、第二条DB失败、文件rename失败、DB提交后崩溃、完整重开。假的第二条失败合同仅故障注入，验收另用真实SQLite故障点和正式UI。10k导入时能取消/显示进度，UI不逐行同步写。

## 5. 当前组与原版入口

对应CP-14，SP-16/17。

通过application保存canonical `SubIndexId`，profiles_controller build读取；不是只让本地state存住。组删除后fallback All，保存失败可见，列表刷新保留合法ID。

`profiles_page.dart`的新增订阅直达空白SubEdit，编辑当前订阅读取当前group稳定ID后直达其SubEdit；All下门控按冻结原版。新增取消零写入，保存后当前组变化按原版。总列表仍保留独立入口。

底栏运行摘要来自 `RuntimeActualDescriptor`，不从active/selection推导。ACT-STAT-004的availability入口按冻结 `PreviewMouseDown` 实际绑定核对，不能看到函数名DoubleClick就自行绑定双击。失败切B而旧A仍运行时底栏必须仍表示A；改备注、重开、停止也不混淆。

消息新结构拟带source/operationId/time/severity/validGeneration。新操作反馈能显示，旧平台错误单独保留诊断，不永久优先盖住shell消息；不可通过清空仍有效故障假称平台已恢复。

## 6. 节点右键与选择键鼠核对表

对应SP-18。已有即时选择和target冻结不回退。以下属于真实WPF对照清单，未实测项不能提前标verified：

| 场景 | 要固定的合同与证据 |
|---|---|
| 右键已多选中的一行 | 保留哪些选择、主目标是谁、菜单命令作用集合；冻住ID，数据刷新不换目标 |
| 右键未选中的一行/空白 | selection变化与可用动作按原版；空白不能复用旧目标 |
| 左单击/Ctrl/Shift/立即Enter | 事件时捕获modifier；无额外300ms选择等待；键盘动作读即时选择 |
| 双击/Enter/F5/设置默认 | 单击不启动；各入口selected/default/actual目标不混用；原版幂等保留 |
| 滚动/失焦/窗口失活/切组/删除目标 | 主菜单及子菜单关闭、anchor有效性、迟到命令门控逐项记录 |
| 主菜单Esc/子菜单Esc/方向键 | 原版逐级还是整链关闭必须真对照；当前整链Esc不足以证明一致 |
| 高DPI/屏幕边缘/跨屏 | 菜单边界、submenu方向、target锚点、焦点返回不漂移 |
| 执行中节点排序/订阅更新 | 命令使用冻结ID，删除目标后按明确不存在结果，不错操作下一行 |

采用合成节点、双方同布局/窗口尺度，截图与逐事件trace脱敏记录。每一菜单动作对应ACT-PROF ID核对可用条件、快捷键、确认、取消、核心效果；不能只拍主菜单标题。

## 7. 底栏、间距、主题与连接表

SP-19：读取WPF DockPanel分区和原版800px最小宽。改成有界left/right加弹性middle，左选择器按原版受限宽，长摘要ellipsis+tooltip，二级错误详情不撑主行。文字与控件间距从统一density token出发，验证中文/English及200%DPI，不盲目缩小字体挤布局。800/1200/1280，100/125/150/200%，亮暗与长文本下关键左右控件初始同屏；横向scroll到达不算同屏通过。

子窗snapshot加presentation `{themeMode,brightness,accent,fontFamily,fontSize,locale,textScale,dpi}`，产品文本接资源key。不要每子窗再启动Rust业务engine。主设置改变后窗口更新/重开时机按原版，严格区分文字缩放与物理DPI。

SP-20：连接表保留现有筛选、关闭选中/全部、自动刷新和generation保护。用可视行构建代替全DataTable；接canonical ConnectionsColumnItem（FLD-CFG-136）宽/序号/auto-size与退出持久化；按上游补排序和右键close入口。异步关闭失败保留行且可重试；会话切换迟到结果不能关闭新会话同名connection。

10k合成连接只是性能夹具；最终另用真实内核监控API生成并读取连接，校验选中close/closeAll/断线/重连/停止事实。列布局保存后独立重开，不仅同一widget重建。

## 8. 异步数据链与稳定选择

对应CP-15，SP-21/22。当前SQLite有分页，但UI同步循环读全表；绘制虚拟化不解决这个阻塞。

拟 `QueryProfilesPageAsync`合同见主方案；用FRB异步Rust入口或清楚的worker，不能 `Future.sync` 包装同步重活冒充搬离UI。SQLite连接/锁设计须允许取消或丢弃迟到结果且不泄漏事务；滤排序在Rust并验证索引/执行计划。

分页采用稳定排序+ID tiebreak；cursor绑定datasetRevision/filter/sort。变更时保留按ID的选择、focus和可见锚点，明确重查；请求generation不同则旧response丢弃。total可以独立轻量异步取，不能为了精确count先全表反序列化ProfileDTO。首屏字段最小化，完整编辑按ID取详情。

overlay以ID增量存储，对可见行更新，150ms合并；不再每次全base合并/排序/深比较。预取有内存上界，缓存键带revision；100k全局全选以查询选择集合/排除集表达，不能悄悄只操作loaded页。导出/批删/测速等获取全选ID由后台实现，故障取消保持原语义。

连接、代理组、日志、订阅解析分别记录批次/背压。日志为有界环形buffer，UI丢弃中间显示更新可接受但最终错误/生命周期不能丢；用户明确导出日志另走持久化用例。实际流量不经过Flutter/FRB。

## 9. 执行卡与验收

| 卡 | 唯一流程 | 最先转绿的当前合同 |
|---|---|---|
| SP-11 | 独立窗保存回复丢失后确认结果 | native_reply_loss两项、异常reply/关窗隔离 |
| SP-12 | 保存成功、生效失败后重试 | settings_retry/newRevision/autostart分阶段 |
| SP-13 | 路由部分提交失败后关闭 | routing_partial_commit + 畸形快照 |
| SP-14 | All导入确认失败或取消 | ungrouped_import_failure + Custom预览no-diff |
| SP-16 | 当前组重开与编辑 | group_reopen/真正组G往返 |
| SP-17 | 失败切换时查看真实节点与当前结果 | actual descriptor/旧消息不遮盖 |
| SP-18 | 右键及快捷键正常操作 | 原版逐场景对照，不把fake当原生 |
| SP-19 | 最小宽主窗与子窗阅读操作 | status_viewport同屏 + presentation |
| SP-20 | 连接表管理和列重开 | column持久化/close结果/虚拟绘制 |
| SP-21/22 | 大库切组筛选并取消 | UI timer/真实DB分页/迟到响应/overlay |

以上合同只能证明对应行为；完整正式包SP-30仍逐185动作/53窗口核对入口、错误、权限、确认、取消、重开。详细性能采样和门槛看主方案及SP-31，不把本轮测得的单次同步2981.659ms直接当p95。

未运行：本分册产品测试、真实HWND/原版右键、高DPI、14核监控、正式包性能。当前状态identified，执行后必须按实测补证据。
