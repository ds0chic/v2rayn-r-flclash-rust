# R3-MISC 修复证据（R3-08 / R3-PROF-06 / R3-PROF-09 / R3-SET-05 / R3-SET-06 / R3-ROOT-01）

日期：2026-10-04。基线 HEAD：`593e289`。冻结原版 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本目录只记录本轮实际运行的命令与边界；未运行的门禁与平台行为在末尾明确列出。

## 改动文件

- `crates/application/src/update_service.rs`（R3-08：`release_repo_for` / `app_source_unconfigured_check` + 2 测试）
- `crates/bridge_api/src/api/t16.rs`（R3-08：`t16_check_updates` 对 App 分流 + blocked 行 + 1 测试）
- `crates/bridge_api/src/api/speedtest.rs`（R3-PROF-06：`raw_custom_export` / `custom_export_file_name` + 1 测试）
- `apps/desktop/lib/features/update/update_controller.dart`（R3-08：blocked 汇总文案）
- `apps/desktop/lib/features/update/check_update_view.dart`（R3-08：blocked 检查行渲染）
- `apps/desktop/lib/features/profiles/profile_actions.dart`（R3-PROF-06：原始扩展名选择器 + Custom 剪贴板显式失败 + 文件建议名）
- `apps/desktop/lib/features/profiles/profile_fields.dart`（R3-PROF-09：连接 Address，`SecureSocket.secure` 设 SNI）
- `apps/desktop/lib/features/settings/hotkeys.dart`（R3-SET-05：冲突后保持 paused 并撤销实时注册）
- `apps/desktop/lib/features/subs/subs_actions.dart`（R3-SET-06：逐组汇总 + 失败详情）
- `apps/desktop/lib/features/runtime/runtime_controller.dart`（R3-ROOT-01：重入 pending + 空活动提示）
- `apps/desktop/test/recheck_r01_runtime_reload_test.dart`（更新为原版语义 + 2 新用例）
- `apps/desktop/test/support/counting_runtime_bridge.dart`（`applyGate` 测试闸）
- `apps/desktop/test/sr05_sr06_hotkey_test.dart`（+1 partial-failure 用例）
- `apps/desktop/test/r3_prof_06_export_custom_test.dart`（新增）
- `apps/desktop/test/r3_prof_09_cert_sni_test.dart`（新增，≥11808 本地 TLS ClientHello 夹具）
- `apps/desktop/test/r3_set_06_subs_summary_test.dart`（新增）
- `apps/desktop/test/r3_08_update_source_test.dart`（新增，blocked 检查行渲染）

## Rust 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p application --lib update_service::tests --locked` | exit 0，7 passed（含 `app_and_core_checks_use_separate_repositories`、`app_source_unconfigured_check_is_blocked_without_fake_release`） |
| `cargo test -p bridge_api --lib api::speedtest::tests --locked` | exit 0，8 passed（含 `raw_custom_export_preserves_bytes_and_original_extension`） |
| `cargo test -p bridge_api --lib api::t16::tests --locked` | exit 0，4 passed（含 `check_updates_blocks_app_target_without_own_source`，不联网） |
| `cargo fmt -p application -p bridge_api -- --check` | clean |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | clean |

## Flutter 命令与结果（工作目录 `apps/desktop`）

| 命令 | 结果 |
|---|---|
| `dart format <changed files>` | 13 files，5 changed（仅格式化） |
| `flutter analyze` | No issues found |
| `flutter test test/recheck_r01_runtime_reload_test.dart` | 4/4 |
| `flutter test test/sr05_sr06_hotkey_test.dart` | 7/7 |
| `flutter test test/r3_prof_06_export_custom_test.dart` | 2/2 |
| `flutter test test/r3_prof_09_cert_sni_test.dart` | 1/1 |
| `flutter test test/r3_set_06_subs_summary_test.dart` | 2/2 |
| `flutter test test/r3_08_update_source_test.dart` | 1/1 |
| `flutter test test/re_prof_08_export_udp_test.dart`（回归） | 5/5 |
| `flutter test test/t16_update_test.dart`（回归） | 7/7 |
| `flutter test test/recheck_rr04_app_update_test.dart`（回归） | 4/4 |
| `flutter test test/sr01_subs_report_test.dart`（回归） | 1/1 |

## 关键结论与边界

- R3-08：`v2rayN` 目标走 `check_app_update`（app_repo）；未配置时返回 `supported=false` + `note=error.update_app_source_unconfigured`，无 remote/asset；核心仍查各自 frozen repo。未联网、未使用 app_repo override 环境变量。
- R3-PROF-06：Custom 原始文本经 `raw_custom_export` 逐字节返回；Dart 选择器按 `address` 原始扩展名；Custom 剪贴板显式失败（上游 `fileName=null` → `CheckServerSettings`）。真实文件写入仍走 `write_export_file`（`std::fs::write`，无 BOM）。
- R3-PROF-09：连接 `Address:Port` 后用 `SecureSocket.secure(socket, host: SNI)`；本地夹具监听 11808（先探测）并断言 ClientHello 携带文档域名 `www.example.test`，未做域名 DNS。
- R3-SET-05：partial failure 后 `_paused=true` 且撤销成功组合的实时注册；`cancelEdit` 恢复；fake registrar，无真实 OS 热键。
- R3-SET-06：主菜单/当前组 toast 复用逐组 `subsUpdateSummary`，存在失败行时弹出详情对话框；未跑真实菜单网络。
- R3-ROOT-01：重入记录 pending 并在完成后补跑一次；无活动节点 `setMessage('配置项无效，请检查或重新选择')`（上游 `CheckServerSettings`）。

未运行：真实远端 App 发行源下载/验签/覆盖、真实浏览器/系统 PAC、真实 TLS 证书链（仅叶子）、真实 OS 全局热键、真实 Flutter+FRB 菜单订阅部分失败下载、F5 在真实运行中的重复按键、全 workspace 门禁、`flutter build windows`、安装器。未碰 10808，未改系统代理/注册表/TUN，未读用户凭据；夹具均为合成/文档数据。
