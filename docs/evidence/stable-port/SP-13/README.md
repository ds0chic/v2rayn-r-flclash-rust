# SP-13 准备证据(非完整卡完成)

状态：identified（准备范围已做，完整接线未做，不标 implemented/verified）。

基线：`92d46dd`。本次不 commit。

## 1. 准备范围（可独立部分）

- 原版对照：`work/.../7d6a967/.../ServiceLib/ViewModels/RoutingSettingViewModel.cs`
  - `RoutingAdvancedRemoveAsync` 逐个删 `SelectedSources` 后 `RefreshRoutingItems`；
    无旧全集回写。`SaveSettingsAsync` 只存 DomainStrategy 两字段。
  - 关闭语义以上游 `IsModified → Reload` 为准，本次只在窗口侧落实草稿语义。
- 故障夹具：合成数据 + 内存 fake（`PartialDeleteHost` 删 a 成功/删 b 失败；
  畸形快照文本），无真实路由/OS 副作用。
- 正确失败合同与实现（`apps/desktop/lib/features/routing/routing_windows.dart`）：
  - `_deleteSchemes` 部分失败时经 `_dropCommitted` 立即对账已提交 id，
    确定不再用旧全集复活；失败/未尝试项保留可重试。
  - `decodeRoutingSnapshot` 畸形（非 JSON/非 Map/无 List schemes/条目非 Map
    或无 id）抛 `RoutingEditorLoadException`，读失败只进重试页（无确定按钮，
    `_ok` 的 `_loadFailed` 守卫保留）。
  - 取消/关闭只 `close()`，不补写全量草稿。
- Rust 侧只加回归测试（`routing.rs` / `dns.rs` 内 `sp13_*`）：畸形输入在解析
  边界拒绝，内存仓不变（读失败不写）。未加新 API/DTO。

## 2. 等待 A04（SP-12）的部分

- 每次增量提交后对权威 snapshot 的重查对账（含 active 提升、并发/重开语义）。
- 主确定仅回写原版策略字段、不回写旧全集的完整接线，需 SP-12 的
  `SettingsSaveReceipt/newRevision` 与重试/查询合同（`settings_controller.dart`
  为 A04 独占，本次未碰）。
- 需要 receipt/版本合同时由 SP-00 整合者登记，本次无新增共享接口。

## 3. 实际运行命令与 exit

- `flutter test test/repair/sp_13_partial_commit_test.dart`：先红（2 失败：
  确定复活 a；畸形解码为空草稿），修后 4/4 通过，exit 0。
- `flutter test`（sp_13 + r4_14 + r4_12 + r3_wpf_routing_window）：23/23 通过，
  exit 0。`test/repair/sp_11_window_reply_test.dart`：12/12 通过，exit 0。
- `dart format --set-exit-if-changed`（本卡两 Dart 文件）：exit 0。
  `rustfmt --check`（routing.rs/dns.rs）：exit 0。
- `flutter analyze`：本卡文件 0 issue；全应用剩 1 warning 在
  `settings_controller.dart`（A04 在途文件，未碰）。
- `cargo fmt -p application -- --check`：本卡文件干净；包级失败来自他卡在途
  文件（SP-21 相关，非本卡）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：本卡两
  文件 0 issue；剩余 3 error 全在 `net_host_client.rs`（他卡 SP-07 在途大改，
  写锁外，未碰）。
- `cargo test -p application --locked`（routing/dns）：被上条 `net_host_client.rs`
  编译失败阻塞（`v.as_u64` 缺括号，他卡笔误），本卡 Rust 新增测试未能执行；
  待该文件恢复后补跑。

## 4. 改动文件

- `apps/desktop/lib/features/routing/routing_windows.dart`（对账 + 严格解码）
- `apps/desktop/test/repair/sp_13_partial_commit_test.dart`（新增，4 合同）
- `crates/application/src/routing.rs`（新增 `sp13_malformed_import_never_writes`）
- `crates/application/src/dns.rs`（新增 `sp13_malformed_dns_template_never_writes`）
- 本 README（证据）

## 5. 未完成/下一步前置

- 待 `net_host_client.rs` 恢复编译后补跑 `cargo test -p application --locked`
  的 routing/dns 用例（含本卡 2 个新增）。
- 待 A04（SP-12）交付 receipt/版本/重查合同后，由整合者接线完整 SP-13
  （权威对账 + 确定仅策略字段 + 真实 DB 部分失败/save 成功 apply 失败/并发/
  重开无复活），届时以真实入口复验。
