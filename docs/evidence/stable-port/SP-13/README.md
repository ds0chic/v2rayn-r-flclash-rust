# SP-13 路由部分提交后关闭 — 证据

状态：implemented（定向检查绿；真实原生/正式包/OS 隔离验收未跑，不写 verified）。

基线：`788adf6`。本次不 commit。合成数据；内存 fake；无 socket；不碰宿主
路由/DNS/网络/系统代理；`work/`、`outputs/` 只读未动；`compat/` 未动。

## 0. 缺陷依据（CP-08）与前置

- CP-08：路由规则读失败/畸形快照仍当可写空规则；多选删除 a 成功、b 失败后
  再确定，旧全稿复活 a。对应 `docs/evidence/complete-port-audit-2026-10-06/README.md`。
- 上游：`ServiceLib/ViewModels/RoutingSettingViewModel.cs`
  （`RoutingAdvancedRemoveAsync` 逐个删无旧全集回写；`SaveSettingsAsync` 只存
  DomainStrategy 两字段）。审计基线与 HEAD 见任务卡。
- 前置 SP-12 已 implemented：本卡复用其 `saveGroup` 保存语义与分阶段
  receipt/重试合同（`settings_controller.dart` 未碰，无新增 DTO/FRB）。

## 1. 改动文件（写锁内）

- `apps/desktop/lib/features/routing/routing_windows.dart`：
  `RoutingDraft/RoutingDraftDecoded` 新增 `incrementalCommitted`（缺省 false，
  编解码兼容）；独立窗 `_ok` 在 host 具增量能力时置 true。
- `apps/desktop/lib/features/routing/routing_actions.dart`：
  新增纯函数 `routingCommitDeletes`（增量确定零删除，旧窗才回放缺失删）、
  `verifyRoutingIncrement`（delete 验缺席/save 验存在/setDefault 以权威
  active 为准，不一致即失败且不重放）；`_applyRoutingDraft` 增量分支只存
  两策略字段 + 权威重查（不再 save/delete 方案、不盲目 setDefault）；
  旧窗分支保持历史全量回放；`_applyRoutingAction` 每次增量写后显式
  `reload()` + 校验，再走 runtime reload。
- `apps/desktop/lib/features/routing/dns_controller.dart`：
  `DnsState.loadFailed` + 纯函数 `loadFailedFor`（仅无可用基线时成立，
  部分失败有缓存仍可编辑）；`reload()` 计算并成功时清零。
- `apps/desktop/lib/features/routing/dns_window.dart`：
  `_save` 在 `loadFailed` 时拒绝写（提示重试），取消仍可关。
- `crates/application/src/routing.rs` / `dns.rs`：各新增
  `sp13_partial_delete_reconciles_against_authoritative`（已提交删不复活、
  失败项保留可重试）。
- 新增 `apps/desktop/test/repair/sp_13_reconcile_test.dart`（7 项，见 §3）。
- 未改：engine.rs、lib.rs、settings_controller.dart、IPC/stable DTO、
  BridgePort、FRB 生成物、Cargo 锁。

## 2. 命令与 exit（本次范围）

- `flutter test test/repair/sp_13_reconcile_test.dart`：先红（缺符号编译失败），
  修后 7/7 通过，exit 0。
- 受影响既有（逐文件单跑）：sp_13_partial_commit 4/4；r3_wpf_routing_window
  6/6；sp_11 12/12；fix08_routing_draft 2/2；fix08c 3/3；fix08_dns_draft 3/3；
  fix08b_dns_apply 4/4；r3_wpf_routing_structure 2/2；t11_routing 6/6；
  t11_dns 5/5。合计 47/47，exit 0。
- `dart format --set-exit-if-changed`（本卡 Dart 文件）：exit 0。
- `flutter analyze`（apps/desktop）：No issues found，exit 0。
- `cargo fmt -p application -- --check`：本卡 routing.rs/dns.rs 零 diff；
  包级剩余 diff 在 `tests/sp25_root_cert_trust.rs`（他卡在途，未碰）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：
  本卡两文件 0 issue；剩余 error 全在 `subscriptions/tls.rs`、`updater`
  （他卡在途，写锁外，未碰）。
- `cargo test -p application --locked --lib`：319 pass / 0 fail，exit 0；
  其中 sp13 4/4、routing 9/9、dns 11/11。
- `cargo test -p application --locked`（全 target）：被他卡
  `tests/sp25_root_cert_trust.rs` + `updater` 编译失败阻塞（E0599/E0631），
  本卡 Rust 单测已由 `--lib` 覆盖执行。

## 3. 新增测试（7/7 pass，不得改弱）

`test/repair/sp_13_reconcile_test.dart`（合成数据，内存 fake）：

1. 增量窗草稿带已提交标记，旧窗草稿保持全量语义（编解码往返）。
2. 增量确定不从旧全集删方案（含并发新增保留）；旧窗回放才删缺失项。
3. 每次增量提交权威验一致：删后仍在/save 缺席/active 不符即失败不重放。
4. DNS 读失败判定只在无可用基线时成立。
5. 增量窗确定带标记且关闭；旧窗不带标记（State 复用隔离）。
6. 部分删除后重开不复活（同 host 新窗只列权威）。
7. DNS 读失败时保存不写（桥基线不变、窗不关），取消可关。

## 4. 语义保证

- 增量提交成功即事实；失败/未知项保留可重试，从不重放非幂等写。
- 主确定（增量窗）仅回写原版两策略字段，不回写旧全集；active 提升以权威为准。
- 读失败/畸形只进重试页（路由窗无确定按钮；DNS 窗拒绝保存），取消/关闭只
  `close()`，不补写全量草稿，不谎称未改变。
- SP-12 receipt/重试合同复用现状：策略保存走既有 `saveGroup` 路径；
  跨层稳定 DTO/版本冻结留给 SP-00 整合者（本次无新增共享接口）。

## 5. 未运行 / 未验证 / 阻塞（登记，不私改他卡文件）

- 未运行：`flutter build windows --release`（无 native/AOT 接线变更）；
  真实 FRB/SQLite 独立重开、真实 DB 部分失败、save 成功 apply 失败、
  并发双窗、OS 隔离验收（需整合候选，留 SP-34/整合者）。
- 未验证：六平台中非 Windows 单元；24h/500 切换。
- 阻塞：`cargo test -p application` 全 target 与 workspace clippy 被他卡
  `updater`/`subscriptions`/`sp25_root_cert_trust.rs` 在途破坏阻塞；
  本卡文件 fmt/clippy/单测干净。t11 与 fix08_dns 在多文件并跑曾现瞬态
  “did not complete”（SP-29 已知 runner 隔离问题），逐文件单跑全绿。
