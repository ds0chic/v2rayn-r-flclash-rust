# R4-16 导入导出单次提交 — 证据

状态：implemented（部分场景未运行 / 未实测，见“缺口”）。

## 固定基线 / 环境

- 应用 HEAD（本轮起点，未 commit）：`841accd`
- 应用审查基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- 上游固定：`7d6a967c18c697f28dc6917122ed3a4993fcf336`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）
- `armed=false`；无 AUTO_SMOKE、无预置 active、无开发 Xray 环境变量。
- Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1 stable-msvc；Windows 11 25H2 26220。
- 端口策略：本轮无任何监听 / 网络连接，未占用任何端口，未触碰 `127.0.0.1:10808`。
- 测试数据：仅合成分享链接 / 合成 DTO / 合成 JSON，未读取或写入任何真实订阅 URL 或用户凭据。

## 上游语义对照（7d6a967）

- `ServiceLib/Handler/ConfigHandler.cs:2037 AddBatchServers(..., isSub)`：仅在 `isSub && subid` 时先 `RemoveServersViaSubid(..., true)` 再重映射 active。
- `ConfigHandler.cs:1627 AddBatchServersCommon`：`if (isSub) arrData = arrData.Distinct();` —— **手工批导入（isSub=false）不去重**；每行 `profileItem.Subid = subid; profileItem.IsSub = isSub;`；末尾一次 `InsertAllAsync`。
- `ConfigHandler.cs:484-502`（MainWindowViewModel 剪贴板）走 `AddBatchServers(..., _config.SubIndexId, false)`。
- 结论：手工粘贴/扫码 = 绑定命令开始时的当前组 + `IsSub=false` + 无去重 + 单次批量插入；无组时插入不分组。“parse/preview 不写库、commit 一次事务”为本次落点。

## 改动

- `apps/desktop/lib/features/subs/import_persistence.dart`：新增 `ImportPreview` / `previewImport`（subid=null，不落库）/ `commitImport`（有组走 Rust 单次批量；无组走 FIX-04 逐行兜底）；保留 `persistImportedProfiles`。
- `apps/desktop/lib/features/subs/subs_actions.dart`：剪贴板 / 粘贴 / 扫码共用 `_importPipeline` = 先 `previewImport` 再 `commitImport` 单次提交；手工导入 `deduplicate:false`（原版来源规则）。
- `crates/bridge_api/src/api/subs.rs`：抽出 `parse_import` 共享解码；新增 `preview_import_text`（parse-only，不落库、不物化文件，FRB 绑定待整合者再生成）；`import_from_text` 成为单次 commit；`materialize_custom_configs` 改为内容哈希文件名（重复解析不产生孤儿文件）；新增 3 个单测。
- `apps/desktop/test/repair/r4_16_repro_test.dart`（新）：复现“组内粘贴 = 1 次批提交 + N 次逐行保存”。
- `apps/desktop/test/r4_16_contract_test.dart`（新）：parse/preview 与 commit 分离、无重复写、手工不去重、坏行/失败/取消、字段回环与导出互导。
- `apps/desktop/test/recheck01_import_snapshot_widget_test.dart`：适配“预览(subid=null) → 单次组提交(subid=快照)”调用序列，快照语义不变。

## 门禁命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_16_repro_test.dart`（修复前） | FAIL：`saveImportedCalls` 期望 0 实际 2 |
| `flutter test test/repair/r4_16_repro_test.dart test/r4_16_contract_test.dart test/recheck01_import_snapshot_widget_test.dart` | PASS 11/11 |
| `flutter test`（导入相关既有回归：t21e_import_ux / t09_import_export / t21e_import_forms / ux_parity_fix04b / ux_parity_fix04_inner / recheck01_group_inheritance） | PASS 21/21 |
| `cargo fmt --all -- --check` | PASS（exit 0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS（exit 0） |
| `cargo test --workspace --locked` | PASS（exit 0） |
| `cargo test -p bridge_api -- import` | PASS 12/12（含 3 个新增） |
| `flutter analyze lib/features/subs test/r4_16_contract_test.dart test/repair/r4_16_repro_test.dart test/recheck01_import_snapshot_widget_test.dart` | PASS：No issues found |
| `flutter analyze`（整仓） | **BLOCKED（非本卡）**：`test/r4_13_s05_contract_test.dart` 引用尚未实现的 `SettingsController.saveAndApply`（R4-13.S05；`features/settings/**` 属本卡禁改范围） |
| `flutter build windows --release` | PASS：`build\windows\x64\runner\Release\v2rayn_desktop.exe` |

构建前检查：无本项目实例锁 `target/release` exe（仅 `dist/...` 下的 `net_host` 在运行，非构建目标，未终止）。

## 缺口（未运行 / 未实测）

1. **17 格式逐条矩阵未运行**：VMess/VLESS/Shadowsocks/Socks/Trojan/Hysteria2/TUIC/WireGuard/Anytls/Naive/Inner/V2ray/Sing-box/Clash/HtmlPage/Base/FmtHandler 未逐条建立“导入→重开→导出→再导入”证据。本轮仅覆盖通用/内部/结构化自定义的既有 Rust 单测与合成 Dart 管线。
2. **真实 FRB 重开互通未实测**：`t21e_import_real_bridge_test` 需 `bridge_api.dll` 且走旧 `persistImportedProfiles` 接缝，本轮未运行；字段级“导入→重开→导出→再导入”仅在 DTO / 合成层验证。
3. **无组批量单事务缺口**：桥未暴露单事务批写入口；`commit_import_text`（对已解析 profiles 单次落库）缺失，无组导入仍走逐行 `saveImportedProfile`。需整合者再生成 FRB 后接线。
4. **组提交二次解析**：有组 commit 复用 `import_from_text`（唯一单事务入口），会再解析一次负载；大列表（FLOW-PROF-F 10000 条）响应时间未计时。内容哈希物化已避免孤儿文件。
5. **文件导出只读性**：`write_export_file` 失败分支仅在合成桥验证为可读结构化错误；真实文件系统权限失败未实测。
6. **完整配置导出（Export2ClientConfig）**：沿用既有 `exportClientConfigText`，与原版再导入未做本轮实测。

## 下一步前置

- 整合者生成 FRB 绑定：暴露 `preview_import_text` 与新增单事务 `commit_import_text(profiles, subid)`，令 Dart 无组导入也走单事务且避免组提交二次解析。
- R4-17（IsSub 删除范围）需与本卡衔接：确认订阅替换只删 `is_sub=1`，手工 `is_sub=false` 节点留存。
- 建立 17 格式逐条 matrix 夹具（合成），补真实数据目录重开与跨进程导出再导入证据。
