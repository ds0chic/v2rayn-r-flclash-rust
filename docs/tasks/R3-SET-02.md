# R3-SET-02 — “本地恢复原版 ZIP”按原版整体替换 DB/config（P1）

状态：`implemented`（Rust 存储/服务层替换语义与失败回滚有测试；未启动真实本地恢复选择器与窗口）。

任务 ID：R3-SET-02

本次唯一用户流程：现有数据含节点 A，选择只含节点 B 的原版 backup ZIP 执行“本地恢复”，恢复后的节点集必须来自所选备份（A 不存在、B 存在），而不是 candidate 合并成 A+B。

前置任务及已验证证据：冻结版；开始 HEAD `6699c31`；复核见 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` R3-SET-02（A→B 得 rows=2，确认为合并）。

上游对照：`U/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133/137`（退出/关 SQLite，解压同名 guiNDB 替换数据库）。

对应 feature / field / action：F-BACKUP-001..004；`guiNDB.db`、`guiNConfig.json`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：原版 `guiConfigs/` ZIP 或目录。
- 输出：`restore_from_path` 以空 candidate 仅导入源行后原子提交，替换 live DB/config。
- 错误/回滚：沿用 `commit_candidate` 的 `.bak` 回滚与两阶段 `activate_and_install` 回滚；失败保持旧 A。
- 合并导入保留：`BackupService::import_upstream_merge`（迁移流程），独立命名并注册 bridge 入口。

允许修改的模块：同 R3-SET-01。

禁止改变的已有行为：不伪造恢复结果；不改 `frb_generated`；不触碰用户真实备份/凭据。

测试夹具和原版预期：合成 A、仅含 B 的合成上游目录；恢复后断言 A 不在、B 在、active=按 B 指纹派生的 B 行。

本次必须通过的命令/真实场景：
- `cargo test -p persistence --lib --locked candidate::tests::restore_replace`
- `cargo test -p application --lib --locked backup_service::tests::restore_replace`
- `cargo test -p application --test t16_backup --locked`

证据文件位置：`docs/evidence/recheck-fixes/R3-SET-01-04/`。

完成条件：A + 仅 B 源恢复 → 重开后 A 不存在、B 存在；activation/resource 失败回滚保持 A、B 均不残留半成品。门禁通过。

本轮实际结果：新增 `persistence::candidate::restore_from_path`（`ImportMode::Replace`，candidate 不再复制现有库）；`BackupService::import_upstream` 切到替换语义，`import_upstream_merge` 保留合并；t16_backup 16/16（含原有回滚用例）通过；新增 `restore_replace_*` 两测通过。
