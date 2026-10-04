# 设置领域第二轮查漏

状态：`identified`。当前提交 `a2b6905c595cb69a61e53c27a6fb34acaa210a81`；开始检查及运行 Rust 测试时为 `efd6b2fdd8bfb22fcf191c8dc5a3ea1ce6e200ca`，两者间仅 profiles 四文件与 dist 元数据变化，本报告涉及的实现未变化。冻结原版为 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 以 WPF 为依据。

本轮只复查新 FIX 后的正常流程及旧清单遗漏，不复用上一轮 1251cbc 的“未修复”结论。普通空 URL 组、当前组更新对象、订阅提前返回 job ID、正常启动 scheduler、路由同草稿保存/删除空规则、DNS 默认模板预览、热键 WPF 编码和回调等均已有新实现；下面只列当前仍能从调用链确认的缺口。未修改生产文件、个人资料或宿主代理/自启/TUN，未触发原生全局热键。

文中 `原版/` 代表 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。各定位均为当前文件的一基行号。

## SR-01 — 部分订阅失败被前端重造为全部成功（P1，新 FIX-09 回归）

触发：有 A、B 两个启用的下载订阅和 C 普通空 URL 分组；执行“更新全部”，A 返回有效节点，B 返回错误或空内容。实际后端报告可分别成功、保留旧节点、跳过；前端把 A/B/C 都显示 updated，success=3。同数量替换的节点也会被报 added=0，因为 added 被算成总数净增长。

- 原版：`原版/ServiceLib/Handler/SubscriptionHandler.cs:17` 开始逐组处理；`:28` 跳过禁用项；`:41` 仅成功处理才增加 successCount；`:47` 单组异常回报；`:60` 排除无 URL 分组。原版结果会保留逐组失败信息，不把无 URL/失败组认作成功。
- 当前：`crates/bridge_api/src/api/subs.rs:253` 的 `finish_sub_job` 只要 `report.success_count()>0` 就置 `JobState::Done`；完整 report 仅通过 `:240` 附近的 `subscriptions_updated` 事件发送。`apps/desktop/lib/features/subs/subs_controller.dart:267` 处理 Done 时没有读取 report，而把 `_targets(subIds)` 全部构造 `status:'updated'`，`:280` 用 entries.length 作为 success；`:328` 的 targets 对全部更新只过滤 enabled，没有过滤空 URL。`:275/:314` 用节点数量正差重造 added，无法表示真实更新内容。
- 生效链：Rust 下载与事务保存可以正确；损坏发生于 job 终态→Dart结果重建→toast/status，原始 per-entry 状态、错误与 added/removed 被丢弃。现有 `apps/desktop/test/fix09_subs_job_cancel_test.dart:89` 的成功用例用未改变节点的 Done job，并只要求结果 ok/entries 非空，反而放过该问题。
- 测试：本轮未运行这条真实 FRB/窗口组合。建议统一用三个合成分组、本地成功/失败端点实测，断言逐组结果与后端 report 相同；禁止真实用户订阅。需接回同 job ID 的 terminal report，不能由 Done 布尔值推断每组成功。

## SR-02 — 原版 ZIP 导入不安装配置资源（P1，FIX-14 仅修好自家 manifest 包）

触发：原版 ZIP 内含 `guiConfigs/config/custom.json`，Custom 节点 Address 为 `custom.json`；在新数据目录通过“本地恢复 ZIP”或“导入原版备份”导入，然后激活此节点。DB 行和 active ID 可以导入，但 live `config/custom.json` 没有被复制，文件型节点无法取得其原始配置。PAC、脚本及其他源配置目录资源同样缺独立安装阶段。

- 原版：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:158` 打包整个配置目录，`:169` CopyDirectory；`:137` 解压完整目录到配置路径，资源随配置一起恢复。
- 当前：`crates/application/src/backup_service.rs:203` 的 import_upstream 只调用 candidate DB 导入及 `:223` activate_upstream_config；后者仅写 guiNConfig。`crates/persistence/src/upstream_db.rs:241` 的 snapshot 只拷贝 DB/config；`crates/persistence/src/candidate.rs:125` 提交 candidate 数据库。没有把原版 ZIP 资源复制到 live data 的阶段。已新增的 `crates/application/src/backup_service.rs:159` restore_resources 仅用于本项目 manifest bundle 的 restore，并不被 upstream import 调用。
- 后续消费：`crates/application/src/engine.rs:634` 的 custom_file_text 根据 Address 读取绝对路径或 `<data>/config/<Address>`；`:646` 读取失败返回 None。源目录内文件存在/候选路径校验通过，并不能证明目标运行目录存在。
- 测试：本轮 backup 9/9 通过，其中 nested_resources_roundtrip_and_restore_complete 验的是自家 manifest bundle；upstream_import_activates_config_and_active_id 验的是配置/active ID，没有断言原版 ZIP 的 custom/PAC 文件落到 live。未实际激活资源型导入节点。修复验收应增加含资源的原版 ZIP→导入→live 文件哈希→重开→实际生成/运行，并保留回滚语义。

## SR-03 — 恢复后引擎换库，主窗口与运行会话仍旧（P1，FIX-14 的恢复生命周期不完整）

触发：正常打开应用，旧节点 A 已显示或运行；导入包含节点 B、不同主题/语言的备份，关闭备份窗口，直接查看节点表/分组/主题/状态或操作现有托盘菜单。引擎能读新库，但这些长期存活的 Dart provider 与原运行会话没有统一失效和重载。

- 原版：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133` AppExitAsync，`:134` 释放 SQLite，`:141` 之后重启，`:150` Shutdown；新启动自然重建 UI/配置/内核，不让旧运行状态继续冒充已恢复配置。
- 当前：`crates/application/src/engine.rs:259` quiesce 只把四个仓库换成空内存仓库；不停止正在运行的 runtime、订阅 scheduler 或已经启动的更新 job。`:278` reopen 重新读 settings、active、revisions 等，但不通知 Flutter 各 provider，也不停止/重新应用已有 runtime。`apps/desktop/lib/features/backup/backup_controller.dart:141` 的 _refreshAfterRestore 仅加载 WebDAV/coreVersions；`:239` importUpstream 连此刷新也没有执行，只显示“原版设置/活动节点已激活”。没有 profiles/subs/routing/DNS/settings/ui shell 的统一 reload。
- 明确边界：重新打开 Option/Theme/Hotkey 窗口会在 init 中调用 settings.load，因此本报告**不**声称“重新开设置必然用旧草稿覆盖导入配置”。确定缺口是恢复完成后主表/分组/即时主题和原运行会话没有统一刷新；Dart 缓存错误覆盖及 scheduler 与换库竞争的具体结果仍需合成运行验证。
- 错误处理：`crates/bridge_api/src/api/t16.rs:180/:182`、`:270/:272` 和 WebDAV `:604/:606/:638/:640` 忽略 quiesce/reopen 的 Result；换库后重开失败仍可能根据文件服务报告向 UI 返回成功。需让停止任务/会话→关句柄→交换→重开→刷新全部 provider→按原版时机恢复运行组成可观察的流程。
- 测试：本轮 live_engine_quiesce_restore_reopen_roundtrip 通过，证明底层 DB/config 重开，未覆盖长期 Dart provider、受管运行恢复或并行 scheduler。未运行真实恢复窗口/核心，未碰宿主代理。

## SR-04 — 导入 DB 已提交后配置激活失败，没有回滚（P1，新两阶段提交缺口）

触发：导入的数据库候选验证成功，但 live guiNConfig 写入/替换失败，例如目标配置路径不可写；UI 返回“导入失败（未修改现有数据）”，数据库实际上已经换成导入结果，配置/active 仍可能是旧值。

- 原版依据：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:130` 先备份当前配置再恢复；本项目自身同样承诺失败保持旧配置/资源（FIX-14 及恢复 UI 文案）。这里不宣称原版所有解压失败均具备原子事务，而是新导入的数据库+配置合同未闭合。
- 当前：`crates/persistence/src/candidate.rs:125` 的 commit_candidate 已交换目标 DB 并在 report 提供 target_backup；`crates/application/src/backup_service.rs:223` 接着执行 activate_upstream_config，`:280` write_json_atomic 返回失败即通过 `?` 退出，未使用 target_backup 回滚 DB。`apps/desktop/lib/features/backup/backup_controller.dart:248` 对失败仍声称未修改现有数据。
- 同类窄口：`crates/application/src/backup_service.rs:127` restore 已换 DB，`:135` backup_previous(config_path)? 在建立配置回滚副本失败时也直接退出，没有执行稍后 copy_atomic 错误分支的 rollback_database。现有 resource_copy_failure 测试覆盖的是更后面的资源复制失败。
- 测试：未注入上述配置激活/建立 prior 副本失败；已有 9 项备份测试不能证明这两个错误位置可回滚。需在临时目录对每个提交后失败点验证 DB、config、resources、active 一致，并让错误文案只报告实际事实。

## SR-05 — 热键编辑期间没有暂停旧热键动作（P1，旧清单没有单列的源生命周期）

触发：已保存显示/隐藏或代理动作热键，打开热键窗口，点录制并按下已注册组合。原生注册仍生效，原处理器可执行窗口/代理动作，而不是仅将这个组合送给录制器。代理动作本轮禁止实测；可优先用 showWindow 合成绑定做安全验收。

- 原版：`原版/v2rayN/Views/GlobalHotkeySettingWindow.xaml.cs:16` 打开时 IsPause=true，`:17` 关闭恢复 false；`原版/v2rayN/Manager/HotkeyManager.cs:145` 暂停时把 WM_HOTKEY 转换为控件 KeyDown，`:161` 只有非暂停时才执行动作。
- 当前：`apps/desktop/lib/features/settings/global_hotkey_window.dart:46` init 只 load，`:92` _onKey 只处理 Flutter KeyDown，未暂停原生分派/注销旧绑定或安装录制路由；`apps/desktop/lib/features/settings/hotkeys.dart:194` 注册 keyDownHandler 直接 onTriggered。`apps/desktop/lib/app/shell/desktop_integration.dart:90` 已安装真实 _onHotkey handler。新 WPF 编码和 modifier 录制修复不解决旧绑定仍执行动作这一生命周期。
- 测试：未运行真实 OS 热键；已有假 registrar 的编解码/dispatch 测试不证明原生编辑模式。验收需“打开编辑→按旧组合只录制→取消恢复旧注册→保存使用新注册”，并测试原生组合是否还能到 Flutter。

## SR-06 — 同组合绑定多动作没有保留原版分组注册（P2，剩余 parity）

触发：两个动作录制相同组合后保存。原版把同组合注册一次并按动作列表分派；当前按 action identifier 分别注册同一个 OS 组合，第二项可能被原生插件/Windows拒绝，或插件索引覆盖，只执行其中一项。

- 原版：`原版/v2rayN/Manager/HotkeyManager.cs:8` 用 Dictionary<int,List<EGlobalHotkey>>；`:47` 首次建列表、`:55` 同 key 追加不同动作；`:64` 按组合注册；`:161` 逐动作分派。
- 当前：`apps/desktop/lib/features/settings/hotkeys.dart:173` 遍历每个 binding，`:185` identifier 按动作，`:187` 每项 manager.register，回调也只解析一个 action。没有按 modifiers+WPF Key 分组。注册冲突被记录，但该文件 `:294` 的 HotkeyController.save 在 `:305` 最终仍返回 true，窗口会关闭。
- 测试：未跑原生重复组合；不把“报告冲突”当原版正常多动作绑定能力。需要一个组合注册结果对多个动作分发，测试无需调用宿主代理，可用记录型 handler。

## 本轮实际验证与边界

运行命令（exit 0）：

```powershell
& 'C:/Users/Colby/.cargo/bin/cargo.exe' test -p application --test t16_backup --test t11_routing_dns --locked
```

结果：t11_routing_dns **20/20**、t16_backup **9/9**，共 **29** 个合成后端用例通过。临时数据/NullRuntime、纯配置与本地回环夹具；未启动核心、未写宿主代理/自启/TUN。它们确认路由草稿保存底层、DNS存储/生成、manifest资源回拷、底层引擎重开等能力，不能推广为上面六条 UI/原生生命周期已通过。

未运行：新增真实 Flutter/FRB订阅部分成功场景、原版 ZIP 资源型节点生效、带活动核心/后台更新的恢复、配置激活失败注入、OS热键编辑/重复组合、macOS/Linux。未重算旧800行的“完成率”。此报告只写六个可定位缺口，原清单分母不变；热键暂停生命周期、上游资源安装、分项订阅结果与恢复全局失效应作为细化验收项追加登记。
