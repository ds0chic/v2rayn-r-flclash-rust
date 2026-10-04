# RE-PROF-08 — 完整客户端配置两导出与 UDP 测速

状态：`implemented`（Rust 侧导出生成与 UDP 回环/失败分支已单测通过；Dart UI 分派、剪贴板/文件保存/取消/写失败与 UDP 入口状态已 widget 测试通过。生产 `FrbBridgePort.exportClientConfigText` 因新增 FRB 函数未重生成，暂返回结构化“待重生成”错误，故不写 `verified`；真实节点路由 UDP 与真实内核未实测）。

任务 ID：RE-PROF-08

本次唯一用户流程：在节点表右键「导出 → 导出所选完整配置 / 导出所选完整配置至剪贴板」得到所选（或活动）节点的完整客户端配置文本（分别经保存对话框落盘、或复制到剪贴板）；右键「测试 UDP 延迟 (多选)」在支持时启动 UDP 测速，不支持时给出明确提示且入口保持可见。对上游 `ProfilesViewModel.Export2ClientConfigAsync/Export2ClientConfigResult`（`ProfilesViewModel.cs:743-797`）与 `SpeedtestHandler` 的 UDP 动作。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-08（`identified`）；旧行 PR-13/PR-28。开始 HEAD `842520b`（本卡只读 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**` 及禁止清单内文件；工作树中 `engine.rs`/`runtime`/`net_host`/`profiles_models.dart` 等并发子代理改动未触碰、未回退）。

对应 feature / field / action / layout ID：`ACT-PROF-022`、`ACT-PROF-023`（完整客户端配置两导出）、`ACT-PROF-018`（UDP 测试）、`F-PROFILE-*` 节点操作族、`CFG-*` 生成。

必读上游文件、符号和固定 commit：
- `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/ProfilesViewModel.cs:743-797`：`Export2ClientConfigAsync(bool blClipboard)`（选中项 → `CoreConfigContextBuilder.Build` → 剪贴板 `GenerateClientConfig(context,null)` / 文件 `SaveFileDialogInteraction`）与 `Export2ClientConfigResult(fileName,item)`（`GenerateClientConfig(context,fileName)`）。
- 上游 `SpeedtestHandler` UDP 分支（`ESpeedActionType.UdpTest`，按 `UdpTestTarget`/协议目标）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：单个选中节点优先，其次活动节点（`selected.length==1` / `activeId`）；节点 id 为稳定 `IndexId`。
- 输出：完整客户端配置文本（`AppEngine::build_codegen_input` + `generate` 的既有路径，pretty JSON）。文件入口经 `file_selector` 保存对话框写盘；剪贴板入口 `Clipboard.setData`。
- 错误：未知节点/生成失败 → 结构化失败提示；写盘失败 → 提示，绝不伪成功；UDP 端点不可用/超时 → `delay==-1` 的失败结果。
- 取消：保存对话框取消 → 不写任何文件、不提示、不改剪贴板。
- 权限：仅本机 UI + FRB/Rust；不启动内核、不写系统代理/TUN、不监听用户端口；测试端口 ≥ 11808。
- 持久化：导出为只读文本，不落库；UDP 结果走既有 `ProfileExItem` 语义（FIX-10/10B 集合/排序不变）。
- 生效：两导出即时可见/可粘贴；UDP 在支持矩阵为真时可用，为假时测速入口禁用并提示，入口不删。

允许修改并实际修改的模块：
- `crates/application/src/speedtest.rs`（新增 `udp_target_endpoint`/`udp_ping` + `#[cfg(test)] re_prof_08_udp_tests` 回环成功/超时失败/runner 分支）。
- `crates/application/src/codegen.rs`（新增导出文本生成断言单测，未改生成逻辑）。
- `crates/bridge_api/src/api/speedtest.rs`（`NetHostTestSession::udp_ping` 委托直接探测、`speedtest_supported().udp=true`、新增 `ExportClientConfigDto` + `export_client_config`（`#[frb(sync)]`，未改 `frb_generated`）、修正 UDP 支持断言、新增导出失败分支测试）。
- `apps/desktop/lib/bridge/bridge_port.dart`（新增 `exportClientConfigText` 抽象方法；`FrbBridgePort` 返回结构化“待 FRB 重生成”失败；`SyntheticBridgePort` 记录调用、可注入写盘成功与生成失败；`speedTestSupport().udp` 可切换）。
- `apps/desktop/lib/features/profiles/profile_actions.dart`（`clientConfigSavePickerProvider` 可注入保存选择器 + `exportSelectedClientConfig`）。
- `apps/desktop/lib/features/profiles/profiles_table.dart`（两导出入口分派到真实动作；`_withRuntimeCapabilities` 按 `SpeedTestSupport.udp` 动态启用 UDP 条目并保留入口）。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`（`startSpeedTest` 增加 UDP 支持门，不支持时明确提示不启动）。
- `apps/desktop/test/re_prof_08_export_udp_test.dart`（新增）。
- 本卡、`docs/evidence/recheck-fixes/RE-PROF-08/**`。

禁止改变的已有行为（本卡未动）：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`main_shell`/features 其它域、`crates/application/src/engine.rs`/`backup_service.rs`/`subs.rs`/`dns.rs`/`routing.rs`/`monitor.rs`、`crates/updater/**`、`crates/subscriptions/**`。FIX-10/10B 的测速集合（Mixed/Fast 取可见、其余取选中）与 `ProfileExItem.Sort` 排序/结果语义未改；不删入口、不降分母、不伪造导出/测速结果。

测试夹具和原版预期：合成节点（RFC5737/示例地址）；UDP 用 `127.0.0.1` 上 ≥11808 的回环 UDP 回声夹具与静默 socket。原版预期：导出文本为节点完整生成配置；取消不落盘；剪贴板为完整文本；UDP 按目标协议测往返延迟。

本次实际运行且通过的命令：
- `cargo.exe fmt -p application -p bridge_api`（仅格式化两 crate）
- `cargo.exe test -p application --lib --locked re_prof_08` → 4/4 通过（关键字/`host:port` 解析、回环往返、超时失败、runner UDP 分支）。
- `cargo.exe test -p application --lib --locked client_config_export_text` → 1/1 通过。
- `cargo.exe test -p bridge_api --lib --locked speedtest` → 6/6 通过（含 `udp_is_supported_by_the_direct_probe`、`export_client_config_reports_missing_profile`）。
- `cargo.exe clippy -p application -p bridge_api --all-targets --locked -- -D warnings` → 通过。
- `flutter.bat analyze` → No issues found。
- `dart.bat format`（5 个改动 Dart 文件）→ 仅测试文件格式。
- `flutter.bat test test/re_prof_08_export_udp_test.dart` → 5/5 通过（剪贴板、取消、写成功、写失败、UDP 门控）。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-08/`（`README.md`、`observations.json`）。

完成条件与剩余：Rust 生成文本/UDP 回环与失败分支、Dart 导出取消/写失败/剪贴板与 UDP 入口状态均已测试通过；两导出入口不再是 `notImplemented`/永久禁用；UDP 条目按支持矩阵启用。**未完成**：生产 `FrbBridgePort` 未接到 `export_client_config`（见接口缺口），真实节点路由 UDP 与真实内核未实测，未复跑 FIX-10/10B 测速集合/排序回归。

接口缺口（登记，FRB 重生成需求）：新增 FRB 函数 `export_client_config(index_id) -> ExportClientConfigDto` 与 `#[frb(sync)]` 已在 `crates/bridge_api/src/api/speedtest.rs` 实现，但按硬约束未改 `frb_generated.*` 与 `lib/bridge/api/**`。**根代理前置**：运行 FRB 2.13.0 codegen 重生成两处 `frb_generated` 后，把 `FrbBridgePort.exportClientConfigText` 改为调用 `speedtest.exportClientConfig(indexId: id)`，将 `text` 映射到 `ShareExportResult.text`、`error` 映射到 `ShareExportResult.error`；在此之前 Dart 不引用任何未生成符号，生产路径返回 `E_FRB_REGENERATION_REQUIRED`。

本轮实际结果：`application::speedtest` 新增协议目标解析与直接 UDP 往返探测（ntp/dns/stun/mcbe 关键字与 `host:port`），`bridge_api` 将其接入 `NetHostTestSession::udp_ping` 并把 `SpeedTestSupportDto.udp` 置真；新增 `export_client_config`（复用 `AppEngine::build_codegen_input` + `generate`）。Dart 侧两导出与 UDP 门控接线完成，`flutter analyze` 与定向 Rust/Dart 测试全绿。并发子代理在 `engine.rs`/`runtime`/`net_host` 等文件的进行中改动本卡未触碰、未回退。
