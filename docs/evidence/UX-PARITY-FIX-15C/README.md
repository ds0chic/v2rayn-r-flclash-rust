# UX-PARITY-FIX-15C 证据 — 自启提交时机 / PAC 数据源

任务卡：`docs/tasks/FIX-15C.md`
来源：`docs/tasks/FIX-15.md` 登记卡（repair-queue.md:40 “自启/PAC另卡”）
开始 HEAD：`a940c21`（工作树干净）
上游基准：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`

## 上游对照结论

- 自启提交时机：`OptionSettingViewModel.SaveSettingAsync` 在 `ConfigHandler.SaveConfig` 成功后调用 `AutoStartupHandler.UpdateTask`；`UpdateTask` 先清除再按 `GuiItem.AutoRun` 设置。运行开关、切换控件、取消都不触碰 Run 项。本项目 `option_setting_window.dart` 现有的“保存成功且值变化才写”与之相符；本卡补真机写读删复原与 Flutter 时机测试。
- PAC 数据源：`PacManager.InitText` = 自定义 PAC 存在则用之，否则 `config_dir/pac.txt`；缺失时写入内嵌 `Sample/pac`；读全文后替换 `__PROXY__`。本卡在 Rust 提供等价 `resolve_pac_script`，Dart 侧提供等价 `selectPacFile`/`startPacFromConfig`。
- 模式提交边界：`StatusBarViewModel.SetListenerType` 先改 `SystemProxyItem.SysProxyType` → `UpdateSysProxy` → `SaveConfig`。本项目 `_applyProxyMode` 先应用再 `_persistProxyMode`，顺序一致。

## 实际命令与结果

Rust（单 crate，仓库根）：
- `cargo fmt -p platform -p bridge_api -- --check` → exit 0
- `cargo clippy -p platform -p bridge_api --all-targets --locked -- -D warnings` → exit 0
- `cargo test -p platform -p bridge_api --locked` → exit 0
  - `platform::pac::tests` 5/5：`default_template_has_proxy_placeholder`、`custom_existing_file_is_used`、`missing_custom_falls_back_to_config_pac`、`resolve_seeds_default_when_pac_missing`、`resolve_reads_existing_pac_without_seeding`
  - `bridge_api::api::platform::tests::pac_resolve_script_seeds_and_reads_default` 1/1

Flutter（apps/desktop）：
- `dart format --output=none --set-exit-if-changed <5 changed files>` → 0 changed
- `flutter analyze lib test` → No issues found
- `flutter test test/fix15c_pac_resolve_test.dart` → 8/8
- `flutter test test/fix15c_autostart_timing_test.dart` → 3/3
- 回归（逐文件）：`fix15_tray_pac_test` 8/8、`fix08_option_cancel_test` 1/1、`fix08_option_apply_test` 1/1、`t13_platform_models_test` 10/10、`t13_statusbar_test` 2/2（首次 exit 79，重试通过）、`fix15b_startup_test` 3/3

## 真机写读复原记录（isolated，已复原）

前置快照（`real-pre-state.json`，2026-10-04T02:19:05）：`ProxyEnable=1`、`ProxyServer=127.0.0.1:20808`、`ProxyOverride` 原值、`AutoConfigURL=null`、`HKCU Run` 无 `v2rayNAutoRun*`。

1) 自启 Run 项（`real-autostart.log`）：
- 写入：`HKCU\...\Run\v2rayNAutoRun_<md5(temp probe)> = "\"C:\Users\Colby\AppData\Local\Temp\v2rayn-r-autostart-probe.exe\" --autostart-probe"`
- 删除：`disable` 后查询为 `None`
- 残留校验：`RESIDUAL_RUN_VALUES=[]`（无任何 `v2rayNAutoRun*`）

2) 系统代理 ForcedChange（`real-sysproxy.log`）：
- before：`enabled=true server=127.0.0.1:20808 autoConfig=null autoDetect=false`
- applied：`Server 127.0.0.1:20808 -> 127.0.0.1:11808`
- during：`enabled=true server=127.0.0.1:11808`
- after：`enabled=true server=127.0.0.1:20808 autoConfig=null autoDetect=false`

后置快照（`real-post-state.json`）与前置逐字段比对：`proxyEnable=True proxyServer=True proxyOverride=True autoConfigURL=True runEmpty=True`。

## 未完成 / 未验证

- `flutter build windows --release`、全量 `flutter test`、全量 Rust workspace 门禁未运行（任务约束）。
- 真机只验证 `ForcedChange`；`ForcedClear`/`Unchanged`/`Pac` 未在真机逐一执行（`Pac` 会绑定端口）；四模式状态机由内存 backend 覆盖。
- Rust `pac_resolve_script` 因禁改的 `frb_generated` 未重生成，暂不可被 Dart 调用；App 走 Dart 侧解析，`pac.txt` 缺失时以最小模板播种而非完整上游 `Sample/pac`。
