# 桌面与设置真实生效审计（2026-10-05）

结论：不能宣称设置已全部真实有效。当前除了平台效果尚未验证，还有确定的保存/失败重试缺陷、未接入生产消费者的字段以及多个状态存储源冲突。这不表示所有设置都无效；66 行中多项具有真实消费者，但必须逐个验证最终用户效果。此前分组测试通过只能证明相应层的合同，不能替代用户从界面保存到实际效果的验收。

审计基线：工作树 HEAD `672e666`，上游冻结 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。工作树已有的 `codegen.rs`、net_host/helper、发行信息未提交改动全部保留，本审计不修改产品代码、不构建/替换 dist。范围来自 `compat/fields.settings.yaml`，不是推测的控件列表。

## 范围与证据口径

本分册完整登记 12 类、66 个叶子字段，见同目录 `desktop-fields.csv`。Config 25 个容器行归总审计；剩余 89 个叶子字段归引擎分册。66 行包括：GUIItem 9、MsgUIItem 2、UIItem 17、ConstItem 4、KeyEventItem 5、CoreTypeItem 2、ColumnItem 3、ClashUIItem 8、SystemProxyItem 6、WebDavItem 4、CheckUpdateItem 3、WindowSizeItem 3。

CSV 状态为 `identified` 23、`implemented` 31、`blocked` 12。这里 `implemented` 只表示找到实际消费者；本次没有运行原生界面/注册表/远端/TLS 场景，不能提升为 `verified`，这些数字也不能当成完成百分比。`blocked` 内区分平台待验与缺少平台实现，详见 notes，不能统称“只是需要授权”。

执行约束：未启动真实应用，未写宿主代理、Run-key、TUN 或路由；未绑定任何端口，未访问用户数据和凭据。只运行以 `SyntheticBridgePort`、`CountingRuntimeBridge`、`FakePlatformBridge`、`MemoryUiStateStore` 完整替代生产依赖的两项故障合同。

## AUD-DESK-01：开机自启写失败后再点确定会假成功（P1，已复现）

路径：`apps/desktop/lib/features/settings/settings_controller.dart:204-218,246-261,315-332`。

第一次保存 `GuiItem.AutoRun=true` 时，先持久化 true，再写 Run-key。如果写入失败，第一次结果诚实报告失败；但控制器已保存 true。用户仍在设置窗口点确定重试，`previousAutoRun` 已是 true，条件判断跳过 `_writeAutostart`，最终可报告成功，实际系统自启仍未设置。

证据：`desktop_repro_test.dart` 第一个测试；`desktop_repro.log` 输出 `autostart attempts=1; second.ok=true`，正确合同要求 attempts=2 且再次失败，测试退出 1。全程内存平台 fake，没有写系统。

上游：`ServiceLib/ViewModels/OptionSettingViewModel.cs:397-405` 每次成功 SaveConfig 后调用 `AutoStartupHandler.UpdateTask`，不是只比较已保存布尔值。

修复要求：记录 desired 与实际平台应用结果，或每次提交均幂等对账，失败保留可重试状态；不能仅比较 JSON。补首次失败、同值重试、用户外部改 Run-key、写成功后重开/安装路径改变的合同。

## AUD-DESK-02：旧设置草稿能覆盖编辑期间的新状态（P1，已复现）

路径：`settings_actions.dart:25-33,53-58`、`settings_window_host.dart:64-72,157-175`、`settings_controller.dart:140-146`。

独立窗口拿到完整 JSON 快照，却没有携带打开时 revision。确定时主窗口使用实时 `state.revision`，把旧完整草稿伪装成最新版本提交。后台更新选项、托盘模式、其它窗口或恢复操作推进 revision 后，旧窗口仍能覆盖这些新值。

证据：第二个纯测试先取旧草稿，再 `saveGroup('UiItem')` 将语言改为 en，最后提交旧完整草稿。日志为 `old draft save.ok=true; language=zh-Hans`，正确合同应拒绝旧稿或合并只修改的字段，测试失败。这验证控制器算法；本次没有运行原生多窗口。

修复要求：editor snapshot 必须包含捕获的整树/分组 revision；提交按捕获版本检查。若采用字段 patch，必须同时保留未知键和未触及字段；备份恢复后旧窗口必须作废。不能自动用最新 revision 重试覆盖。

另一个组版本缺陷由总审计登记：Rust 完整保存推进所有组 revision，Dart `saveDocument` 未同步 `groupRevisions`，随后 `saveGroup` 可立刻报 stale。勿将本问题和组版本不同步合并，它们是两个相反方向的并发错误。

## AUD-DESK-03：系统代理实际失败不进入设置总结果；部分设置变更被去重（P1，前者已由总审计故障注入复现）

路径：`platform_controller.dart:44-48,235-244,343-363`、`settings_controller.dart:225-261`。

设置保存触发平台 listener，但 `syncAppliedMode()` 返回 void，设置总结果只检查 RuntimeView.error 和自启结果。系统代理/PAC应用失败可留在 PlatformView，而设置窗口仍得到 `ok=true`。总审计 `root_contract_repro_test.dart` 第四项已使用持续拒绝的内存平台 fake 复现：运行应用成功，PlatformView.error 为 `E_PLATFORM_BACKEND`，总结果却 `ok=true`；日志见 `root_contract_repro.log`，正确合同断言失败。此证据没有执行真实系统代理写入。

另外 `_syncKey` 只包含 mode/session/port，不含 SystemProxyExceptions、NotProxyLocalAddress、SystemProxyAdvancedProtocol、CustomSystemProxyPacPath。同一 session 同一 mode 改这些字段，listener 虽已增加，`syncAppliedMode` 仍可直接 no-op。完整 saveAndApply 若确实创建新 session 可能补救；因此不能泛化为“所有完整保存都无效”，但当前以同一会话保存组和幂等 apply 的合同存在明确缺口。

`_persistAndReturn`/`_persistMode`（`platform_controller.dart:390-428`）也忽略 `saveGroup` 结果。平台操作成功但模式持久化失败时，菜单结果和重开状态可能分离。修复必须分别报告持久化与系统效果，并将生效内容的 revision/hash 纳入去重；不能只靠 session ID。

## AUD-DESK-04：可保存但实际功能缺失的消费者（P1/P2，静态确认）

| 字段 | 当前链路 | 原版/修复要求 |
|---|---|---|
| GuiItem.EnableHWA（062） | 仅 domain/DTO/timing/UI；Windows runner 没有读取 | 原版 `v2rayN/Views/MainWindow.xaml.cs:151-153` 在启动时选软件渲染；需要启动前实际 Flutter renderer 选择和帧性能验收 |
| GuiItem.EnableLog（063） | 仅 domain/DTO/default；没有应用日志开关 | 原版 `ServiceLib/Manager/AppManager.cs:101` 控制应用日志；不得用 CoreBasicItem.LogEnabled 的内核日志代替 |
| GuiItem.RootCertProvider（064） | platform 选择 API 能映射 system/chrome/mozilla，但下载客户端未读 | 原版 `CertPemManager.cs:313-357` + `DownloadService.cs:171,267` 选择应用 HTTPS 自定义信任；要给订阅、更新、WebDAV 等请求统一接入 |
| MsgUIItem.MainMsgFilter / AutoRefresh（065/066） | 日志窗局部 keyword/autoRefresh 存内存，不读取/回写本组 | 原版 `MsgViewModel.cs:20-28,116` 从该组初始化/回写；保存→重开应保留实际过滤与刷新 |
| ClashUIItem.EnableIPv6 / EnableMixinContent（129/130） | `codegen.rs:461-477` 映射函数仅测试调用 | 生产 `engine.rs:2923-2948` 生成/复制 native custom body，没有调用 Mihomo merge；需要真实主计划接线 |

RootCertProvider 缺口不是“需要授权安装系统证书”。其上游语义是不改变系统证书库、也不改变内核证书校验。当前 `crates/subscriptions/src/download.rs:119-137`、`crates/updater/src/fetch.rs:36-49`、`crates/application/src/webdav.rs:110-126` 使用独立 Client builder，无 provider 参数。本字段的下拉选项能保存不等于改变 TLS 信任。R4-13.S10 README 将它描述为证书安装/OS blocked 的口径应纠正。

CoreTypeItem 两字段有真实独立消费者：`engine.rs:2777-2791` 直接查绑定，单节点显式 core 优先。虽然 `AppSettings::core_for` 只被测试使用，不能据此误判 CoreTypeItem 完全无效。

## AUD-DESK-05：原版窗口、分隔比例和表格列设置未接入实际状态源（P2）

涉及：UiItem.MainGirdHeight1/2、MainColumnItem、WindowSizeItem，ColumnItem.Name/Width/Index，WindowSizeItem.TypeName/Width/Height，ClashUIItem.ConnectionsColumnItem。

当前主壳读 `ui_state.json` 的 `layout.horizontal_split/vertical_split`（`ui_shell_controller.dart:189-207,350-356`），不读 MainGirdHeight1/2；节点列读 `column_layout`（`profiles_controller.dart:760-790,1788`），不读 MainColumnItem；原生启动窗口读取 exe 旁 INI（`windows/runner/main.cpp:46-83`），不读 WindowSizeItem。Rust 的 geometry helper 目前只有定义/测试，未连到 native。连接表使用固定 DataColumn（`connections_view.dart:232-244`），不消费 ConnectionsColumnItem 宽度/次序。

所以“MemoryUiStateStore 保存重开”证明的是另一份状态文档，不证明冻结原版配置的字段对当前界面有效。恢复/导入原版设置后窗口与列仍可能保留另一状态源。这里并未判定窗口完全没有尺寸恢复：当前 Win32 INI 本身能够记录/恢复尺寸，缺口是它不消费被迁移的 canonical WindowSizeItem，也未建立二者的迁移/优先级合同。

修复要求：建立唯一 canonical UI 设置及旧 ui_state/INI 一次迁移规则；明确哪个源优先，取消双向互相覆盖。验收从导入原版合成配置→实际显示尺寸/列顺序→拖动→canonical 持久化→彻底重开→备份恢复走全链，不只测存储对象。

## AUD-DESK-06：平台与外部来源仍有代码缺口，不能都归成环境未验证（P1/P2）

ConstItem.RouteRulesTemplateSourceUrl 非空时 `crates/bridge_api/src/api/routing.rs:407-427` 明确返回 `error.routing_external_template`，没有另一个异步下载/导入入口。字段实际改变行为为“拒绝”，不是实现了外部模板更新。需要异步 fetch、校验、事务化导入和取消，失败不覆盖既有路由。

MacOSShowInDock 没有实际 app activation policy 消费者。CustomSystemProxyScriptPath 在 UI 与 domain 保存、平台 API 只校验文件存在（`platform_service.rs:391-398`），未有 macOS/Linux 生产执行链。上游这项是 OSX/Linux 专属（`ProxySettingLinux.cs:20-36`、`ProxySettingOSX.cs`），Windows 原版不用，Windows 不能将其视作有效开关；移植项目要求的其它平台却不能当作已实现。

## AUD-DESK-07：界面状态赋值不等于界面消费（P2）

`UiShellController.applySettingsDocument` 将 EnableAutoAdjustMainLvColWidth 写进 `autoAdjustColWidth`（243），HideColumnIpInfo 写进 `hideIpInfo`（241），EnableStatistics 写进 `showStatistics`（242）；全 lib 搜索这些成员只有构造/复制/赋值，没有实际读取者。

因此自动列宽与隐藏 IP 信息开关没有控制节点表，统计总功能虽然引擎有消费者，但统计列显示合同未证实。表格的 `fittedColumnWidths` 会自行运行，不能证明自动列宽选项起作用。上游 `ProfilesView.xaml.cs:332-355` 使用 canonical 列配置与 HideColumnIpInfo 控制显示。

## AUD-DESK-08：独立设置窗口没有继承用户主题/字体/语言（P2）

`option_setting_window_entry.dart:19-25` 固定 `buildAppTheme(Brightness.light)`，未读取传入 snapshot 的 UI 主题、字体、字号与语言，也没有主应用 locale/翻译 delegates。主窗设置有主题消费者（`app.dart:31-57`），不能据此证明新开的独立窗口对齐。

主题 dialog 还提供 Aquatic/Desert/Dusk/NightSky，但 `_themeModeFrom`（`ui_shell_controller.dart:204-215`）把这些全部落到 system；冻结 Windows WPF 的 theme dropdown 只 `Take(3)`（`ThemeSettingView.xaml.cs:15`）。需按平台可用值准确展示，不能展示无法实现的模式。

## AUD-DESK-09：更新/监控开关的立即保存失败会被静默忽略（P2）

`update_controller.dart:175-188,191-209` 先更新 state，再 `saveGroup`，不检查结果，异常也吞掉。`proxies_view.dart:79-102`、`connections_view.dart:77-93` 的排序/刷新开关同样先改本地 timer/UI，忽略持久化失败。首次完整设置保存造成组 revision 不同步时，此路径更容易稳定失败。

影响 CheckPreReleaseUpdate、UpdateViaProxy、SelectedCoreTypes，及 ProxiesSorting/ProxiesAutoRefresh/ConnectionsAutoRefresh；当次操作可以使用本地新值，重开却恢复旧值，无可见失败提示。要求 save→成功才应用，或失败回滚且显式提示；不要把瞬时本地效果当作持久化成功。

## AUD-DESK-10：加载失败伪造成功与副作用说明失真（加载 P1；说明 P2）

`app.dart:116-121` 正常启动恢复 active 后总会 `restoreAppliedModeOnLaunch`；后者 mode=0 在没有应用中节点时也走 ForcedClear（`platform_controller.dart:311-322,296-299`）。这解释为何按现有数据打开真实包会修改宿主代理；当前审计未执行。按已保存代理模式恢复启动效果若符合冻结原版，本身不是移植缺陷；这里登记它是为了准确说明实际效果，并遵守仓库对本机审计禁止改宿主代理的约束，不能当成普通只读 UI 启动。

P2 产品误导：设置页 `option_setting_window.dart:1133,1183` 还声称系统代理/TUN“本页仅保存配置”，实际确定经过 `saveAndApply` 会应用 runtime，可能触发系统代理或 TUN。需明确“保存并应用”时机与失败结果，删去实现阶段占位话；不能让用户从说明推断按确定没有系统副作用。

P1 加载失败：异常仍伪造默认文档为 loaded=true（`settings_controller.dart:111-117`），注释“纯测试”不能限定生产 catch。总审计 `root_contract_repro.log` 第三项使用抛异常的合成 bridge 已复现 loaded=true，正确合同断言失败。`openOptionSettingWindow` 没检查 load 返回状态就打开旧/默认 draft。应加载失败显示错误、禁止提交直到真实读取成功；不要以默认覆盖或旧草稿维持看似正常的表单。

## AUD-DESK-11：找不到的自定义 PAC 路径被自动创建（P2）

`platform_controller.dart:161-174` 对任意 `selection.path` 缺文件会创建父目录并写默认模板，没有判断 `selection.isCustom`。冻结上游 `PacManager.cs:34-44` 只有自定义文件存在才使用；不存在时回退到 config/pac.txt，再创建该默认文件。当前会把用户指定的错误/缺失路径悄悄变成新脚本，偏离原版。应覆盖路径拼错、无读权限、存在目录、编码错误、PAC 服务启动失败和外部修改后刷新；本次只读未创建文件。

## AUD-DESK-12：取消勾选全部更新项会被后端解释成全部（P1，静态确认）

`update_controller.dart:212-216,249-259` 按用户勾选构造 `_selectedCores`，零勾选时传空数组；`crates/bridge_api/src/api/t16.rs:869-875` 将空数组定义为全部 BUILTIN_TARGETS。因此 null/default（全部选中）和用户显式空选择（什么都不选）在桥接处丢失语义。冻结上游 `UpdateService.cs:116-127` 对空 SelectedCoreTypes 不执行任何 core 检查，只有 null 才默认全选。

修复应区分“未指定”与“显式空选择”，UI 零勾选禁用按钮/明确提示，后端也不能把来自 UI 的空列表扩成全部。须验收全选、部分选、空选择和缺省/旧配置恢复。此项未追加测试，不宣称真实远端请求已运行。

## 本次命令与结果

只读：`Get-Content` 读取 AGENTS、R4-13 任务卡、相关分组 README、当前 Dart/Rust/native 与冻结上游；`rg --files`/`rg -n` 定位字段消费者；Python yaml/csv 生成并校验 66 行 matrix；`git status --short` 确认已有产品改动保留。

故障实测（在 apps/desktop，使用固定 Flutter 路径）：

```powershell
& 'C:/Users/Colby/toolchains/flutter/bin/flutter.bat' test '../../docs/evidence/tun-settings-audit-2026-10-05/desktop_repro_test.dart' --reporter expanded
```

实际结果：退出码 1，两项按正确合同写的断言都失败，分别复现自启假成功与旧草稿覆盖；不是编译/引擎崩溃。本次没有执行 cargo、全量 Flutter tests、flutter analyze、原生 UI、打包或任何平台应用命令。首次调用日志路径用错而未启动测试，随后纠正为上面的命令；不把该首次调用计作测试通过。

## 修复验收优先顺序

1. 保存合同：捕获草稿版本、同步整树/分组 revision，明确保存成功/运行已应用/平台已应用/需重启四种结果，保留失败重试。
2. TUN 与系统代理总结果：在独立 TUN 分册修生命周期；本分册修系统失败回传、完整配置去重和启动副作用说明。
3. 无消费者字段：HWA、应用日志、TLS 根来源、消息持久化、Mihomo merge、外部路由模板，逐个从正式 UI 入口验证，不以 helper 单测顶替生产接线。
4. UI 状态唯一源：canonical proportions/columns/geometry 与旧缓存迁移，备份恢复/多窗口冲突/重开验证。
5. 原生平台专项：自启/热键/WinINET/PAC/DPI/托盘，以及 macOS/Linux 专属行为。在可授权环境验收后才写 verified；真实环境未验和缺代码必须分别登记。
