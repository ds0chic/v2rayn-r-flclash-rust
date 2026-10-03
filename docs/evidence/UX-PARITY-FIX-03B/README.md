# UX-PARITY-FIX-03B — Custom/Outbound 编辑器文件导入与外部编辑

任务卡：`docs/tasks/FIX-03B.md`（FIX-03 后续卡，上游 `AddServer2ViewModel.BrowseServer` /
`EditServer` + `ConfigHandler.AddCustomServer` / `AddCustomOutboundServer`）。

状态：`implemented`（Dart/UI 侧 browse→import→Address 改写→备注默认→保存 与 edit
full-path 解析→外部打开已由 widget/单元测试覆盖；文件型配置的 config codegen 读取由根代理
Rust 改动 + `crates/application/tests/fix03b_custom_file.rs` 覆盖；未跑真实 Windows 窗口
双窗口对照，故不写 `verified`）。

## 本轮范围

1. 编辑器「浏览」：`file_selector` 选 json/yaml/yml/txt → `bridge.customImportFile(sourcePath)`
   → 草稿 `address = 返回的存储文件名`；备注为空时默认
   `import custom@yyyy/MM/dd HH:mm:ss`（Outbound 为 `import custom outbound@...`）。
   取消不修改草稿；导入失败保留 Address 并显示错误。
2. 编辑器「编辑」：`resolveCustomConfigPath` 解析 fullPath（绝对路径直接用，否则
   `<dataDir>/config/<address>`），`Process.run('cmd', ['/c','start','',fullPath])` 外部打开；
   文件不存在时明确提示。
3. 桥接 seam：`apps/desktop/lib/bridge/bridge_port.dart` 新增 `customImportFile(String)` 与
   `dataDir()`，实现于 `FrbBridgePort`（转发 FRB 生成的 `engine.dart`）与
   `SyntheticBridgePort`（记录调用、返回合成文件名，可注入失败）。
4. `apps/desktop/lib/features/profiles/profile_actions.dart` 在三个 Custom/Outbound 编辑器
   调用点（`editSelectedProfile` 分派、`startAddCustomProfile`、`editSelectedCustom`）把 bridge
   的 `customImportFile` / `dataDir` 注入对话框。

## 上游对照

- `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/AddServer2ViewModel.cs:24`（BrowseServerCmd → BrowseConfigFileInteraction）
- 同上 `:79`（BrowseServer：写 `Address=fileName`、空备注默认、调用 AddCustomServer/AddCustomOutboundServer）
- 同上 `:109`（EditServer：`Utils.GetConfigPath(address)` + `File.Exists` + `ProcUtils.ProcessStart`，缺失提示 `FailedReadConfiguration`）
- `.../ServiceLib/Handler/ConfigHandler.cs:550/587`（AddCustomServer/AddCustomOutboundServer：拷贝到 config 目录、`{guid}{ext}` 命名、`import custom[@]@yyyy/MM/dd HH:mm:ss` 备注）
- `.../ServiceLib/Common/Utils.cs:1191`（GetConfigPath：`Path.Combine(StartupPath, "guiConfigs")`，绝对路径原样）

Rust `AppEngine::import_custom_file` 将文件拷贝到 `<data_dir>/config/<new_index_id>.<ext>` 并
返回文件名（`crates/application/src/engine.rs:604`）；`data_dir()` 返回当前数据目录
（`crates/bridge_api/src/api/engine.rs:112`）。本卡 Dart 侧复用同一命名/目录语义，未改 crates。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib/... profile_actions.dart bridge_port.dart test/fix03b_custom_browse_test.dart` | 0 changed |
| `flutter analyze` | No issues found（最终一轮全绿；中途一轮曾报 `test/fix14*`、`test/t16_backup_test.dart` 3 处，属另一并行 backup 子代理在飞改动，随后已消失） |
| `flutter test test/fix03b_custom_browse_test.dart` | 14/14 通过（首次两次 `did-not-complete` 环境抖动，重试即过；日志见 `fix03b-run.log`） |
| `flutter test test/t10_custom_editor_test.dart` | 5/5 通过（回归） |
| `flutter test test/fix03_special_editors_test.dart` | 7/7 通过（回归） |

未运行：真实窗口 integration test、`flutter build windows`、Rust 门禁（crates 由根代理负责）。

## 覆盖的验收点

- Browse 成功：`customImportFile` 收到 source、Address 变为返回文件名、备注前缀正确、保存落库。
- Browse 取消：不调用导入、Address 不变。
- Browse 失败：保留 Address、显示 `custom-file-error`。
- 备注默认仅当为空；非空不被覆盖。
- Edit 相对名→`<dataDir>\config\<name>`；绝对路径原样；缺失显示显式错误；空 Address 拒绝。
- Synthetic seam：记录 source、带扩展名返回、可注入结构化失败。

## 未完成 / 接口缺口

- 未做真实 Windows 窗口 + 真实 `file_selector` 的端到端窗口验证（本卡只跑到 widget 层）；
  建议并入后续真实窗口回归。
- `Edit` 的非 Windows 分支未实现（返回 false 提示），当前仅 Windows 有真实打开路径；与上游
  桌面端一致，如实标注。
- 生成配置真正使用该文件内容由根代理 Rust 侧负责，本卡未新增 Rust 测试。
