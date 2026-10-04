# R4-27 存储迁移和失败恢复 — 证据

状态：implemented（本卡门禁本地通过；全工作区 cargo test 存在一条与本卡无关的既有失败，见下）。

- 审查基线 HEAD：`ef02954`（工作树在本卡开始时干净；并行协作者在我开始后修改了 `profiles_table.dart`、`status_bar_view.dart`、`app_theme.dart` 等无关文件，未触碰）。
- 冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；应用审查基线 `77c74ed`。
- armed=false；未启动内核；未监听/连接任何端口（含 10808）；未改系统代理/注册表/路由/TUN/自启；未读取用户凭据；夹具全部为合成数据。

## 本卡完成的行为

1. **production fail-closed（D20）**：`crates/bridge_api/src/api/engine.rs`
   - 生产 `engine()` 打开存储失败时不再 `AppEngine::in_memory()` 降级；改为记录结构化错误并返回 fail-closed 引擎。
   - `AppEngine::storage_unavailable`（`crates/application/src/engine.rs`）让 `snapshot`/`query_profiles`/`save_profile`/`save_profile_imported`/`delete`/`copy`/`set_remarks`/`set_active`/`load_settings`/`save_settings`/`apply_runtime`/`profile_ex_*` 全部返回该存储错误；NullRuntime 永不产生假 `Accepted`。
   - `init_engine` 打开失败返回 `ok:false` + `ErrorDto`（复用既有 `SimpleResult`，无 FRB 变更）；失败不写入 `ENGINE`，修复后可重试。
   - `get_snapshot` 存储失败返回带 `runtime_error` 的结构化快照，不再返回空快照（D22）。
2. **错误可读/可重试（D22）**：`crates/application/src/store_repo.rs` 新增 `persistence_storage_error`，保留 `E_PERSIST_IO/SQLITE/...` 稳定码与 `error.persist_*` message key，并标记可重试；打开路径与读写路径改用该分类。
3. **候选失败保原件**：既有 `commit_candidate` 六步 gate 与 `failed_validation_leaves_target_untouched` 保留，未回退。
4. **UI 状态写数据目录 + 原子 + 迁移 + 失败可见（D21）**：`apps/desktop/lib/features/profiles/ui_state_store.dart`
   - `FileUiStateStore` 默认路径改为数据目录（`V2RAYN_R_DATA_DIR` 或 `%LOCALAPPDATA%\v2rayn-r\data`），不再写 exe 旁。
   - 写盘改为同目录临时文件 + `rename` 原子替换；失败记录 `lastWriteError` 并回调 `onError`，不再静默吞错。
   - 读取时若新路径缺失则读取旧 exe 旁 `ui_state.json` 并迁移写入数据目录。
5. **未知键保留**：UI 状态 section 保存保留未知顶层键；persistence 既有 `unknown_columns_are_retained_in_raw_records` 覆盖导入侧未知列保留。

## 断言先行（先失败后通过）

- 新测试：`apps/desktop/test/repair/r4_27_repro_test.dart`（契约断言）。
- 现版（`git stash` 临时还原 `ui_state_store.dart` 到 HEAD）运行该文件：编译失败（缺 `resolveDefaultPath`/`resolveLegacyPath`/`onError`/`legacyOverridePath`），即“现版失败”。
- 修复后同文件 4/4 通过；`apps/desktop/test/r4_27_contract_test.dart` 5/5 通过（见 `commands.log`）。

## 已知未完成 / 接口缺口

- 全工作区 `cargo test --workspace --locked`：唯一失败 `crates/application/tests/t18b_runtime_e2e.rs::rr07_custom_config_plan_serves_on_its_real_port`（真实内核启动即退出 `0xffffffff`）。已在把本卡 4 个 Rust 文件 `git stash` 回到 `ef02954` 后复跑，同样失败 → **本卡引入前既存**，非回归，未修（超出本卡范围）。
- `SnapshotDto`/`ProfilePageDto` 无通用错误字段且不得改 FRB：`query_profiles` 失败仍只能返回空页（D22 的该分支未端到端可见）；存储错误统一经 `init_engine.error` 与 `get_snapshot.runtime_error` 暴露。已登记为接口缺口，需根代理在允许 FRB 重生成时增补。
- routing/dns/subs 的 bridge 模块与本卡无关且不在允许修改范围，其读写路径未加 fail-closed 门（引擎内存后端仍被这些模块使用）；生产首要运行/资料路径已 fail-closed。
- 未做真实 GUI/内核验收（本卡存储行为以单元/契约测试与 release 构建为准）。

## 工件

- 证据：`docs/evidence/repair/R4-27/{README.md,observations.json,commands.log}`。
- 测试：`apps/desktop/test/r4_27_contract_test.dart`、`apps/desktop/test/repair/r4_27_repro_test.dart`、Rust 侧新增于 `engine.rs`/`store.rs`/`bridge engine.rs` 的 `storage_*` 测试。
