# R3-PROF-06 — Custom 原始配置导出保持字节不变

状态：`implemented`（Rust 导出分支逐字节断言 + Dart 选择器/剪贴板分派 widget 断言；真实 OS 保存窗口字节未复验）。

任务 ID：R3-PROF-06

本次唯一用户流程：对 Custom 节点执行「导出完整配置」→另存为文件时，文件内容与原始配置逐字节一致（上游 `File.Copy`），文件选择器按原始扩展名（如 `.yaml`）；复制到剪贴板对 Custom 明确失败，不伪造文本。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` RE-PROF-08 / R3-PROF-06。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Handler/CoreConfigHandler.cs:16-23,44-83`（Custom 复制原文件；`fileName=null` → `CheckServerSettings`）；`ServiceLib/ViewModels/ProfilesViewModel.cs:743-797`（剪贴板传 `null`，文件传真实路径）。

对应 feature / field / action：`RE-PROF-08`、`F-PROFILE-*`（导出完整配置）、`ACT-PROF-*`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：节点 `ConfigType`、`Address`（Custom 的原始文件名）、stored raw 文本。
- 输出：`ExportClientConfigDto.text` 为原始文本（无 JSON 引号/转义）；`file_name` 保留原扩展名。
- 错误：raw 缺失 → `error.custom_config_file_missing`（结构化失败）；文件写失败保持 `error.file_write_failed`。
- 取消：保存对话框取消不写文件。
- 权限：`None`；只读本机配置与磁盘写入目标路径。

允许修改的模块：`crates/bridge_api/src/api/speedtest.rs`、`apps/desktop/lib/features/profiles/{profile_actions.dart,profile_fields.dart}`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。未改 `frb_generated`。

禁止改变的已有行为：非 Custom 节点的生成配置导出仍为 pretty JSON；`write_export_file` 语义；剪贴板普通节点导出。

测试夹具与原版预期：Rust `raw_custom_export` 以合成 YAML 断言 `text == raw` 且 `file_name` 保留扩展名、缺失时结构化失败；Dart `SyntheticBridgePort` 合成 Custom 行断言建议名非 `config.json` 且带 `address` 扩展名、剪贴板不写。原版预期：Custom 文件导出为原文件副本。

本次必须通过的命令/真实场景：
- `cargo test -p bridge_api --lib api::speedtest::tests --locked`
- `flutter analyze`
- `flutter test test/r3_prof_06_export_custom_test.dart test/re_prof_08_export_udp_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-MISC/README.md`。

完成条件：Custom 原始文本导出字节不变、扩展名正确、剪贴板明确失败；普通导出无回归；门禁通过。

接口缺口（登记）：`ShareExportResult`（bridge_port 映射）未暴露 `fileName`，Dart 侧从 `ProfileSummary.address` 推导扩展名；建议后续在共享导出结果中带出 `file_name`，避免重复推导。

本轮实际结果：`speedtest.rs` 新增 `raw_custom_export`/`custom_export_file_name`，Custom 分支绕过 JSON generator；`profile_actions.dart` 按 `address` 原始扩展名建议保存名、Custom 剪贴板显式失败并新增 `_profileInfoFor`/`_exportFileName`。Rust 1 新测试、Dart 2 新用例与回归通过。
