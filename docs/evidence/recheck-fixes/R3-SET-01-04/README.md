# R3-SET-01..04 修复证据

基线：HEAD `6699c31`（工作树另含先前已存在的 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` 改动，未回退）。
复核来源：`docs/evidence/parity-recheck-2026-10-04/round3-settings.md`。
本目录仅合成数据；未启动真实核心、未写宿主代理/注册表/TUN、未使用 10808、未注册真实 OS 热键。

## 结论摘要

- R3-SET-01：重复导入为幂等 no-op；每批次 `upstream_config:<fingerprint>` 独立保存、激活时优先读取。A→B→A 后 active 仍指向 B 且存在于库中。
- R3-SET-02：`restore_from_path`（`ImportMode::Replace`）使“本地恢复原版 ZIP”整体替换 DB；合并导入改为独立 `import_upstream_merge`。
- R3-SET-03：scheduler tick 可取消、手动任务可 cancel/drain；`prepare_restore` 先 bump restore epoch，stale 提交被拒绝；超时返回结构化错误并阻断恢复。
- R3-SET-04：恢复后 `HotkeyController.reloadFromSettings()` 重载并重注册恢复后的 `GlobalHotkeys`。

## Rust 命令与结果（`rust-tests.log`）

```
cargo fmt -p persistence -p application -p bridge_api -- --check      # exit 0
cargo test -p persistence --lib --locked                              # 65 passed; 0 failed
cargo test -p application --lib --locked                              # 218 passed; 0 failed
cargo test -p application --test t16_backup --locked                  # 16 passed; 0 failed
cargo clippy -p persistence -p application -p bridge_api --all-targets --locked
# 改动文件无新增 warning；仓库原有 warning 在 codegen.rs/monitor.rs/r3_02_prescreen（非本卡文件）
```

新增/相关用例：`candidate::tests::restore_replace_removes_existing_and_keeps_source`、
`candidate::tests::reimport_of_earlier_source_is_noop_and_keeps_scoped_config`、
`backup_service::tests::merge_import_a_b_a_is_idempotent_and_active_stays_resolvable`、
`backup_service::tests::restore_replace_drops_existing_and_keeps_only_source`、
`subs::tests::scheduler_stop_cancels_slow_download_before_commit`（合成慢响应，bind 11808..11949）、
`engine::tests::replace_sub_profiles_rejects_stale_restore_epoch`、
`engine::tests::cancel_and_drain_sub_tasks_times_out_on_stuck_job`、
`engine::tests::prepare_restore_bumps_epoch_before_exchange`。

## Flutter 命令与结果（`flutter-tests.log`）

```
flutter analyze                                                       # No issues found
flutter test test/r3_set_restore_test.dart                            # 1 passed
flutter test test/sr05_sr06_hotkey_test.dart                          # 6 passed
flutter test test/sr02_04_backup_import_test.dart                     # 1 passed
flutter test test/sr03_restore_lifecycle_test.dart                    # 4 passed
flutter test test/fix14b_backup_restore_test.dart                     # 5 passed
flutter test test/fix09d_scheduler_ctrl_test.dart                     # 1 passed
flutter test test/t16_backup_test.dart                                # 6 passed
flutter test test/fix14_backup_activate_test.dart                     # 3 passed
flutter test test/fix14c_webdav_backup_test.dart                      # 4 passed
```

`test/r3_set_restore_test.dart` 使用 fake registrar 与可切换 settings 文档，覆盖 K→恢复 J→立即 J→打开/取消仍 J；未注册真实 OS 热键。

## 验证边界（未运行）

- 真实 FRB + 窗口的整链恢复、真实本地恢复选择器、真实 OS 热键组合、真实核心/PAC。
- 真实窗口下“慢下载与恢复”并发；本轮为合成慢响应 + 存储层 epoch 断言。
- 回滚动作自身失败（沿用旧报告边界）。
