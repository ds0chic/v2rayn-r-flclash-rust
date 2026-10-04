# R3-SET-01 — 重复导入保留幂等 no-op，配置按批次读取（P1）

状态：`implemented`（Rust 存储/服务层 A→B→A 幂等与批次配置有单测；未做真实 FRB+窗口整链实测）。

任务 ID：R3-SET-01

本次唯一用户流程：已导入来源 A（node-a/Dark），再导入 B（node-b/Light），再次导入未变的 A。第三次必须保持已导入的幂等 no-op，不得用全局最后一次 B 的 `upstream_config` 以 A 的指纹重映射，导致 active 悬空、主题错乱。

前置任务及已验证证据：冻结版 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始 HEAD `6699c31`；复核见 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` R3-SET-01（合成复现 `A_again=AlreadyImported; theme=Light; active_exists=0`）。

上游对照：`U/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:128/137/141`；本项目迁移幂等合同 `crates/persistence/src/candidate.rs`、`crates/persistence/src/report.rs`。

对应 feature / field / action：F-SUB-003、F-BACKUP-001..004；`AlreadyImported`、`upstream_config`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`BackupService::import_upstream_merge`（迁移/合并）与 `import_upstream`（替换恢复）。
- 输出：`Imported` / `AlreadyImported` 真实状态；成功提交才激活配置。
- 错误：`AlreadyImported` 不再触发 `activate_and_install`，不写 live 配置、不装资源。
- 持久化：`build_candidate` 额外写 `upstream_config:<fingerprint>` 批次键；`activate_upstream_config` 优先读该键，回退全局键。

允许修改的模块：`crates/persistence/src/{candidate.rs,report.rs}`、`crates/application/src/{backup_service.rs,engine.rs,subs.rs}`、`crates/bridge_api/src/api/{t16.rs,subs.rs}`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：不伪造导入结果；`Custom`/凭据不写日志；不改 `frb_generated`。

测试夹具和原版预期：合成上游 DB（ProfileItem + guiNConfig），无真实节点/订阅。原版预期：所选来源整体生效、重复导入幂等。

本次必须通过的命令/真实场景：
- `cargo fmt -p persistence -p application -p bridge_api -- --check`
- `cargo test -p persistence --lib --locked`
- `cargo test -p application --lib --locked`
- `cargo test -p application --test t16_backup --locked`

证据文件位置：`docs/evidence/recheck-fixes/R3-SET-01-04/`。

完成条件：A→B→A 后 `ProfileItem` 计数不变、live `active_index_id` 指向库中真实存在的行、主题与 active 一致；批次配置键保留 A 自身配置。门禁通过。

接口缺口（登记）：`t16_backup_import_upstream` 目前即替换恢复；独立迁移入口 `t16_backup_import_upstream_merge` 已在 Rust/bridge 注册但未接入 Dart（`lib/bridge/api/**` 禁止修改，需 FRB 重生成后接线）。

本轮实际结果：`import_from_path` 拆分为 merge/`restore_from_path`；`build_candidate` 写批次配置键；`activate_upstream_config` 优先读批次键；`import_upstream_inner` 仅 `Imported` 才激活。单测 `candidate::tests::reimport_of_earlier_source_is_noop_and_keeps_scoped_config`、`backup_service::tests::merge_import_a_b_a_is_idempotent_and_active_stays_resolvable` 通过；application lib 218/218、t16_backup 16/16。
