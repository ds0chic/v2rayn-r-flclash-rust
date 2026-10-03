# FIX-04B — 完整配置导入（v2ray/sing-box JSON、Clash YAML）落盘与生成使用

状态：`implemented`（识别→文件型落盘→重开→生成实际消费内容已由 subscriptions/application/bridge 单测与真桥 Flutter 链路验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-04B

本次唯一用户流程：在既有剪贴板/文本导入入口粘贴一份完整配置（v2ray/Xray JSON、sing-box JSON、Clash YAML）→ 解析为 `Custom` 节点（上游 `AddBatchServers4Custom` 路径）→ 原文按文件型持久化（`Address` 指向 `<data>/config/` 文件）→ 重开仍存在 → 生成内核配置时实际读取并使用该原文（含未知键）。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；FIX-04（`docs/tasks/FIX-04.md`）已建立冻结 wire DTO 与文件 loader；RT-08/FIX-03B 已提供 `AppEngine::custom_file_text` 与 `import_custom_file`。登记来源：`repair-queue.md` 第 22 行“完整配置导入另卡”。

对应 feature / field / action / layout ID：`F-IMPORT-005`、`ACT-MAIN-016`（从剪贴板批量导入）、`F-IMPORT-002`（文本导入）、`ProfileItem.Address`/`CoreType`/`ConfigType=Custom|Outbound`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Handler/ConfigHandler.cs:1715`（`AddBatchServers4Custom` → `AddBatchServersDefaultCustom` 优先 `V2rayFmt.ResolveToCustom` / `SingboxFmt.ResolveToCustom`，再 custom outbound，最后 `ClashFmt.ResolveFull` / `Hysteria2Fmt.ResolveFull2`）、`:1842`（`SaveCustomRawFileServer` 写文件 + `Address=fileName` 与 `DetectFileExtension`）、`:2037`（`AddBatchServers` 总入口）、`ServiceLib/Handler/Fmt/V2rayFmt.cs:56/89/111`（`ResolveFull`/`ResolveFullToOutbound`/`ResolveOutbound` 均 `WriteAllText` 后 `Address=文件名`）、`ServiceLib/Handler/Fmt/SingboxFmt.cs:56/88/110`（同上）、`ServiceLib/ViewModels/ProfilesViewModel.cs` 导入命令。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：剪贴板/文本对话框中的完整配置文本（JSON 对象/数组、Clash YAML、Hysteria2 完整配置）。
- 输出：`importFromText` 返回解析出的 `ProfileDto`（`ConfigType=Custom`，`CoreType` 为 Xray/SingBox/Mihomo/Hysteria2）；同时把原文写入 `<data>/config/<indexId><ext>` 并把 `Address` 设为该文件名（无 data dir 的测试引擎写系统临时目录并存绝对路径）。
- 错误：非 JSON/YAML 的自由文本仍走既有 `describeImportFailure`（不新建节点）；未知顶层键随原文逐字保留（`RawConfig` 只在解析层短暂存在，导入后写入文件，不再作为内联冒充）。
- 取消：纯导入流程无取消；粘贴对话框取消沿用既有语义（不解析、不落库）。
- 权限：仅本机解析 + 写 `<data>/config/` 文件 + SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：文件型 `Address`（相对文件名）随 `ProfileItem` 落库，重开后 `custom_file_text` 按 `<data>/config/` 解析；内联型（`customConfigText`，FIX-04 inner `CustomOutboundObj`）保持 `Address` 为空，两者互不冒充。
- 生效：`build_codegen_input` 逐节点以 `custom_config_text` → `custom_file_text` 组装 `custom_outbound_content`；`codegen.rs::build_input` 让 active 的 `Custom`/`Outbound` 也从该 map 取内容，故完整配置节点被选中时生成结果实际包含其原文（含未知键）。

允许修改的模块：`crates/subscriptions/**`、`crates/application/src/codegen.rs`、`crates/bridge_api/src/api/subs.rs`（不改 frb_generated）、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-04B/**`、本卡、`compat/features.yaml`/`actions.yaml`（仅 notes 追加）。

禁止改变的已有行为：`main_shell.dart`/`app.dart`/两处 `frb_generated`/`lib/bridge/api/**`/`features/profiles/**`（`template_window.dart` 未动）/`features/runtime|monitor|update|backup|routing/**`/`crates/application/src/engine.rs`/`crates/updater/**`；FIX-04 已提交的 `v2rayn://` inner URI 语义（`inner.rs`/`wire.rs` 未动，parity 9/9 复跑通过）；未跑全仓 `cargo fmt --all` 与 `flutter build windows`。

冻结源码路径结论：`AddServerViaClipboard`/`AddBatchServers` 对完整 JSON/YAML 走 `AddBatchServers4Custom`，产出 `ConfigType=Custom` 且 `Address=文件名` 的节点，而非 `FullConfigTemplate`（模板窗口是另一条独立路径，`codegen::template_for` 已存在）。因此本卡按文件型 Custom 落盘实现，未改 `template_window.dart`。

测试夹具和原版预期：合成载荷（`node.example.invalid`、合成 UUID `11111111-2222-3333-4444-555555555555`、端口 11888/11980~11982 均为数据字段或测试入站、未知键 `x-future`）；无真实节点/订阅。原版预期：完整 JSON/YAML → Custom 节点 + 文件原文；未知键保留；文件型与内联型区分；active 生成实际含原文。

本次必须通过的命令/真实场景与结果：
- `cargo test -p subscriptions --locked`：全绿（含 parity_original_inner 9/9、parse_pipeline 13/13、新增 batch 单测）。
- `cargo test -p application --locked --lib`：183 通过（含新增 `codegen::tests::active_file_type_custom_consumes_outbound_contents`）。
- `cargo test -p bridge_api --locked api::subs::tests`：12 通过（新增 full v2ray、full singbox、clash yaml、file/inline 区分 4 项；其中 full v2ray 项端到端验证落库+读回+生成含原文与未知键）。
- `cargo clippy -p subscriptions -p application -p bridge_api --all-targets --locked -- -D warnings`：通过。
- `cargo fmt -p subscriptions -- --check`、`rustfmt --edition 2021 --check crates/application/src/codegen.rs crates/bridge_api/src/api/subs.rs`：0 差异。
- Flutter：`dart format --output=none --set-exit-if-changed test/ux_parity_fix04b_config_import_test.dart` 0 改变；`flutter test test/ux_parity_fix04b_config_import_test.dart` 2/2（真桥完整 Xray 配置导入→文件落盘→持久化→重开；粘贴对话框取消）；`flutter test test/ux_parity_fix04_inner_test.dart test/t21e_import_ux_test.dart` 6/6 无回归；`flutter analyze` No issues。

证据文件位置：`docs/evidence/UX-PARITY-FIX-04B/`（`postfix-subscriptions.log`、`postfix-application.log`、`postfix-bridge-subs.log`、`postfix-clippy.log`、`postfix-fmt-check.log`、`postfix-subscriptions-fmt.log`、`postfix-flutter-config-import.log`、`postfix-flutter-regression.log`、`postfix-flutter-analyze.log`、`README.md`）。

完成条件：完整 v2ray/sing-box JSON 与 Clash YAML 经既有导入入口识别为 Custom 节点；原文文件型落盘（`Address` 指向配置目录）且重开可读；生成实际消费原文（含未知键）；文件型与内联型互不冒充；FIX-04 inner 语义不变；门禁通过。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：订阅刷新路径 `application::subs::refresh_subscriptions*` 直接落库，未经过 bridge `import_from_text`，因此“完整配置”类订阅内容的文件物化仍未覆盖；该路径在 `crates/application`（本卡禁改），需后续卡在 application 层补物化。
- 接口缺口（登记）：`ACT-MAIN-016` 在 `compat/actions.yaml` 仍为 `identified`，实现与证据已齐，状态提升由根代理判定。

本轮实际结果：`crates/subscriptions/src/fmt/batch.rs` 新增 `take_raw_config`、`detect_config_extension`（对标 `SaveCustomRawFileServer.DetectFileExtension`）并补单测；`fmt/mod.rs`、`lib.rs` 重导出。`crates/bridge_api/src/api/subs.rs` 新增 `materialize_custom_configs`（完整配置→`<data>/config/<indexId><ext>` 文件型，`Address` 存文件名；测试引擎无 data dir 时退化为系统临时目录绝对路径），在 `import_from_text` 落库前调用，并新增 4 项单测。`crates/application/src/codegen.rs` 的 `build_input` 让 active 的 `Custom`/`Outbound` 优先从 `outbound_contents`（已由引擎按 `custom_file_text` 解析）取内容，并新增 1 项单测。`apps/desktop/test/ux_parity_fix04b_config_import_test.dart` 新增真桥成功/失败链路与取消 widget 测试。`compat/features.yaml` F-IMPORT-005 与 `compat/actions.yaml` ACT-MAIN-016 仅追加 notes。
