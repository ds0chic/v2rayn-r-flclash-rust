# FIX-03B — Custom/Outbound 编辑器文件浏览导入与外部编辑

状态：`implemented`（Browse/Edit 的 Dart/UI 全链与桥接 seam 已由 widget/单元测试覆盖；文件型配置的 config codegen 读取已由根代理 Rust 改动 + `crates/application/tests/fix03b_custom_file.rs` 覆盖；未做真实 Windows 窗口双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-03B

本次唯一用户流程：Custom/Outbound 编辑器「浏览」按钮：选文件 → 拷贝到配置目录 → Address 改写 → 保存 → 重开 → 生成配置使用该文件内容。验收合同（见 TODO 卡）：
1. 编辑器 Browse：`file_selector` 选 json/yaml（取消不修改草稿）；调用新桥接 `bridge.customImportFile(sourcePath)`（生成的 Dart 函数 `customImportFile` 已存在，见 `apps/desktop/lib/bridge/api/engine.dart`）；成功后草稿 address=返回的存储文件名；备注为空时默认 `import custom@yyyy/MM/dd HH:mm:ss`（Outbound 用 `import custom outbound@...`），对齐上游 `AddServer2ViewModel.BrowseServer` + `ConfigHandler.AddCustomServer/AddCustomOutboundServer`。
2. Edit 按钮：外部打开该配置文件（Windows：`Process.run('cmd', ['/c', 'start', '', fullPath])` 或等价）；fullPath：address 为绝对路径则直接用；否则 `bridge.dataDir()` + `config` + address。文件不存在时明确提示。
3. 桥接 seam：`bridge_port.dart` 新增 `customImportFile(String)` 与 `dataDir()` 抽象及 Frb/Synthetic 实现（Synthetic 记录调用、返回合成文件名）。
4. 保存→重开：草稿落库后 address 保留；生成配置（`engine.build_codegen_input` 已支持文件型读取）由根代理 Rust 测试覆盖，本卡补 Dart 侧 UI 测试。
5. 不改 FIX-03 已提交的分派/策略组/五模式语义。

前置任务及已验证证据：FIX-03（`docs/tasks/FIX-03.md`、`docs/evidence/UX-PARITY-FIX-03/`）；`repair-queue.md` 第 21 行；上游冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。FIX-03 已分派 Custom/Outbound→custom 编辑器，接口缺口登记“需要 file-picker 依赖 + bridge 文件导入 API，建议后续卡 FIX-03b”。

对应 feature / field / action / layout ID：`CFG-013`（Custom/Outbound）、`F-PROFILE-002`（编辑节点）、`ACT-PROF-001`（编辑选中节点）；上游动作 `AddServer2ViewModel.BrowseServerCmd / EditServerCmd / SaveServerCmd`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/ServiceLib/ViewModels/AddServer2ViewModel.cs:24/79/109`（BrowseServerCmd→BrowseConfigFileInteraction；BrowseServer 写 Address+空备注默认+AddCustomServer/OutboundServer；EditServer `Utils.GetConfigPath`+`File.Exists`+`ProcUtils.ProcessStart`，缺失 `FailedReadConfiguration`）、`ServiceLib/Handler/ConfigHandler.cs:550/587/631`（AddCustomServer/AddCustomOutboundServer 拷贝到 config 目录、`{guid}{ext}` 命名、`import custom[@]@yyyy/MM/dd HH:mm:ss`；EditCustomServer 仅更新字段）、`ServiceLib/Common/Utils.cs:1191`（GetConfigPath）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：编辑器内 Address 文本 / file_selector 返回的 source path。
- 输出：`customImportFile` 返回 `CustomFileResult{ok,fileName,error}`，fileName 写入草稿 `address`；保存走既有 `saveProfile`/engine 路径。
- 错误：导入失败保留 Address 并显示错误；Edit 时 Address 为空或文件不存在/无法打开显示明确提示。
- 取消：文件选择取消直接返回，草稿零修改；取消对话框不落库（沿用 FIX-03）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN/注册表/路由、不监听端口、不按名杀进程。
- 持久化：SQLite（`ProfileItem`）；`address` 保存后保留，重开一致。
- 生效：生成配置读取该文件内容（Rust `build_codegen_input` 文件型分支，根代理覆盖）。

允许修改的模块：`apps/desktop/lib/features/profiles/**`（改 `custom_editor_dialog.dart`、`profile_actions.dart`）、`apps/desktop/lib/bridge/bridge_port.dart`、`apps/desktop/test/**`（新增 `fix03b_custom_browse_test.dart`）、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-03B/**`、本卡、compat 台账（仅追加）。

禁止改变的已有行为：`main_shell`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`（生成文件）、features/settings|subs|runtime|monitor|update|backup|routing 目录、`crates/**`（根代理已完成，如发现问题只登记）；不改 FIX-03 的分派/策略组/五模式语义；不删入口或降分母；不伪造导入/落库结果。

测试夹具和原版预期：合成 source path（`C:/tmp/src.json`、`C:/tmp/src.yaml`）与合成存储名（`stored.json`）；`dataDir='C:\\data'`。原版预期：绝对 Address 原样打开，相对名落到 `<dataDir>/config/<name>`；空备注默认 `import custom@...`/`import custom outbound@...`；取消零修改。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib/features/profiles/custom_editor_dialog.dart lib/features/profiles/profile_actions.dart lib/bridge/bridge_port.dart test/fix03b_custom_browse_test.dart`
- `flutter analyze`
- `flutter test test/fix03b_custom_browse_test.dart`（偶发 `did-not-complete`/exit 79 环境抖动，重试即过）
- 相邻回归：`flutter test test/t10_custom_editor_test.dart test/fix03_special_editors_test.dart`
- 不跑 `flutter build windows --release`；无 Rust 改动，不跑 Rust 门禁。

证据文件位置：`docs/evidence/UX-PARITY-FIX-03B/`（`README.md`、`observations.json`、`fix03b-run.log`）。

完成条件：Browse 成功/取消/失败、备注默认、Edit 路径解析、Synthetic seam 全部由 widget/单元断言覆盖；草稿保存后 Address 保留；format/analyze/测试门禁通过。真实窗口与 file_selector 端到端、非 Windows 外部打开、文件型 config codegen（Rust）拆至后续/根代理。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：本卡未做真实 Windows 窗口 + 真实 `file_selector` 的端到端窗口验证；建议并入后续真实窗口回归。
- 接口缺口（登记）：`Edit` 的非 Windows 分支未实现（返回 false 提示），当前仅 Windows 有真实打开路径；与上游桌面端一致，如实标注。

本轮实际结果：`bridge_port.dart` 新增 `customImportFile(String)`/`dataDir()` 抽象，`FrbBridgePort` 转发 FRB、`SyntheticBridgePort` 记录 source 并可注入失败；`custom_editor_dialog.dart` 新增 `pickCustomConfigFile`/`openConfigFileExternally`/`resolveCustomConfigPath`/`defaultCustomRemarks` 及 Browse/Edit 按钮与错误提示，Address/备注改用 controller 以支持程序化改写；`profile_actions.dart` 在三个 Custom/Outbound 编辑器调用点（`editSelectedProfile` 分派、`startAddCustomProfile`、`editSelectedCustom`）注入 bridge seam。`fix03b_custom_browse_test.dart` 14/14 通过；回归 `t10_custom_editor_test.dart` 5/5、`fix03_special_editors_test.dart` 7/7。门禁：format 0 changed；`flutter analyze` 全绿（No issues）。未改 crates、未改生成文件。compat 仅追加（actions.yaml 该动作条目补 implementation_location/test_ids/notes，status identified→implemented；features.yaml 因另一并行子代理正在改写而本轮未动）。
