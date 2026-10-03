# FIX-04 — 原版 v2rayn:// 内部 URI 导入/再导出双向互通

状态：`implemented`（导入→导出→再导入往返、落库/重开、真桥 UI 链路已验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-04

本次唯一用户流程：拿到原版 `v2rayn://<type>/<base64url>` 内部链接（剪贴板/文本导入入口）→ 解析为节点 → 落库并在重开后存在 → 从节点表导出为内部链接（右键导出/内部链接）→ 再导入得到等价节点。完整 Xray/sing-box/Clash 配置导入是另一张卡，不在本卡范围。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `71dac24c0cb43a3d62da0887905bdf4ebda5b1a2`（工作树有并行子代理的未提交改动，本卡只改自己的文件，未回退他人改动）。审查结论见 `docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-04 行、`profiles-report.md` PR-08/PR-06/PR-09/PR-13、`root-report.md` ROOT-01、`runtime-report.md` RT-08。复现证据：`docs/evidence/UX-PARITY-FIX-04/repro-fail.log`（`parity_original_inner -- --ignored` 2/2 失败：PascalCase VLESS 被解析成默认 Vmess，导出 ConfigType 为 Null）。

对应 feature / field / action / layout ID：`F-IMPORT-005`（v2rayn:// 内部协议导入导出）、`ACT-MAIN-016`（从剪贴板批量导入）、`ACT-PROF-026`（批量导出内部链接）、`FLD`/实体 `ProfileItem`/`ProtocolExtraItem`/`TransportExtraItem`（fields.entities.yaml 的 InnerFmt 相关行）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Handler/Fmt/InnerFmt.cs`（`:7` 批量入口、`Resolve` 的 indexId 重发/组改写、`:98 ToUri`、`ResolveSingle` 的 ProtoExtraObj→ProtoExtra 折叠/CustomOutboundObj 落文件、`ToUriSingle` 的反折叠/Subid-IsSub 剥离/RemoveEmptyJson）、`ServiceLib/Handler/ConfigHandler.cs:1977`（`AddBatchServers4InnerUri` 按 ConfigType 分派 `Add*Server`，普通节点不校验 Address/备注）、`ServiceLib/ViewModels/ProfilesViewModel.cs:832`（`Export2InnerUrlAsync`）、`ServiceLib/Models/Entities/ProfileItem.cs`（字段全集与 `IsValid`）、`ProtocolExtraItem.cs`/`TransportExtraItem.cs`、`Global.cs`（Flows/SsSecuritiesInSingbox）、`Common/JsonUtils.cs`（反序列化大小写不敏感、空值策略）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：单行/多行 `v2rayn://` 文本（导入入口已存在：`subs_actions.dart` 的剪贴板/文本导入，提示含 v2rayn://）；导出对象为节点表选中 ID。
- 输出：`importFromText` 返回解析出的 `ProfileDto` + 定位错误；`exportProfiles(ids, 'inner')` 返回换行分隔的内部 URI 文本。
- 错误：`ConfigVersion != 4`、未定义 `ConfigType`/`CoreType`（仅接受 null/Xray/sing_box）/数值 `MultipleLoad` 越界、`Custom` 节点、缺 `CustomOutboundObj` 的 `Outbound` 整行跳过（与上游 null 一致）；无有效文件且无内联内容的 `Outbound` 导出时跳过。
- 取消：纯函数解析/渲染，无取消路径；UI 侧沿用既有导入对话框取消语义。
- 权限：仅本机解析 + SQLite；不启动内核、不写系统代理/TUN、不监听端口（测试端口未使用，无监听）。
- 持久化：SQLite（`ProfileItem`）。订阅归属导入走既有 `replace_sub_profiles`（无校验，与上游批量入库一致）；新增 `Engine::save_imported_profile` 供无 subid 导入（接受空备注/地址，组仍做环检查，Custom/Outbound 只做 normalize）。
- 生效：导入后节点表可见；重开后仍存在；导出文本可再导入（往返稳定）。

允许修改的模块：`crates/subscriptions/src/fmt/**`（新增 `wire.rs`；改 `inner.rs`、`mod.rs`、`lib.rs`）、`crates/subscriptions/tests/parity_original_inner.rs`（去 `#[ignore]` + 新增用例）、`crates/domain/src/profile.rs`（新增 `is_valid`，只加方法）、`crates/application/src/engine.rs`（新增 `save_imported_profile`，只加方法）、`crates/application/tests/fix04_inner_import.rs`（新增）、`crates/bridge_api/src/api/subs.rs`（`render_export` inner 分支加文件 loader + 单测，不新增 FRB 函数）、`apps/desktop/test/ux_parity_fix04_inner_test.dart`（新增）、`fixtures/synthetic/inner-wire/**`（新增合成夹具）、`docs/evidence/UX-PARITY-FIX-04/**`、本卡、`compat/features.yaml`/`actions.yaml`（仅 notes 追加）。

禁止改变的已有行为：编辑器草稿校验（`Profile::validate`/`validate_custom` 原样保留，导入与编辑仍是两套合同）；`main_shell.dart`、`bridge/frb_generated.dart`、`frb_generated.rs` 未动；未跑全仓 `cargo fmt --all` 与 `flutter build windows`；`work/`、`outputs/` 只读；`compat/` 只追加 notes。

测试夹具和原版预期：合成载荷（`node.example.invalid`、`127.0.0.1` 不涉及监听、`11980~11985` 均为数据字段、`11111111-2222-3333-4444-555555555555` 合成 UUID），无真实节点/订阅。原版预期：PascalCase 属性按 `ProfileItem` 还原（含 `ConfigType` 数值、`ProtoExtraObj`/`TransportExtraObj` 对象形态、`CustomOutboundObj` 内联出站、未知键保留）；`Subid`/`IsSub` 导出剥离；组 `SubChildItems=self` 解析为导入方 subid 并重映射 ChildItems；空组删除；`Custom` 跳过。

本次必须通过的命令/真实场景：
- `cargo test -p subscriptions --locked --test parity_original_inner -- --ignored` 先复现 2/2 失败（已存档），修复后去 ignore 全绿。
- `cargo test -p subscriptions --locked` 全绿；`cargo fmt -p subscriptions -- --check`、`cargo clippy -p subscriptions --all-targets --locked -- -D warnings` 通过。
- 联动包：`cargo test -p domain -p application --locked`、`cargo test -p bridge_api --locked`、`cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` 通过；`cargo fmt` 仅格式化本卡文件（dns.rs/routing.rs 两处并行改动的格式差异未动）。
- Flutter：`dart format --output=none --set-exit-if-changed`（新测试文件）、`flutter analyze` 无问题、`flutter test test/ux_parity_fix04_inner_test.dart test/t09_import_export_test.dart` 通过（含真桥导入→落库→导出→再导入→同目录重开）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-04/`（`repro-fail.log`、`postfix-parity.log`、`postfix-subscriptions.log`、`postfix-domain.log`、`postfix-application-import.log`、`postfix-bridge-subs.log`、`postfix-fmt-check.log`、`postfix-clippy.log`、`postfix-flutter-chain.log`）。未做真实窗口截图（integration windows 运行留给根代理统一跑；链路已由真桥 widget 测试覆盖）。

完成条件：冻结 wire DTO 独立存在且 domain 不依赖它；合法原版载荷（含空备注/地址普通节点、内联/文件型 Outbound、组引用、未知键）导入→导出→再导入稳定；两处 `#[ignore]` 已移除；门禁全绿；UI 既有入口验证接线正确（缺失入口则补，但本卡入口齐全，无需改 `features/subs` 以外）。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：剪贴板/文本无 subid 导入的 UI 持久化（`persistImportedProfiles`→`saveProfile`）仍走编辑器校验，空备注/地址节点会保存失败（`E_FIELD_REQUIRED`）。Rust 侧已备好 `Engine::save_imported_profile`，但 UI 可达需新增桥接函数（例如 `import_profiles_persist` 批量导入持久化），涉及 FRB 生成文件，归根代理统一排期。本卡在 application 层验证了该函数（落库+重开），bridge/UI 侧未接线。
- 接口缺口（登记）：文件型 `Outbound` 的导入侧文件落盘（上游 `WriteAllText`→`Address=文件名`）：本卡纯解析层将内联对象存入 `customConfigText`（codegen 可直接消费），但订阅/剪贴板导入后尚未物化出站文件；`build_codegen_input` 的 Address 文件读取缺口（RT-08）仍归属后续卡。
- 接口缺口（登记）：`ACT-PROF-026` 在 `compat/actions.yaml` 仍为 `identified`，实现与证据已齐，状态提升由根代理判定。

本轮实际结果：新增 `crates/subscriptions/src/fmt/wire.rs`（`InnerProfile`/`ProtoExtraWire`/`TransportExtraWire` 三 DTO + `wire_to_profile`/`profile_to_wire_object`/`inline_outbound_text`，未知键 flatten 保留，`RawConfig` 等本地标记不外泄）；`inner.rs` 切到 wire 首选 + legacy snake_case 回退（门控防止复活已拒绝载荷）+ `emit_with` 文件 loader 注入；`to_inner_uri_with_outbound_loader` 新增并被 bridge `render_export` 采用（直接路径→data dir 回退）；`domain::Profile::is_valid` 对标 `IsValid`（Guid/Flow/SS 方法/Reality 公钥规则）；`Engine::save_imported_profile` 新增。测试：subscriptions 117 通过 0 ignored（含 parity 9/9）、domain 47、application 全绿（含新增落库重开 2/2）、bridge_api 41（含新增真解析/真文件导出 2 项）、flutter 新链路 1/1 + t09 4/4、`flutter analyze` 无问题。clippy 四包 `-D warnings` 通过。`compat/features.yaml` F-IMPORT-005 与 `compat/actions.yaml` ACT-MAIN-016/ACT-PROF-026 仅追加 notes。
