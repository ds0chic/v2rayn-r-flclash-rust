# R4-13.S05 证据：CoreBasicItem 设置保存到实际效果

- 任务：docs/repair/tasks/R4-13.S05.md（R4-13 协调包首个实例）
- 仓库 HEAD：`841accdeab3147a8f97d0f80720bedfa388a6c86`（执行时工作树含其他代理改动，未触碰）
- 冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，UP=work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/
- 应用基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- armed=false；未启动本项目内核，未监听端口，未写宿主代理/注册表/路由/TUN/自启，未读用户凭据。

## 结论

实例状态 **verified（配置生成消费者）**。

- 修复前：保存路径（D14 / UFS-06）不 await `applyActive`，应用失败仍关窗返回成功；复现测试 `test/repair/r4_13_s05_repro_test.dart` 失败。
- 修复后：`SettingsController.saveAndApply` 持久化成功后 await 真实 apply，区分 saved/applied，失败可见并回传重启/未应用提示；两条保存入口统一走该用例。复现测试转绿，合同测试全绿。
- 消费者：codegen 由 `crates/application/src/codegen.rs::core_basic_item_reaches_generated_config` 断言（settings_from_app 投影 + xray/sing-box 实际输出）。

## 复现（先失败后通过）

| 阶段 | 命令 | 结果 | 日志 |
|---|---|---|---|
| 修复前 | `flutter test test/repair/r4_13_s05_repro_test.dart --reporter expanded` | FAIL：`Found 0 widgets with key <'settings-save'>`（应用失败仍关窗假成功） | `_prefix_repro.log` |
| 修复后 | `flutter test test/r4_13_s05_contract_test.dart` | PASS 4/4 | `_postfix_tests.log` |
| 修复后 | `flutter test test/repair/r4_13_s05_repro_test.dart` | PASS 1/1 | `_postfix_tests.log` |

注：本仓库 widget 测试跨文件同批运行存在已知 did-not-complete 抖动（审计 settings-audit.md 亦记录），故合同与复现分开单文件运行；两文件各自单跑均绿。

修复前通过临时 `git stash push -- <本卡4个settings文件>` 取得旧实现，测完 `git stash pop` 复原；未 add/commit，未触碰其他代理改动。

## 必过命令结果

- `flutter analyze` → No issues found!
- `flutter test test/r4_13_s05_contract_test.dart` → All tests passed（4/4）
- `flutter test test/repair/r4_13_s05_repro_test.dart` → All tests passed（1/1）
- `cargo fmt --all -- --check` → 通过（无输出）
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → 通过
- `cargo test --workspace --locked` → EXIT=0，`test result: ok` ×105，`FAILED`×0，`panicked`×0
- `flutter build windows --release` → `Built build\windows\x64\runner\Release\v2rayn_desktop.exe`

## 上游对照

- `UP/v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:330-344` 写回 `CoreBasicItem.*`；`:152-166` 读取绑定。
- 保存后主窗 `MainWindowViewModel` Reload（应用计划）；自启 `AutoStartupHandler` 在保存成功后处理。
- 消费点：V2rayLogService.cs:9/12/18、SingboxLogService.cs:9/14/24/28、V2rayOutboundService.cs:307/317/364、SingboxOutboundService.cs:442/452/493、V2rayConfigTemplateService.cs:227/261、SingboxConfigTemplateService.cs:148/161、V2rayInboundService.cs:84、CoreConfigV2rayService.cs:59/63/205/209/281/285、SingboxRoutingService.cs:133/150、SingboxStatisticService.cs:16、CoreConfigClashService.cs:77、Builder/CoreConfigContextBuilder.cs:119-133/172-173。
- 本项目对应：`crates/application/src/codegen.rs:318-329`（settings_from_app）+ `crates/config_codegen/src/{xray,singbox}/*`。

## blocked / 未验证

- 真实 Windows 内核重启、系统级/硬件级效果未在授权隔离环境实测 → 登记 blocked/未验证，不伪造。
- 独立原生窗（第二 engine）中的应用失败/重启提示可见性未做真机截图验证（主窗状态提示已在合成测试断言）。
- 本实例不涉及 HWA 实际硬件加速与证书安装。

## 文件哈希（SHA256）

- test/r4_13_s05_contract_test.dart ：529E9BBCA2FB8B99D8D3FB0887EAB7080832508248E9E31C03DECBF26D210F51
- test/repair/r4_13_s05_repro_test.dart ：474130A23E5B8BE5FCD417B1ED9253843E225B542BC295B35A24DC38986D41E0
- _prefix_repro.log ：059B33C4A299C3D3B9ABD3C6D7F65F6850EA7C95ACC9EDB08F913CA155DEE6D6
- _postfix_tests.log ：28B6B1D138EF9BDFCCCEABBEDF03B7F90267B29FBCD3FAE08D06247B8F978287

## 改动文件

- apps/desktop/lib/features/settings/settings_controller.dart（新增 `SettingsApplyOutcome` + `saveAndApply`；autostart/apply/提示）
- apps/desktop/lib/features/settings/settings_actions.dart（主引擎回调改走 `saveAndApply`）
- apps/desktop/lib/features/settings/option_setting_window.dart（内嵌窗确定改走 `saveAndApply`；回传重启提示）
- apps/desktop/lib/features/settings/settings_window_host.dart（message 语义文档）
- crates/application/src/codegen.rs（新增 codegen 消费者测试）
- apps/desktop/test/r4_13_s05_contract_test.dart（新增）
- apps/desktop/test/repair/r4_13_s05_repro_test.dart（新增）
- apps/desktop/test/r4_11_contract_test.dart（补 runtimeBridgeProvider 成功替身，不改 R4-11 预期）
- compat/fields.settings.yaml（FLD-CFG-026..034 追加 r4_13_evidence，仅追加）
- docs/repair/tasks/R4-13.md、R4-13.S05.md（状态与协调登记）

## 清理

- 未启动任何进程；未新增监听；未改宿主设置。修复前 stash 已 pop，无残留 stash。
