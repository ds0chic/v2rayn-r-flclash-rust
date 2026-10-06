# 完整稳定移植复审：设置、备份迁移与更新

状态：identified。基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`；冻结原版 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本轮不修改产品代码，不把旧审计的失败清单当作当前结果，也不把静态消费者、mock 成功或单测数量换算成移植完成度。

结论：Phase 1 已关闭若干保存合同缺陷，但设置窗口的失败重试仍有可复现问题，多个设置仍没有生产消费者。不能验收为完整稳定版本。180 个设置 ID 已全部登记到本轮矩阵；全部字段的真实 UI→Rust→持久化→重开→内核/平台效果并未逐项完成。

## 审计范围与证据

- `settings-current-180.csv`：180 唯一 ID，与 `compat/fields.settings.yaml` 集合完全一致；23 容器、2 内部状态、155 叶子字段。134 行静态 implemented、46 行 identified，**不代表 134 项已端到端验证**。候选字符串引用包含写表单的代码，不能单独证明消费；历史控件/测试引用单列保存，没有冒称本轮重新实测。
- `settings_retry_contract_test.dart` / `settings_retry_contract.log`：3 项正确预期合同均失败，复现下文 CP-SET-01/02/03。Root 实际执行；SyntheticBridgePort、CountingRuntimeBridge、FakePlatformBridge 完全替代真实后端。没有加载原生 Rust 或写入 OS。
- `pure-probe/`：当前真实 application/domain/config_codegen 库与合成 SQLite/config 的观察工具，运行时为 NullRuntimeClient。没有网络、socket、内核、helper 或系统代理/路由操作。首轮 `run.log` 因合成节点遗漏必填 Remarks 退出，不计为产品缺陷；夹具补 Remarks 后更正运行退出 0，每轮使用独立目录。结果见 `run-corrected.log`、`observations.json`。
- `core-inventory.json`：本轮复算 14 个下载资产 SHA256，全部与 lock 匹配；14 个 exe 存在，13 个有独立期望哈希且全匹配，Xray lock 没有独立 exe 哈希，只登记本次实际值与匹配的 ZIP 哈希。
- 关联任务：R4-13、R4-25、R4-28、R4-29；设置 FLD-CFG-001..180；备份 F-BACKUP-001..004、ACT-BACKUP-001..005；更新 ACT-UPDATE-001..004；设置 ACT-OPT-001、ACT-THEME-001、ACT-MSG-001/002；布局按 `compat/layouts.yaml` 的 MainGirdHeight/WindowSize/列存储规则。

## 当前确认的缺陷与修复验收

### CP-SET-01 / P1：自启写入失败后重开设置，会再次假成功

当前位置：`apps/desktop/lib/features/settings/settings_controller.dart:165,310`，入口 `settings_actions.dart:26`。

同一控制器不重新加载时，修复后的 `_autostartApplied` 可使第二次提交重试；但是每次打开设置入口调用 `load()`，该方法直接把已保存 AutoRun 当作 OS 确认值赋给 `_autostartApplied`。流程“勾选自启→Run-key 写失败→关窗→重开→再确定”第二次不会写 OS，仍可返回成功。本轮故障合同实际 attempts=1，正确值应为 2。

原版证据：`ServiceLib/ViewModels/OptionSettingViewModel.cs:397-405` 在成功 SaveConfig 后执行 AutoStartupHandler.UpdateTask；未把读取 JSON 作为平台确认。

修复：保存 desired 与 OS 对账结果分离；读取设置不得覆盖已知失败/未知状态。必要时查询真实平台值或每次提交做幂等对账。验收覆盖首次失败、窗口内重试、关窗重开重试、应用重启重试、外部删除自启项。OS 真实写入仍须独立验收。

### CP-SET-02 / P1：独立设置窗口首次保存后应用失败，窗口内重试永远 stale

当前位置：`settings_actions.dart:31-38,62-69`；`settings_controller.dart:196,208,295`；`settings_window_host.dart:63-86,157-178`。

独立窗口闭包永久捕获打开时 snapshotRevision。首次保存成功推进 revision，但应用失败时窗口保持打开；下一次确定仍传原 revision，保存被正确拒绝为 stale，实际 apply 无法重试。控制器对原对象的 `_draftRevisions` 更新不能补救 JSON 跨引擎路径显式传入的旧版本。

本轮测试是**控制器故障合同 + 正式入口源码追踪**，没有启动第二个 Flutter 引擎、没有模拟 native method-channel callback，不能称原生窗口实测。它准确复现入口使用相同捕获版本的参数序列：首次 saved=true/applied=false，解除运行错误后第二次 ok=false。

修复：窗口协议返回 saved、applied、新 revision 和失败阶段；仅接受本窗口已成功提交的 revision 更新。重试已保存草稿可直接重试应用，避免再次覆盖整树。其它窗口推进版本仍应冲突，禁止用任意最新 revision 盲重试。验收独立窗口真实桥接的保存失败、运行失败、平台失败、同值重试和编辑期间外部修改。

### CP-SET-03 / P2：更新/监控选项保存失败只改变当前 UI，重开丢失且无错误

当前位置：`features/update/update_controller.dart:175-209`；监控 `proxies_view.dart:79-102`、`connections_view.dart:77-93`。

本轮通过拒绝 saveSettingsGroup 的内存桥注入失败。setPrerelease 仍改变本地 state，既不回滚也不设置 error，正确合同失败。后端的存储守卫修好，不能让忽略返回值的前端自动变成正确。受影响字段见矩阵 FLD-CFG-131/132/134/147/148/149。

修复：成功落盘才提交本地效果，或保留临时状态并明确“未保存”、提供重试；持久化失败不能被 catch 当作测试环境特例吞掉。验收存储不可用、stale、重开和重试。

### CP-SET-04 / P1：canonical IndexId 与 active_index_id 的分裂会影响上游 ZIP 恢复

当前位置：`crates/application/src/engine.rs:1128-1164,1779`；`backup_service.rs:430-453`。

普通 active 切换写 engine 自有 `active_index_id`，没有同步 canonical IndexId。上游 ZIP 激活则只取 IndexId，再覆盖 active_index_id。不是“所有上游恢复都不工作”：正常原版 ZIP 的 IndexId 会被真实 remap/激活；风险是本项目使用新活动节点但保留旧 IndexId，再导出兼容 ZIP/上传 WebDAV 的往返链。

本轮 pure probe 实际执行 SQLite 保存 A/B→当前活动 B、保留 canonical A→BackupService.create_local→zip_upstream_layout→生命周期 restore。结果 `backup_active_before=synthetic-b`、`backup_canonical_index_before=synthetic-a`、`backup_restore_status=Imported`、`backup_active_remarks_after_restore=A`，确定恢复成旧活动节点。这个复现没有 mock 存储、备份、ZIP 或激活流程；只有 runtime 为不会启动真实内核的 NullRuntimeClient。

修复应统一活动状态的 canonical 读写规则及旧文件迁移，不删除旧键。验收 A→B 切换后新格式备份、兼容 ZIP、WebDAV 上传格式和原版客户端恢复，活动节点一致。

### CP-SET-05 / P1：Happy Eyeballs 开关不控制最终生成

当前位置：`crates/application/src/codegen.rs:399,578`；`crates/config_codegen/src/xray/dns.rs:374-382`。FLD-CFG-176..180。

生成器按参数是否默认决定添加 happyEyeballs，未检查 enable_happy_eyeballs；参数非默认时关闭开关仍生效。上游 `V2rayDnsService.cs:543-547` 先检查开关。pure probe 实际观察关闭仍发出对象、同参数 off/on 生成 JSON 完全一致。

修复：在所有适用 outbound/sockopt 分支共同执行开关与 skip 条件。验收关→开→关、默认/非默认参数，以及代理/直连/拨号分支的最终配置与真实 Xray 校验。

### CP-SET-06 / P1：原版合法 MaxSplit 范围仍被拒绝

当前位置：`option_setting_window.dart:358-364`；`crates/application/src/settings.rs:91-99`。FLD-CFG-153。

UI 与 Rust 都只接受整数，原版 `OptionSettingViewModel.cs:303` 调用 `Utils.TryParseMaxSplit(input,0,10000)`，接受 `1-3`。`Utils.cs:634` 与 `V2rayOutboundService.cs:831-843` 是来源。本轮纯 Rust 实际返回 `E_FIELD_FORMAT`、field=Fragment4RayItem.MaxSplit。不能仅把校验放宽而继续丢失范围含义；需保持原始字符串、按原版解析、用于正确生成。验收单值、范围、反向范围、边界、空值与非法输入。

### CP-SET-07 / P1/P2：可存储的设置仍缺生产效果

| ID | 缺口 | 原版消费证据 / 正式修复 |
|---|---|---|
| 062 EnableHWA | Rust/DTO/表单存在；runner 无渲染选择 | WPF `MainWindow.xaml.cs:151-154`；实际 renderer 选择和帧性能验收 |
| 063 EnableLog | 应用日志无开关消费者 | `AppManager.cs:101`；不能以内核 LogEnabled 替代 |
| 064 RootCertProvider | cert 模块只映射 system/chrome/mozilla；订阅/updater/WebDAV 客户端未接 provider | `CertPemManager.cs:313-357`、DownloadService；给应用 HTTPS 统一信任来源 |
| 065/066 MsgUIItem | 日志过滤与刷新只用局部内存 | `MsgViewModel.cs:20-28,116`；canonical 保存与重开 |

当前定位：`crates/subscriptions/src/download.rs:119`、`updater/src/fetch.rs:36`、`updater/src/download.rs:104`、`application/src/webdav.rs:110` 独立 reqwest Client builder 不读 RootCertProvider；`domain/src/settings.rs` 与 `bridge_api/src/api/settings.rs` 只有持久化/DTO。`logs_view.dart` 没有 MsgUIItem/MainMsgFilter 消费者。

**RootCertProvider 不是安装 OS 证书**，它控制应用 HTTPS 的自定义根信任，不改系统证书库，不影响内核 TLS。R4-13.S10 旧证据把它写成“证书安装/OS blocked”有误；平台 cert 模块注释本身已说明正确语义。无需把缺少 HTTP 消费者归因于用户未授权。

### CP-SET-08 / P1：Mihomo merge 与外部路由模板仍未走正式运行链

FLD-CFG-129/130：`codegen.rs:461` 能生成 MixinOptions，但当前生产 runtime build 没调用它；`engine.rs:2955` 走 native custom 原文分支。相关单测只证明投影/merge helper。原版 `CoreConfigClashService.GenerateClientCustomConfig` 实际改 ipv6、端口、TUN/Mixin。修复要在真实 custom Mihomo 计划生成中接入，未知 YAML 顺序/字段保留，配置校验与启动验收。

FLD-CFG-087：`bridge_api/src/api/routing.rs:411-427` 配置外部模板 URL 时明确返回 unavailable，无另一个异步 fetch/导入入口。原版 InitRouting 的外部来源功能仍缺失。区域 Russia/Iran 仅存待下载 URL，不等于完整预设落地。修复异步下载/取消、内容校验、事务化导入及失败保留旧规则。

本地 SRS：已有资源下载与 SrsSourceUrl 消费者；但生产构建没填 `CodegenSettings.local_srs_files`，不能以下载了 srss 文件证明生成器选择本地资源。补正式来源枚举与离线/损坏文件回退验收。

### CP-SET-09 / P1/P2：平台总结果修好，但去重与持久化仍有缺口

当前 `platform_controller.dart:384-387` 的 key 仍只有 mode/session/port，忽略 exceptions、local bypass、advanced protocol、PAC path。同会话保存组时可能跳过有实际变化的系统代理。完整 saveAndApply 新会话可补救，不能泛化为每次完整保存都无效。

`_persistMode` 忽略 saveGroup 结果；OS 操作成功但持久化失败时，下次启动模式不同。自定义 PAC 缺文件仍在用户指定路径生成默认内容；原版 PacManager.cs:34-44 对缺失自定义路径回退默认 config/pac.txt。

修复使用实际平台配置内容 hash/revision 去重；结果分 desired 持久化、core 应用、proxy/PAC 应用。验收正常及失败恢复均须保护当前已应用端点。本轮未写真实系统代理。

### CP-SET-10 / P2：canonical UI 状态与实际 UI 状态源不一致

MainGirdHeight1/2 用 `ui_state.json` split 替代，节点列用 column_layout，窗口用 exe 旁 INI，连接表固定 DataColumn。对应 FLD-CFG-068/069/081/082/117..119/136/156..158。

`ui_shell_controller.dart:241-243` 的 hideIpInfo/showStatistics/autoAdjustColWidth 只有赋值，没有实际表格读取。统计引擎有 EnableStatistics 消费者，不代表统计列显隐也对齐。

独立 `option_setting_window_entry.dart:24` 固定 light 主题，没有主窗 locale/font delegates；主窗主题/字体有效，独立窗缺继承。Windows 仍展示 Avalonia 扩展主题名称却落成 system。修复统一 canonical 源、一次迁移缓存、明确原版分平台可用选项；验收从原版合成配置导入到实际尺寸/列/分割，再拖动→落盘→彻底重开→备份恢复。

### CP-SET-11 / P2：订阅选择状态仍需 canonical 往返验收

SubIndexId 保留和 import remap 实现存在；正式 UI 的当前订阅还保存在单独 UI 状态源。不能以 ConfigDocument 保留键证明恢复后的活动分组、筛选、托盘范围正确。验收切到非默认订阅→保存→重开→兼容备份恢复，避免跟新默认分组混淆。

### CP-SET-12 / P1/P2：其它平台专属设置有代码缺口

MacOSShowInDock 没有 app activation policy 消费者；CustomSystemProxyScriptPath 只有路径存在校验，没有 macOS/Linux 正式执行消费链。Windows 原版不用这两个字段，Windows 项不应宣称有效；完整多平台移植仍需实现与实测。系统自启与热键消费者存在，但真实 OS 调用及多动作 dispatch 本轮未验证。

### CP-SET-13 / P1：TUN 设置消费者上下文未完整

本分册复核：production has_global_ipv6_address 没有真实探测填入，protect_core_executables 没有真实 core 路径；IPv6/legacy protect 不能仅靠布尔映射称生效。RouteExcludeAddress 的 split(',') 未 RemoveEmptyEntries，原版接受的尾逗号触发空 CIDR。更完整的提权/生命周期/实际 lease 分册由 runtime 审计负责，本分册不重复计数。

### CP-SET-14 / P1：旧 RoutingIndexId 的迁移缺失

`default_routing()` 当前只查 RoutingItem.IsActive，fallback first；ConfigHandler.cs:2616-2623 会将旧 RoutingIndexId 对应路由设 active 并清空旧字段。当前普通“设为默认路由”已实现；缺陷限定旧配置/旧备份含 RoutingIndexId 的迁移，不宣称所有 routing 都不能用。验收旧 ID remap 后选择指定路由，非法旧 ID 按原版回退。

### CP-SET-15 / P1：Rust 加载仍可能伪造默认成功

`engine.rs:3374-3375` 对已存在空 guiNConfig.json 返回空对象，`read_settings_state:3399` 对某个已知字段反序列化失败整树 unwrap_or_default。这绕过本次 Dart loadFailed 守卫：bridge 获得“成功默认文档”，UI 无法识别错误。空文件与 malformed known field 的 pure probe 结果见 observations。

本轮实际结果：已有空文件仍 load success；合成文档只是 TrayMenuServersLimit 字段类型错误，其余合法配置 CurrentLanguage=en、KeepOlderDedupl=true、LocalPort=11808 也全部丢失，返回 zh-Hans、false、10808 默认值。只读取生成的配置，没有监听 10808。

原版 `ConfigHandler.cs:29-34` 对已有但读到空内容返回 null；坏字段整树默认即使旧版也可能容错，稳定移植目标至少不得静默毁掉其它合法字段或未知扩展。

修复把解析错误带路径传播为存储不可用/可读失败，禁止写默认覆盖；仅对缺省字段应用默认。验收空文件、截断 JSON、字段坏类型、null/empty/missing、保留未知键和只读失败。

## 已修复的关闭项（仅在相应合同范围）

1. 整树保存后重读 groupRevisions；本轮基线代码存在；根重跑既有 6 项审计回归通过。重读失败时仍吞异常，后续 group save 需恢复对账。
2. load throw/失败 DTO 不再生成可编辑默认文档；未关闭 CP-SET-15 的 Rust 侧默认伪造。
3. save_settings_group 补 guard_storage；本轮纯 probe 实际 load blocked=true、group save rejected=true、created_config=false。拒绝不落盘已验证；没有运行真实宿主存储故障。
4. old full draft 使用捕获版本，外部新修改不会被旧稿覆盖；未关闭 CP-SET-02 本窗口自己的成功保存后重试。
5. saveAndApply 采集平台错误，已保存/平台未应用能回传；未关闭平台内容去重/持久化问题。
6. t16_check_updates 已区分 None=默认全部、Some([])=明确不选。UI 空列表不再扩成全部；选项失败持久化仍另有问题。

## 订阅、备份、WebDAV、更新的当前真实边界

| 流程 | 当前实现与证据 | 完整稳定验收还缺什么 |
|---|---|---|
| 订阅下载/解析/替换 | application refresh→候选 parse/filter→epoch guard→事务 replace；现有 subs_pipeline、r4_16_matrix 与 sqlite tests 有真实 loopback 和真实库合同 | 最新正式 UI→FRB→运行节点移除/保留→重开，真实 TLS 信任来源，各失败/取消不损旧数据；不能只看 parser |
| 原版 ZIP 恢复 | BackupService lifecycle prepare_restore/quiesce→candidate→replace→activate→reopen，源数据 remap/资源带入；实际 SQLite/ZIP 单测存在 | 用户入口从备份到真实桌面重开与 runtime/hotkey/platform 状态全对账；本轮新增 CP-SET-04 活动节点往返 |
| 本地新格式 backup | SQLite 一致性 snapshot、manifest hash/resource verify、候选恢复；不是只 mock API | canonical UI cache/INI 是否入包、失败/中断回滚与同盘空间不足、进程重开 |
| WebDAV | 当前真实 reqwest，PROPFIND/MKCOL/PUT/GET，上游兼容 guiConfigs ZIP；t16_webdav 的真实 HTTP loopback覆盖401/404/500/timeout/roundtrip | TLS trust-source、真实授权远端、正式 UI/provider/runtime 重载。RootCertProvider 缺代码，不能归“只是没授权” |
| 备份取消 | BackupController.cancel 只 generation++/busy=false，没有后端 job/token；返回晚结果被UI丢弃，已交Rust的写入仍会完成 | production UI 没有取消按钮，关闭只 Navigator.pop；若暴露取消必须后端取消/commit边界；不可把旧mock“结果未显示”称磁盘没修改 |
| 核心更新 | 自动目标仍按冻结上游仅 v2rayN/xray/mihomo/sing_box，其它核应人工安装；14 核锁已存在，不能沿用“只锁2核” | 全14核正式UI→FRB→net-host运行和配置效果，而非直接核心脚本监听；版本升级/校验/回滚真实入口 |
| 应用自更新 | 已实现 PgpDetachedVerifier/GPG fallback、缺/错签拒绝、stage/runner/rollback helper；release app_repo 默认 None | 自有正式发行源+可信公钥；目前硬编码上游公钥，不能验证自有签名。实际 self-replace/安装器当前未实测 |

自更新另有接线问题：Rust `t16_apply_app_update_spec_with_flags(prerelease,via_proxy)` 已存在，但 `bridge_port.dart:903`/`update_controller.dart:437` 正式 UI 仍用 no-arg 旧入口；`t16.rs:1246` 使用进程内上一次检查 flags。检查后改开关或直接“更新应用”可能用旧选择。修复生成 FRB 并把当次选择显式传入；不能因 Rust 新接口存在就关闭问题。

14 核旧实际运行证据位于 `docs/evidence/recheck-fixes/R3-CORE-MATRIX/`：14 个 version 与最小监听会话，并且 hysteria2/overtls 跑了本地自签 TLS 成功/失败与代理 GET。此次只复算库存/哈希，没有重跑进程；直接脚本的最小会话不是当前版本完整前端链。6 核有上游 checksum 对比，8 核只有本地 SHA256。所有自动/人工安装范围沿用原版，没有扩大自动下载需求。

## 修复实施顺序与退出标准

1. **保存和数据安全**：CP-SET-01/02/03/04/15，修真实独立窗口协议和 canonical 数据往返。正确预期故障合同转绿，再从正式入口复验；不能修改测试去适应缺陷。
2. **消费者接线**：CP-SET-05..09/13/14，逐字段做最终配置差异与真实 kernel 校验；跨域 HTTP 信任源统一。以字段 ID 附具体代码、输入/输出和证据。
3. **界面及平台**：CP-SET-10/11/12，canonical UI 状态唯一源、多窗口主题/字体/语言、平台可用项与实际 OS 对账。
4. **整链验收**：订阅失败/取消→活动节点变化→重开；备份→修改→恢复→重开；选定更新渠道/代理→下载验签→安装→启动/回滚；完整 TUN 正常/权限取消/失败清理由 runtime 分册验收。
5. 最终字段矩阵必须把每行具体证据补齐；生产连接失败/保存失败不能显示已生效。所有未运行场景注明未验证；只支持 Windows x64 的构建不得宣称其它五交付平台完整。

本轮未改 `work/`、`outputs/`、compat 分母、生产源码或发行包；未 commit。以上报告和合成夹具是本轮独立交付物。
