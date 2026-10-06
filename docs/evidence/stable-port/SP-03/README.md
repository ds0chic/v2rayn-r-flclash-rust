# SP-03 canonical 身份备份往返 — 证据

状态：implemented（Rust 门禁全绿；真实 OS/正式包/真实远端验收未做，不写 verified）。
基线：`df8582b`（SP-00/01/02 已完成）。不要 commit，根整合者验收提交。
合成数据与专用 scratch 目录（`tempfile::tempdir`，用后即删）；无真实用户数据、
无 socket/端口、无宿主代理/路由/TUN/DNS/Run-key 操作；未占用 10808。

## 0. 缺陷复现（红，修前 1 pass / 7 fail）

`crates/application/tests/sp03_backup_identity.rs` 初建 8 项，修前仅
`sub_index_id_roundtrips_backup_restore` 通过（逐字拷贝偶然成立），其余 7 项
失败定位与审计 CP-07/CP-SET-04 一致：`set_active(B)` 后文件 `IndexId` 仍为旧值
（`None`/A），备份携带旧身份，恢复后活动回到 A，epoch 恒 0，悬空默认不修复，
纯 `IndexId` 老文件打不开默认。证据链见本卡测试注释与 `settings/pure-probe`
`backup_active_before=synthetic-b / backup_canonical_index_before=synthetic-a /
backup_active_remarks_after_restore=A`。

## 1. 改动文件

- `crates/application/src/selection.rs`（新增）：纯函数，无持久化。
  `resolve_visible_selection`（pending →  persisted 默认 → 可见首行，即
  `ProfilesViewModel.RefreshServersBiz:361-375`，临时选 C 永不写默认）；
  `pick_default`（mirror → canonical → 首行，SP-03 迁移优先级）；
  `resolve_current_group`（组存在则留，否则 None=All，不持久化修复，
  对应 `RefreshSubscriptions`）。含 10 项单测。
- `crates/application/src/engine.rs`：
  `set_active` 经 `write_identity_unified` 同时写 canonical `IndexId` 与
  `active_index_id`（切换 bump desired，幂等/修复写不 bump，失败三值回滚）；
  `repair_default_selection`（open/reopen 运行；仅修复“已持久化但悬空”的默认，
  首选可解析 mirror，其次 canonical，再次 Port>0 首行；无默认/无行不动文件，
  保住 T18 零漂移）；`resolve_startup_selection` /
  `resolve_current_group`（展示规则，不写盘）；新增 `set_current_group`
  （`SubIndexId` 显式切换，组须存在）；`save_settings` 整树与
  `save_settings_group` 非身份组在校验前 pin 回权威身份，显式 `IndexId` 组
  走校验+镜像同步（`save_identity_group`）；`persist_config` /
  `build_validated_save` 每次写盘带 `dataset_epoch`（普通保存只保留不推进）；
  `SETTINGS_META_KEYS` 新增 `dataset_epoch`（不泄漏进 `AppSettings.extra`）；
  `open_with_runtime`/`reopen` 从文件加载 epoch + canonical 默认。
- `crates/application/src/recoverable_commit.rs`：`SettingsCommitSnapshot`
  新增 `sub_index_id`（提交 pin 组身份用）。
- `crates/application/src/backup_service.rs`：`restore` 成功交换后发布
  `max(live, bundled)+1`（失败回滚 DB+config；bundle 无 config 时不删 live
  文件）；`activate_upstream_config(fingerprint, new_epoch)` 以 mirror优先
  解析激活默认（mirror 可解析则用其 remap，否则 canonical remap，悬空则原文
  照写、由重开修复收敛）并同写 epoch；`write_dataset_epoch` 对现存不可解析
  配置 fail-closed（`error.config_corrupt`，不铺新文档）；新增
  `read_dataset_epoch` / `profile_exists`。
- `crates/application/src/lib.rs`：注册 `selection` 并导出四个纯函数。
- `crates/application/tests/sp03_backup_identity.rs`（新增，10 项，先红后绿）：
  双身份统一写盘；备份携带 B+G；备份→改 A→恢复→独立重开选 B 且双身份/G一致；
  悬空默认重开落到可见首行并统一写回；纯 `IndexId` 老文件解析；恢复推进 epoch、
  旧 epoch 请求拒绝（无 newRevision）、新 epoch 可用、重开后旧 epoch 仍拒绝；
  连续恢复 epoch 单调递增；上游 ZIP（含 WebDAV 同构 `zip_upstream_layout`）
  激活双身份统一且行存在；`SubIndexId` 往返；无 config bundle + 坏 live 配置
  fail-closed（DB 回滚、坏文件原样保留）。
- Dart 零改动：`profiles_controller._selectDefaultRow` 已是 pending→active→首行
  展示规则（选/默认分离），`backup_controller` 恢复后已重载全部 provider 并
  `resyncAfterRestore`；本卡 Rust 缺陷修完，无需动 Dart。

## 2. 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt --all -- --check` | 0（初检有本卡 3 处格式 diff，`cargo fmt -p application` 限定本包修复后 0） | 通过 |
| `cargo clippy -p persistence -p application --all-targets --locked -- -D warnings` | 初次 1（本卡 `set_current_group` 的 `question_mark`，已修）→ 0 | 通过 |
| `cargo test -p persistence --locked` | 0 | 100 passed（74+5+8+5+8） |
| `cargo test -p application --locked` | 0 | lib 288（含 selection 10 新单测）+ 33 个集成文件全 ok，含 sp03 10/10、t18_stability 5/5（修复中途曾因过度修复触发 `app_reopen_ten_times_has_no_drift`，已收紧为仅修悬空默认） |
| `flutter analyze`（apps/desktop，本卡零 Dart 改动，仅确认） | 1 | 37 issues；`lib/features/backup`（本卡允许的 Dart 范围）零 issue，其余为基线既有及并行 SP-11 在途文件（`settings_window_host.dart` / `routing_windows.dart` / untracked `sp_11_window_reply_test.dart` 归 SP-11 所有，见 manifest，本卡未碰） |
| `flutter test test/r4_28_contract_test.dart`（相邻备份合同无回归） | 0 | 9 passed |
| `flutter test` 全量 / `flutter build windows --release` | 未运行 | 本卡零 Dart 改动；全量批跑历史有基础设施异常，留 SP-29；正式包留 SP-30/SP-34 |

## 3. 正确合同（摘）

- 默认写：`set_active(B)` → 文件 `IndexId=B` 且 `active_index_id=B`；切换 bump
  desired（R4-02/D27），同值重选不 bump 但顺手弥合漂移。
- 选择四身份：selected（内存表格选择，经 `resolve_visible_selection` 展示，
  不落盘）/ currentGroup（`SubIndexId`，`set_current_group` 显式切换）/
  desiredDefault（canonical `IndexId` + mirror）/ actual（`applied_session`，
  本卡不动、不伪造，恢复配置不声称切换核心）。
- 重开：pending 命中 → 默认命中 → 可见首行；持久化修复仅处理悬空默认
  （mirror 可解析优先，其次 canonical，再次 Port>0 首行），无默认/无行不动文件。
- 代次：恢复/替换激活发布 `max(live,bundled)+1` 并落盘；`save_settings_commit`
  旧 epoch → `Rejected` 且无 `new_revision`；订阅 `replace_sub_profiles_at_epoch`
  复用同一内存代次守卫（SP-02 已有）。普通保存/合并导入不推进 epoch。
- 失败：epoch 发布失败 → DB+config 回滚；坏 live 配置 + 无 config bundle →
  整体失败且坏文件原样保留；`AlreadyImported` 幂等不激活（R3-SET-01 保留）。

## 4. 正式入口 / 重开 / 最终消费者

- 正式入口：`AppEngine::open_with_runtime` 真实 SQLite + 真实 `guiNConfig.json`
 （`NullRuntimeClient`，无内核/网络/OS 副作用）；生产恢复经桥接
  `restore_with_lifecycle` / `import_*_with_lifecycle`（`prepare_restore` →
  交换 → `reopen`），本卡测试走同一函数；“重开”用 drop 后全新
  `open_with_runtime` 同目录验证（独立进程语义）。
- 最终消费者：本次为 Rust 持久化快照（重开读回 + 双身份断言 + epoch 断言）；
  FRB/Dart 接线：无需新增（无桥接签名变更；`activate_upstream_config` 为
  application 内部 `pub`，桥接未调用）。
- WebDAV：远端上传内容即 `zip_upstream_layout` 同构包（本卡已断言其激活统一）；
  真实受控远端往返未跑（见 §6）。

## 5. owned 回收

- 全部测试目录为 `tempfile::tempdir`（进程外 scratch，测试结束自动删除）；
  无 10808/宿主资源触碰；`work/`、`outputs/` 只读未动；`compat/` 未动；
  `git status` 本卡为 4 改 + 2 新 + 证据目录 + 任务卡/manifest 状态行；
  另有并行 SP-11 worker 在途的 Dart 改动（非本卡产物，未碰）。

## 6. 未验证与下一前置

- 未验证：真实 OS 副作用（本卡无）、正式未武装包端到端（SP-30/SP-34）、真实
  受控 WebDAV 远端往返（仅包格式同构已证；`t16_webdav` loopback 覆盖传输层，
  身份断言未合入）、其它 OS/架构、24h/500 切换（SP-35）。
- 观察未改（留给归属卡）：bundle 无 config + 资源失败的既有回滚分支在
  `config_prior=None` 时会删 live config（本卡新分支已加 `config_replaced`
  守卫，既有分支未动）；`delete_profiles` 删掉默认节点后内存默认悬空到下次
  重开修复（重开收敛已证，即时修复未做）；`save_settings_group("IndexId")`
  显式组写不 bump desired（无已知调用方）。
- 下一前置：SP-16（当前组持久化归属：本卡只保往返与解析，Dart 组切换落盘属 SP-16）、
  SP-12（重试链复用本卡 epoch 语义）、SP-30（正式 GUI 备份→恢复全流）。
- 阻塞（需根整合者）：无新增共享接口需求。本卡新增 `set_current_group` /
  `resolve_startup_selection` / `resolve_current_group` / `repair_default_selection`
  均为 application 内 `pub`，Dart 如需显式组切换走 FRB，需整合者排期
  （当前 Dart 组状态为 UI 本地源，CP-SET-11 已知）；`sp_03_backup_contract_test.dart`
  未建——本卡零 Dart 改动，Dart 侧 pending→active→首行与恢复重载已有
  R4-28 合同覆盖（9/9），新建 Dart 文件无对应生产变更可锁，归入 SP-30 E2E。
