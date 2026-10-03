# UX-PARITY-FIX-04B — 完整配置导入证据

任务卡：`docs/tasks/FIX-04B.md`（FIX-04 后续卡；登记来源 `repair-queue.md` 第 22 行“完整配置导入另卡”）。

## 结论

完整配置（v2ray/Xray JSON、sing-box JSON、Clash YAML）经既有剪贴板/文本导入入口被识别为 `Custom` 节点；原文按 RT-08 文件型落盘（`Address` → `<data>/config/<indexId><ext>`），重开可读；生成内核配置时经 `build_codegen_input` 的 `custom_file_text` 路径实际包含其原文与未知键。冻结源码 `AddBatchServers4Custom` / `V2rayFmt.ResolveFull` / `SingboxFmt.ResolveFull` 均为“写文件 + `Address=文件名`”，与本实现一致；未改 `v2rayn://` inner URI 语义。

## 命令与结果（日志见同目录）

| 日志 | 命令 | 结果 |
|---|---|---|
| `postfix-subscriptions-fmt.log` | `cargo fmt -p subscriptions -- --check` | 0 差异 |
| `postfix-fmt-check.log` | `rustfmt --edition 2021 --check crates/application/src/codegen.rs crates/bridge_api/src/api/subs.rs` | 0 差异 |
| `postfix-subscriptions.log` | `cargo test -p subscriptions --locked` | 全绿（parity 9/9、parse_pipeline 13/13） |
| `postfix-application.log` | `cargo test -p application --locked --lib` | 183 passed；含 `codegen::tests::active_file_type_custom_consumes_outbound_contents` |
| `postfix-bridge-subs.log` | `cargo test -p bridge_api --locked api::subs::tests` | 12 passed（新增 4） |
| `postfix-clippy.log` | `cargo clippy -p subscriptions -p application -p bridge_api --all-targets --locked -- -D warnings` | 通过 |
| `postfix-flutter-config-import.log` | `flutter test test/ux_parity_fix04b_config_import_test.dart` | 2/2（真桥文件型导入/持久化/重开 + 取消） |
| `postfix-flutter-regression.log` | `flutter test test/ux_parity_fix04_inner_test.dart test/t21e_import_ux_test.dart` | 6/6（无回归） |
| `postfix-flutter-analyze.log` | `flutter analyze` | No issues found |

## 关键断言

- `full_v2ray_config_imports_as_file_type_and_generates`：`ConfigType=Custom`、`CoreType=Xray`、`Address` 非空、`proto_extra.extra_json` 无 `customConfigText`、`extra_json` 无 `RawConfig`；文件内容含 `fix04b-marker` 与未知键 `x-future`；`save_imported_profile` 后 `get_profile` 保 `Address`；`build_codegen_input`+`generate` 输出含 `fix04b-marker`、`x-future`。
- `full_singbox_config_imports_as_file_type`：`CoreType=SingBox`，文件含原文与未知键。
- `clash_yaml_imports_as_file_type_yaml`：`CoreType=Mihomo`，`Address` 以 `.yaml` 结尾，文件原文保留。
- `file_type_and_inline_type_do_not_impersonate`：文件型 `Address` 非空且无内联键；FIX-04 inner `CustomOutboundObj` 内联型 `Address` 为空且含 `customConfigText`。
- Flutter 真桥：`<dir>/config/<address>` 文件存在且含原文；`persistImportedProfiles.saved=1`；同目录重开后节点仍在；自由文本 `importFromText.ok=false` 且不新增节点；粘贴对话框取消不改变节点数。

## 未完成 / 缺口

- 订阅刷新（`application::subs::refresh_subscriptions*`）直接落库，未走 bridge `import_from_text`，完整配置类订阅内容的文件物化未覆盖（`crates/application` 本卡禁改）。
- `compat/actions.yaml` `ACT-MAIN-016` 状态提升由根代理判定。
