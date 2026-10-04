# R3-PROF-11 — Reality 空指纹保存时固化默认值

状态：`implemented`（Rust 单 crate 合成测试本轮已跑并绿；未做真实 Windows 窗口 + FRB + 真 SQLite 端到端，故不写 `verified`）。

任务 ID：R3-PROF-11

本次唯一用户流程：保存或导入一个 `StreamSecurity=Reality` 且 Fingerprint 为空的节点时，把「当时」设置里的 `CoreBasicItem.DefFingerprint` 固化写入该 profile；此后修改默认指纹，旧节点值不变；重开与导出一致。

前置任务及已验证证据：第三轮复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-11 行与 RE-PROF-07；冻结 `UP/ServiceLib/Handler/ConfigHandler.cs:1201-1248` 的 `AddServerCommon`，其中 `:1214-1216` 仅当 `Fingerprint.IsNullOrEmpty() && StreamSecurity==Reality` 时 `Fingerprint = config.CoreBasicItem.DefFingerprint`。既有 `crates/application/tests/re_prof_07_normalize.rs` 覆盖普通协议归一（不回归）。

对应 ID：`FLD-CFG-028`（`CoreBasicItem.DefFingerprint`，`crates/domain/src/settings_timing.rs:86`）、`F-PROFILE-*`、Reality 指纹字段。

必读上游文件、符号和固定 commit：UP `7d6a967`；`UP/ServiceLib/Handler/ConfigHandler.cs:1201-1248`、Reality 配置读取 fallback（本仓 `crates/config_codegen/src/xray/outbound.rs:696-701`、`crates/config_codegen/src/singbox/outbound.rs:843-852`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：save/import 的 `Profile`（`security.stream_security == "reality"`、`security.fingerprint` 空）+ 当时 settings 的 `core_basic_item.def_fingerprint`。
- 输出：persisted profile 的 `security.fingerprint = Some(def)`（默认非空时）；显式指纹与非 Reality 节点不变。
- 错误：无（纯补默认值，不新增校验错误）。
- 取消：不适用（保存路径同步）。
- 权限：仅本机 Rust/SQLite；不启动内核、不监听端口、不写系统代理/TUN。
- 持久化：走既有 `repo.upsert`，产物存 `ProfileItem.Fingerprint`。
- 生效：CODEGEN 在节点指纹非空时直接使用该值，不再回落到「当前」默认；导出的分享链接/完整配置读 persisted 值。

允许修改的模块：`crates/application/src/{custom.rs,engine.rs,codegen.rs}`、`crates/application/tests/r3_prof_11_reality_fingerprint.rs`、本卡、`docs/evidence/recheck-fixes/R3-PROF-11-R3-09/**`、`compat/fields.yaml`（仅追加）。
禁止改变：`features/**` 其它文件、`crates/application/src/{subs.rs,backup_service.rs,speedtest.rs,groups.rs,monitor.rs}`、`crates/subscriptions/**`、`crates/updater/**`、`services/**`；不改 FIX-11/11B/11C 统计轮询、Clash、日志语义；不改 FIX-15 托盘委派结构。

测试夹具和原版预期：合成 Vless Reality 节点（RFC 5737 地址、假 UUID）、临时 SQLite；先设默认 `chrome` 保存空指纹节点，断言落库为 `chrome`；再把默认改 `firefox`，断言旧节点仍 `chrome`，重开后仍 `chrome`。原版预期：保存即固化，后续改默认不影响旧节点。

本次必须通过的命令/真实场景：
- `cargo fmt -p application -- --check`
- `cargo test -p application --locked --lib custom::tests`
- `cargo test -p application --locked --test r3_prof_11_reality_fingerprint`
- 真窗 + 真 SQLite 重开对照（本轮未做，登记）。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-11-R3-09/`。

完成条件：保存/导入空指纹 Reality 固化当时默认；改默认后旧节点不变；重开一致；显式指纹与非 Reality 不被改写。未真窗对照，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`Profile.security.fingerprint` 经 CODEGEN 后为 `String`，无法区分「显式空串」与「未设置」；若配置默认本身就是空串，则节点侧固化不会生效（CODEGEN 仍回落当前默认）。上游默认指纹为非空值，故本轮以非空默认覆盖主合同；如需空默认也可固化，需要 CODEGEN 侧引入显式哨兵。
- 接口缺口（登记）：`normalize_server` 是纯函数，未直接读 settings；本次在 `engine.rs` 保存入口读取 settings 后调用 `apply_reality_fingerprint_default`，与冻结 `AddServerCommon(config, ...)` 的取值时机一致。若未来有其它保存入口（非 `save_profile`/`save_imported_profile`），需同样接线。

本轮实际结果：`crates/application/src/custom.rs` 新增 `apply_reality_fingerprint_default`（仅 Reality 且指纹空时写入 default）及 4 个单元测试；`crates/application/src/engine.rs` 在 `save_profile`/`save_imported_profile` 普通协议分支应用该固化，并新增私有 `default_reality_fingerprint` 读取 settings；新增集成测试 `crates/application/tests/r3_prof_11_reality_fingerprint.rs`（冻结/改默认不变/重开/导入/显式与非 Reality）。命令：`cargo fmt -p application -- --check` → 干净；`cargo test -p application --locked --lib custom::tests` → 9/9（含 4 个新用例）；`cargo test -p application --locked --test r3_prof_11_reality_fingerprint` → 4/4。
