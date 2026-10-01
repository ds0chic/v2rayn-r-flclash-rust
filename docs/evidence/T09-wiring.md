# T09 接线 — 订阅 CRUD/更新管线/调度 + 节点导入导出

状态：**implemented**（Rust 与应用层闭环；端到端真实网络验证归后续，见 §6）。
范围：`crates/application/**`、`crates/bridge_api/**`（含 FRB 再生成）、
`apps/desktop/lib/**`、`apps/desktop/test/**`、`apps/desktop/pubspec.yaml`、
`crates/subscriptions`（仅追加 `to_inner_uri` 导出，未改行为）、
`compat/features.yaml` 与 `compat/fields.entities.yaml`（仅状态/证据追加）、
本文件与 `docs/decisions/T09-wiring.md`。**未 commit。**

> 使命边界：本回合 T09 接线子代理独占 FRB 再生成，其他代理未触碰生成物。

## 1. 文件与接线点

### Rust 应用层（订阅持久化 + 用例）
| 文件 | 内容 |
|---|---|
| `crates/application/src/subs.rs`（新） | `SubItem`（17 字段）与 `to_row`/`from_row`、`validate`（URL/Headers/端口/内核）、`parse_request_headers`、`parse_subscription`（复用 `subscriptions::parse_content`）、`download_all`（主 URL + MoreUrl 合并、经代理回退）、`build_candidates`（候选集构建）、`is_due`、`SubScheduler`/`run_scheduler_tick`、结构化 `SubUpdateOutcome`/`SubUpdateReport`/`report_to_json` |
| `crates/application/src/repository.rs` | 新增 `SubRepository` trait 与 `InMemorySubRepository` |
| `crates/application/src/store_repo.rs` | 新增 `SqliteSubRepository`（共享 `Store`）与 `SubStore`（Memory/Sqlite） |
| `crates/application/src/engine.rs` | 引擎新增 `subs`/`sub_scheduler`/`local_proxy_port`；用例 `list/get/save/delete_sub_item`、`set_sub_enabled`、`reorder_sub_items`、`profiles_by_subid`、`replace_sub_profiles`、`touch_sub_update_time`、`refresh_subscriptions`（候选先构建，失败/空保留旧节点，成功按 subid 先删后写）、`set_local_proxy_port`/`local_proxy_url`、`start/stop_sub_scheduler` |
| `crates/application/Cargo.toml` | 新增 `subscriptions`、`tokio`（rt/time/sync） |

### Rust 桥接层（FRB API）
| 文件 | 内容 |
|---|---|
| `crates/bridge_api/src/api/contract.rs` | 新增 DTO：`SubItemDto`、`SubItemDtoResult`、`SubsPageDto`、`DeleteSubsResult`、`SubUpdateEntryDto`、`SubUpdateResult`、`ParseIssueDto`、`ImportResult`、`UriParseResult`、`ShareExportResult` |
| `crates/bridge_api/src/api/subs.rs`（新） | `list/get/save/delete_sub_items`、`set_sub_enabled`、`reorder_sub_items`、`validate_sub_item`、`set_local_proxy_port`、`update_subscriptions`/`update_subscription`（注册 job + token 贯通取消，`cancel_job` 可达）、`start/stop_sub_scheduler`、`sub_scheduler_running`、`job_view`、`import_from_text`、`parse_share_uri`、`export_profiles`（share/base64/inner）、`write_export_file` |
| `crates/bridge_api/src/api/engine.rs` | `engine`/`error_dto`/`emit_control`/新增 `profile_dto`/`job_view_dto` 提为 `pub(crate)` 供 subs 复用 |
| `crates/bridge_api/Cargo.toml` | 新增 `subscriptions` 依赖 |
| FRB 生成物 | `crates/bridge_api/src/frb_generated.rs`、`apps/desktop/lib/bridge/**`（`flutter_rust_bridge_codegen generate` 生成，二次 NO_DIFF） |

### Flutter 应用层
| 文件 | 内容 |
|---|---|
| `lib/bridge/bridge_port.dart` | `BridgePort` 抽象新增订阅/导入导出面；`FrbBridgePort` 转发真实桥接；`SyntheticBridgePort` 实现确定性内存版本（测试用） |
| `lib/features/subs/subs_controller.dart` | `SubsController`/`SubsState`/`SubStatus`：列表、选择、保存、删除、启停、排序、更新（job/取消）、调度启停、结构化状态 |
| `lib/features/subs/sub_setting_window.dart` | `SubSettingWindow`（ACT-MAIN-019）：列 Remarks/Url/Enabled/AutoUpdateInterval/UserAgent/Sort，按钮 新增/删除/编辑/分享/关闭，右键菜单，状态行 |
| `lib/features/subs/sub_edit_window.dart` | `SubEditWindow`：全部 17 字段编辑 + 本地与桥接双重校验；取消不落库 |
| `lib/features/subs/sub_share_dialog.dart` | `sub_share_dialog`：订阅 URL 二维码（`qr_flutter`） |
| `lib/features/subs/subs_actions.dart` | 剪贴板导入、文本粘贴导入、分享/Base64/内部 URI 导出、导出到文件、节点二维码分享、打开订阅设置、更新全部/当前组（直连/经代理） |
| `lib/app/shell/main_shell.dart` | 接线 ACT-MAIN-016/017/018/019/020/021/022/023；Ctrl+V 剪贴板导入、Ctrl+C 分享、Ctrl+S 节点二维码；`V2RAYN_R_OPEN_SUBS` 证据钩子（release 可截订阅窗口） |
| `pubspec.yaml` | 新增 `qr_flutter: ^4.1.0`（+ `qr`） |

## 2. 门禁结果（真实运行）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | EXIT 1：仅 `crates/config_codegen/tests/*.rs`（**非 T09**，并行代理脏文件）有 diff；T09 全部文件无 diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | EXIT 0 |
| `cargo test --workspace --locked --no-fail-fast` | EXIT 0（全绿） |
| `flutter_rust_bridge_codegen generate`（连续二次） | `frb_generated.rs` 哈希一致 → **NO_DIFF** |
| `dart format --output=none --set-exit-if-changed lib test` | EXIT 0 |
| `flutter analyze` | No issues found |
| `flutter test`（逐文件） | 见 §3；批跑受 flutter_tester 引擎崩溃影响，逐文件可复现 |
| `flutter build windows --release` | EXIT 0 |
| release 截图 | `docs/evidence/screenshots/windows_flutter/t09_subs.png`（release 运行，订阅设置窗口） |

## 3. 测试统计（真实运行）

### Rust
| 目标 | 用例数 | 说明 |
|---|---|---|
| `crates/application/src/subs.rs` 单元测试 | 5 | headers 校验、validate、is_due 间隔、more_url 切分、行往返（17 字段） |
| `crates/application/tests/subs_pipeline.rs` | 9 | 本地 HTTP 服务（端口 11808 起）：成功替换、按 subid 先删后写、HTTP 失败保留、空结果保留、MoreUrl 合并、Headers 传递、Filter 过滤、取消、CRUD 重开一致 |
| `crates/bridge_api/src/api/subs.rs` 单元测试 | 3 | 导入分配 id/subid、自由文本拒绝、share/base64 导出 |

### Flutter（逐文件均为 pass）
| 文件 | 用例数 |
|---|---|
| `test/t09_sub_setting_test.dart` | 4 |
| `test/t09_sub_edit_test.dart` | 5 |
| `test/t09_import_export_test.dart` | 3 |

批跑说明（如实记录）：`flutter test` 全量批跑在 ~40 用例后 `flutter_tester` 崩溃
（exit 0xC0000005，无 Dart 栈），与 `docs/evidence/T01.md`/`tools/flutter_test_retry.ps1`
记录的锁定 Flutter 引擎缺陷一致。逐文件运行全部通过；4 个既有文件
（`profiles_keyboard_test`、`t05_profiles_ui_test`、`t05_shell_chrome_test`、
`t06a_editor_save_test`）在批跑中崩溃，单独重跑均 pass，非本次改动导致。

## 4. 台账更新 ID 清单

`compat/features.yaml` 置 `implemented`（严禁 verified）：

| 类别 | ID |
|---|---|
| 订阅 | F-SUB-001, F-SUB-002, F-SUB-003, F-SUB-004, F-SUB-005, F-SUB-006, F-SUB-007, F-SUB-008, F-SUB-009, F-SUB-010, F-SUB-011 |
| 导入导出 | F-IMPORT-001, F-IMPORT-002, F-IMPORT-005, F-IMPORT-008, F-IMPORT-009, F-IMPORT-010 |

`compat/fields.entities.yaml` 置 `implemented`（同时追加 implementation_location 与 test_ids）：

FLD-ENT-082（SubItem.Id）.. FLD-ENT-098（CustomCoreType），共 17 个字段。

未覆盖、保持 `identified`：F-IMPORT-003（SIP008 有解析但无独立 UI 入口证据）、
F-IMPORT-004（WireGuard conf 同上）、F-IMPORT-006/007（图片/屏幕扫码，本轮不做）、
F-IMPORT-011（全量配置识别为 Custom，UI 入口待补）。

## 5. 菜单与窗口接线（ACT-MAIN-019..023）

- ACT-MAIN-019：`订阅分组设置` → `SubSettingWindow`。
- ACT-MAIN-020：`更新全部订阅 (不通过代理)` → `update_subscriptions([], false)`。
- ACT-MAIN-021：`更新全部订阅 (通过代理)` → `update_subscriptions([], true)`。
- ACT-MAIN-022：`更新当前订阅 (不通过代理)` → 当前选中订阅。
- ACT-MAIN-023：`更新当前订阅 (通过代理)` → 当前选中订阅。
- 状态栏/窗口状态为**阶段文本与结构化提示**，无伪造百分比。

## 6. 未决项与后续前置

1. **扫码/图片导入**（F-IMPORT-006/007）：本轮不做，登记为未决；建议归 T15 或专门任务
   （需摄像头/图像管线与 QR 解码依赖）。
2. **端到端真实网络验证**：本回合仅用本地 loopback HTTP 服务（端口 ≥11808）验证管线；
   真实公网订阅、真实转换服务与真实内核联动归后续端到端任务，届时 F-SUB-* 才可 verified。
3. **转换服务 (ConvertTarget)**：解析/拼接逻辑保留在库层与 SubItem 字段，本轮 UI 未请求真实
   第三方转换服务（AGENTS 硬规则：不上报用户订阅到默认第三方）；仅复现用户已配置入口留待验证。
4. **核配置导出**（F-IMPORT-008 的客户端配置文件部分）：本轮仅导出分享 URI 文本到文件，
   内核配置导出留 T10+。
5. **`更新当前订阅` 依赖订阅窗口的选中态**：菜单直接触发时若无选中会提示先选择；
   与上游 `_config.SubIndexId` 的持久化选中语义可在后续补齐。
6. **调度默认不自动启动**：`SubScheduler` 与 `start_sub_scheduler` API 就绪，但应用启动未默认
   拉起（避免未验证的自动网络行为）；由后续任务按上游 TaskManager 行为接入。
