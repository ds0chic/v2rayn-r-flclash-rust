# 设置领域第三轮复核

审查基线：`7edf1ee4e64590093e39444188229a69beb59908`（生产修复 a4ceab5、waves A–G）；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 按 WPF。文中 `原版/` 指 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`；当前源码定位使用仓库相对路径及一基行号。

只读核对生产源码、任务卡和四台账。相关项包括 F-SUB-003/011、F-BACKUP-001..004、ACT-HOTKEY-006..008、FLD-CFG-001/002/022/089..092、FLD-ENT-001/084/091/092、LAY-SUBSET-001、LAY-HOTKEY-001、LAY-BACKUP-001。未凭台账状态认定通过。没有修改生产/compat/work/outputs/dist，没有提交；没有使用个人节点或订阅，没有启动核心或写宿主代理、自启、TUN。其他代理正在修改的 profiles 文件未触碰。

## 旧 SR-01..06 的当前状态

| 旧问题 | 当前状态 | 本轮确认及验证边界 |
| --- | --- | --- |
| SR-01 部分订阅失败被重造为全部成功 | `implemented` | Rust 将完整逐组结果放入同 job 的 stageKey（`crates/bridge_api/src/api/subs.rs:281`），Dart 读取真实 updated/preserved/skipped/cancelled/added（`apps/desktop/lib/features/subs/subs_controller.dart:239`、`:294`），无报告的 Done 明确返回 unconfirmed（`:260`），空 URL 不再成为下载 targets（`:363`）。本轮 sr01 fake bridge 1/1 通过，**没有真实 FRB+网络+窗口整链实测**。菜单摘要仍有窄口，见 R3-SET-06。 |
| SR-02 原版 ZIP 资源缺失 | `implemented` | import_upstream 调用 activate_and_install（`crates/application/src/backup_service.rs:236`），staging→配置激活→逐文件安装/hash 验证（`:287`、`:330`）。本轮 t16 的 upstream_import_installs_resources_and_generates_file_node 通过：custom、PAC、嵌套脚本落到 live，重开后自定义配置进入生成输入。**未实际运行资源型内核/PAC**。 |
| SR-03 恢复后长期 provider 与运行状态不重载 | `identified` | 原宽泛缺口已经部分修复：prepare_restore 停受管 session→quiesce（`crates/application/src/engine.rs:390`），reopen 读回 settings/active/revisions/templates（`:318`），结果和 reopen 错误合并传播（`backup_service.rs:575`）；Dart reload settings/profiles/routing/DNS/subs、重启先前 scheduler、调用 runtime.resyncAfterRestore（`backup_controller.dart:152`、`:185`；`runtime_controller.dart:155`）。t16 的三项生命周期用例及 sr03 fake 4/4 通过。仍有恢复/迁移语义、在途订阅、原生热键遗漏，见 R3-SET-02..04。 |
| SR-04 提交 DB 后配置激活失败不回滚 | `implemented` | config prior 失败、activation 失败、resource 安装失败分支现在调用配置/DB 回滚（`backup_service.rs:139`、`:304`、`:311`、`:316`）；本轮 t16 三项对应故障注入通过，sr02_04 fake 错误文案 1/1 通过。回滚动作自身失败仍未验证，也没有返回结构化回滚结果，不能扩大为“所有失败均原子恢复”。 |
| SR-05 编辑热键不暂停原动作 | `identified` | 正常打开/取消已修好：窗口 init 调 beginEdit、未保存 dispose 调 cancelEdit（`global_hotkey_window.dart:50`、`:65`）；控制器注销原生绑定（`hotkeys.dart:330`）。本轮 fake 录制/取消测试通过。**部分注册冲突后继续编辑会重新启用组合**，见 R3-SET-05；无真实 OS 热键测试。 |
| SR-06 同组合多动作重复注册 | `implemented` | groupHotkeyBindings 按 modifiers+WPF Key 去重分组（`hotkeys.dart:172`），Plugin registrar 每组合一次 register、回调遍历所有动作（`:247`、`:264`）。本轮 fake 分组/dispatch 用例通过；原生 Windows 同组合注册与录制输送未验证。 |

这些状态针对实现，不将 fake 或部分后端用例升级为完整用户流程 `verified`。

## R3-SET-01 — 重复导入旧来源，激活了另一来源配置并产生悬空活动节点（P1）

状态：`identified`；**合成后端实际复现**。

触发：导入来源 A（node-a、Dark），再导入 B（node-b、Light），最后再次导入未变的 A。用户合理预期是幂等不改变现状，或者明确恢复 A；当前实际返回 already_imported，却把 B 的配置用 A 的指纹重映射。

- 原版依据：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:128` 备份当前配置，`:137` 将**所选** ZIP 内容恢复到配置路径，`:141` 重启读取它。本项目的迁移幂等合同也明确 AlreadyImported 为 no-op（`crates/persistence/src/candidate.rs:81`；`crates/persistence/src/report.rs:12`）。
- 当前：candidate 对已有 fingerprint 在 `candidate.rs:84` 直接返回，未更新配置 meta；每次新导入只保存一份全局 `upstream_config`（`:300`）和 last_import_fingerprint（`:305`）。`BackupService::import_upstream` 对 Imported **和 AlreadyImported** 都执行 activate_and_install（`backup_service.rs:229`），后者 `activate_upstream_config:367` 读取最后一次 B 的全局 meta，再按当前 A fingerprint 派生 B 的 IndexId（`:379`、`:389`）。没有根据该批次读取 A 配置。
- 实测输出：`A=Imported; B=Imported; rows_after_B=2`；`A_again=AlreadyImported; theme_after_A_again=Light; active_exists=0`。active_exists 用 live SQLite `SELECT COUNT(*) FROM ProfileItem WHERE IndexId=?1` 查激活后的 active_index_id，返回 0。
- 用户后果：再次导入 A 显示成功，主题仍为 B；活动 ID 在数据库中不存在，重开/自动 applyActive 无法取得该节点。资源安装同时来自 A，可能形成 B 设置+A 资源的混合状态。**本轮验证到 live 配置/数据库，不声称已运行真实窗口或核心失败。**
- 建议验收：A→B→A 的 source/settings/resources/active 一致；AlreadyImported 必须保持约定的幂等结果，不能套用全局最后一次配置。按批次保存配置或从此次只读 snapshot 激活都需要明确选择语义。

## R3-SET-02 — “本地恢复原版 ZIP”实际合并迁移，保留原有节点（P1）

状态：`identified`；**底层合并实际验证，UI 入口静态证实**。

触发：现有数据含节点 A，选择只含节点 B 的原版 backup ZIP，点“本地恢复”。期望恢复备份中的节点集；当前数据集为 A+B，旧分组/路由等也可继续留存。

- 原版：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133` 退出/关 SQLite，`:137` 解压同名 guiNDB 替换数据库，随后重启，节点集来自所选备份。这里不要求删除 ZIP 没包含的外围资源，问题是数据库语义。
- 当前入口：`apps/desktop/lib/features/backup/backup_controller.dart:221` 的 _restoreArchivePath 识别原版 ZIP 后在 `:233` 调 t16BackupImportUpstream，`crates/bridge_api/src/api/t16.rs:305` 调 import_upstream_with_lifecycle；candidate 的 `crates/persistence/src/candidate.rs:91` 先复制现有 DB 到 candidate，再追加源行。
- 实测：R3-SET-01 的 A→B 导入得到 ProfileItem rows_after_B=2，确认为 candidate 合并行为。**未启动真实本地恢复选择器**；UI 到该合并函数的调用链已定位。
- 建议：将“迁移导入”和“恢复备份”分别定义；恢复原版 ZIP 应按原版替换 DB/config，合并导入可保留为明确命名的独立流程。对 A→仅 B ZIP→重开断言 A 不存在、B 存在，并覆盖失败回滚。

## R3-SET-03 — 恢复没有排空在途订阅，旧下载可在新库写入（P1）

状态：`identified`；**静态证实停机边界不完整，尚未运行并行下载/恢复复现**。

触发：订阅下载响应较慢，期间执行本地/远程恢复；或 scheduler 已进入一个超过两秒的下载。恢复完成后旧请求才返回。

- 原版：`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133` AppExitAsync、`:134` DisposeDbConnectionAsync、`:150` Shutdown；不会保留旧进程的订阅 worker 在新启动的存储上继续提交。
- 当前 scheduler 停止：`crates/application/src/engine.rs:364` 只等待到 timeout；`:375` 超过 deadline 即返回，返回类型为 void，prepare_restore 在 `:391` 固定等两秒后直接 quiesce。`crates/application/src/subs.rs:866` 在 tick 完成前不检查 stop；`:930` 每组下载使用新建、不会被 stop 取消的 CancellationToken。
- 手动 worker：`crates/bridge_api/src/api/subs.rs:249` 克隆 engine 在线程里下载，SUB_JOBS 仅诊断（`:186`），prepare_restore 不 cancel/drain 这些 Job。AppEngine 的 cloned state 是共享 Arc（`engine.rs:106`）。旧下载完成后 `subs.rs:747` 调 engine.replace_sub_profiles；该提交 `engine.rs:1958` 没有 restore epoch/预期 revision/订阅仍存在的保护，`:1981` 操作此时的仓库并写配置 revision。随后 touch_sub_update_time 失败还被忽略（`subs.rs:749`）。
- 用户后果：恢复完成后节点集又被旧下载替换，或旧 subid 的节点被重新插入；scheduler 的两秒等待并不保证“已排空”。结果会随网速变化。尚未验证发生在 quiesce/reopen 哪个时间窗的实际损坏结果。
- 建议验收：本地合成慢响应（空闲端口 ≥11808）、暂停响应→恢复→释放响应；旧任务必须取消/完成后才换库，或者其提交因恢复 epoch 失效明确被拒绝。等待超时应阻断恢复，不得静默继续。

## R3-SET-04 — 恢复 GlobalHotkeys 后，原生仍使用恢复前的绑定（P1）

状态：`identified`；**源码调用链证实，原生 OS 未验证**。

触发：已注册显示窗口组合 K，恢复含显示窗口组合 J 的备份；恢复后按 J/K，或者打开热键窗口查看 J 后取消。

- 原版恢复后重启（`原版/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:141`、`:150`），`原版/v2rayN/Manager/HotkeyManager.cs:25` 从新 AppManager.Config.GlobalHotkeys 构建注册。
- 当前 _reloadProviders（`backup_controller.dart:152`）没有 hotkeyController 重载/注册；`SettingsController.load:102` 的 _applyImmediate 只转 UI shell 和双击偏好（`settings_controller.dart:179`）。生产热键初始 loadFromSettings/registerAll 在 `app/shell/desktop_integration.dart:101`、`:102`；其余重新注册只有热键窗口保存。
- 打开窗口只 settings.load（`global_hotkey_window.dart:59`）；取消执行 `hotkeys.dart:342` 的 cancelEdit→registerAll(state.bindings)，bindings 仍是恢复前 K，而表单 draft 展示恢复后的 J。因此查看/取消不能补齐注册刷新。
- 用户后果：保存文件/表单显示与实际热键不一致；代理动作组合可能继续使用旧配置。**本轮只用源码核对，没有按真实代理类热键。**
- 建议验收：受控 fake registrar 覆盖 K→恢复 J→立即使用 J→打开/取消→仍为 J，之后用只绑定 showForm 的安全原生测试验证。恢复期间同样需要处理已有热键编辑窗口的生命周期。

## R3-SET-05 — 热键部分注册冲突后继续编辑，成功组合已经重新启用（P1）

状态：`identified`；**静态证实，现有 fake 测试未断言此分支的暂停状态**。

触发：两个不同组合，显示窗口 K 注册成功，另一个组合被其他程序占用；点保存，窗口提示冲突并保持打开；继续点录制，按 K。应只录制，当前原生回调可以触发已成功动作。

- 原版窗口从构造到 Closing 一直 IsPause（`原版/v2rayN/Views/GlobalHotkeySettingWindow.xaml.cs:16`、`:17`）；`原版/v2rayN/Manager/HotkeyManager.cs:145` 暂停时转录制 KeyDown，`:161` 仅非暂停时分派动作。
- 当前 `HotkeyController.save` 先保存 bindings，然后 `_paused=false`、registerAll（`hotkeys.dart:399`..`:402`），最后才根据 conflicts 返回 false。Plugin registrar 对成功组合安装实时 handler（`:264`..`:277`）。窗口在 false 时保持打开（`global_hotkey_window.dart:170`..`:183`），后续录制不会重新 beginEdit。
- 取消的同类边界：_paused 已为 false，cancelEdit 在 `hotkeys.dart:343` 直接返回；窗口注释“恢复 pre-edit registration”在这条失败保存分支不成立。这里不推定原版注册冲突必须撤销已持久化配置，确定问题是**仍在编辑期间实际动作恢复**。
- 已跑测试：sr05_sr06_hotkey_test 6/6 通过，其中冲突用例（`:262`）只断言窗口仍开/显示冲突，没有“一个成功、一个冲突→继续录制→动作不触发”或取消后注册断言。未运行原生热键。
- 建议：编辑结束前一直阻止动作 dispatch；如果要尝试注册检查冲突，成功组合也不能恢复实时动作。补 partial-failure→record/cancel/retry 测试。

## R3-SET-06 — 主菜单摘要仍隐去部分订阅失败（P2）

状态：`identified`；**静态展示链核对，未新增真实菜单网络测试**。

SR-01 的逐组真结果已进入 SubsController.status（`subs_controller.dart:413`）。但用户直接在主菜单“更新全部”得到的 toast 是 `subs_actions.dart:356` 的 _updateToast：只要 result.ok 就显示“订阅更新完成：成功 N”，未显示 preserved/error/skipped；当前组 toast 也只按 ok 分支（`:384`）。原版 `原版/ServiceLib/Handler/SubscriptionHandler.cs:47` 单组错误回调会明确显示。修复没有再伪造失败组为成功，但**正常主菜单流程的可见反馈**仍使用户难以得知部分失败，应复用逐组汇总并允许查看错误详情。协议 DTO 没有 removed 字段，`subs_controller.dart:309` 也未读取后端 removed；这不影响新增 added 的真实性，但不能说所有结果字段已完整呈现。

## 实际命令、结果及可复现合成夹具

Rust（仓库根，exit 0）：

```powershell
& 'C:/Users/Colby/.cargo/bin/cargo.exe' test -p application --test t16_backup --test t11_routing_dns --locked
```

结果：t16_backup **16/16**、t11_routing_dns **20/20**。临时目录/Null 或 StopFailingRuntime；路由 URL 夹具逐端口尝试 bind 11808..11949，未使用 10808。覆盖正常迁移资源哈希、配置生成输入、三处回滚失败注入、engine 重开、纯路由/DNS存储与生成；不覆盖真实活动核心、恢复并发和原生热键。

Flutter（apps/desktop，逐文件顺序运行，全部 exit 0）：

```powershell
& 'C:/Users/Colby/toolchains/flutter/bin/flutter.bat' test test/sr01_subs_report_test.dart
& 'C:/Users/Colby/toolchains/flutter/bin/flutter.bat' test test/sr02_04_backup_import_test.dart
& 'C:/Users/Colby/toolchains/flutter/bin/flutter.bat' test test/sr03_restore_lifecycle_test.dart
& 'C:/Users/Colby/toolchains/flutter/bin/flutter.bat' test test/sr05_sr06_hotkey_test.dart
```

分别 **1/1、1/1、4/4、6/6**，共 **12** 个 fake bridge/registrar 用例。没有执行真实宿主热键注册。

A→B→A 附加 harness 已归档为 [round3-settings-repro.rs](C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn/docs/evidence/parity-recheck-2026-10-04/round3-settings-repro.rs)，仅合成数据。它最初在 `C:/Users/Colby/AppData/Local/Temp/v2rayn-round3-import-a2e0169ca887428c893b43fb9b2bc2dd/repro.rs` 编译/执行；归档副本与执行源相同（SHA256 `35810745D2E697810E785A8FE14ABC5FDB10F3351E514BCD68817D3A7B2C2381`），便于 Temp 清理后仍可复现。下述临时路径/rlib 哈希是本轮执行记录；重建时使用归档源码及新编译产物路径。

可重复步骤：创建全新临时目录下的 A、B、live；用 `Store::create` 与 `persistence::UPSTREAM_TABLES` 建两个合成上游 DB，分别插入唯一 Custom ProfileItem（IndexId=node-a/node-b，ConfigType=2，ConfigVersion=4，Address=custom.json，无凭据）；A guiNConfig 的 IndexId=node-a、UIItem.CurrentTheme=Dark，B 为 node-b/Light；各写 `config/custom.json` 为 `{"outbounds":[{"protocol":"freedom"}]}`。对同一个 BackupService(live) 依次 import_upstream(A)、import_upstream(B)、import_upstream(A)，每次使用独立 work 目录。查询 ProfileItem 总数、live guiNConfig 的主题与 active_index_id，再按 active_index_id 查询对应 ProfileItem 数。原临时目录已含运行输出数据，再跑请传**新的**临时目录参数。

编译命令使用本轮 cargo 构建的 application/persistence rlib（旧 rlib 编码不可跨编译版本复用）：

```powershell
& 'C:/Users/Colby/.cargo/bin/rustc.exe' --edition 2021 'C:/Users/Colby/AppData/Local/Temp/v2rayn-round3-import-a2e0169ca887428c893b43fb9b2bc2dd/repro.rs' -L 'dependency=C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn/target/debug/deps' -L 'native=C:/Users/Colby/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows_x86_64_msvc-0.52.6/lib' --extern 'application=C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn/target/debug/deps/libapplication-4d1a48e3b9f00f59.rlib' --extern 'persistence=C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn/target/debug/deps/libpersistence-9c9ed47043b3a7ee.rlib' -o 'C:/Users/Colby/AppData/Local/Temp/v2rayn-round3-import-a2e0169ca887428c893b43fb9b2bc2dd/repro.exe'
```

生成 exe 后重现时使用新临时根目录，避免复用已运行数据：

```powershell
$round3Data = Join-Path ([System.IO.Path]::GetTempPath()) ('v2rayn-round3-fresh-' + [Guid]::NewGuid().ToString('N'))
& 'C:/Users/Colby/AppData/Local/Temp/v2rayn-round3-import-a2e0169ca887428c893b43fb9b2bc2dd/repro.exe' $round3Data
```

首次直接 rustc 缺 native search path，LNK1181（windows.0.52.0.lib），未运行；补上述 `-L native` 后编译及执行均 exit 0。实际执行给同一初次空临时根目录，输出与三个断言：

```text
A=Imported; B=Imported; rows_after_B=2
A_again=AlreadyImported; theme_after_A_again=Light; active_exists=0
```

## 剩余验证边界

未运行：真实 Flutter/FRB 的部分成功订阅下载、原版 ZIP 选择器及恢复后真正核心启动、slow download 与恢复并发、K→恢复 J 的原生注册刷新、部分热键注册冲突后的继续录制、macOS/Linux。SR-04 的 restore_previous/rollback_database 仍忽略 fs 失败（`backup_service.rs:811`、`:830`），with_rollback_note 无条件称已回滚（`:611`）；本轮验证的是三个既定前向失败点在临时目录成功回滚，**没有验证回滚本身失败**。reopen 失败后恢复 UI 的“已保留现有配置”文案也不等价于实际磁盘未交换（`backup_controller.dart:189`，`backup_service.rs:582`）。这些需作为故障验收边界保留，不能用通过数覆盖。

未重算旧 800 行完成率，未改变台账分母。本轮仓库交付仅本报告及合成重现 harness。
