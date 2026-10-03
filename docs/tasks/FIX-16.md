# FIX-16 设置页字段消费矩阵与冻结五页分组（第一批）

状态：`implemented`（分组/缺字段/候选联动 + 语言/字体实际消费者已在本机 widget 测试验证；真实 Windows 窗口集成测试未运行，见「未完成」）。
任务 ID：FIX-16（来源 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 41/43 行；findings SET-16/17/19/20、ROOT-07/09、PR-23/25、RT-16/17）。

本次唯一用户流程：打开「参数设置」→ 看到与冻结上游 `OptionSettingWindow.xaml` 一致的 5 个 TabItem → 编辑字段（含此前缺失的 HWA/证书来源/语言）→「保存」→ 关闭 → 重开窗口，字段值从持久化文档恢复；语言/字体在保存后即时反映到 `MaterialApp`（locale / 主题字体），而不是只报「已保存」。

前置任务及已验证证据：FIX-08 已修复 Apply 语义（用眼前草稿、未知字段保留，见 `settings_controller.saveDocument`），本卡不回退。审计证据：`docs/evidence/parity-review-2026-10-03/settings-report.md` SET-16/17/19/20、`root-report.md` ROOT-07/09、`profiles-report.md` PR-23/25、`runtime-report.md` RT-16/17；字段集取自 `settings-items.json` 与 `domain::settings_timing::FIELD_TIMING`（180 项）。

对应 feature / field / action / layout ID：`LAY-OPTSET-001/002`、`FLD-CFG-056..064`（GuiItem）、`FLD-CFG-073..083`（UiItem）、`FLD-CFG-084..087`（ConstItem）、`FLD-CFG-106..113`（SpeedTestItem）、`FLD-CFG-129..136`（ClashUIItem）、`FLD-CFG-137..142`（SystemProxyItem）、`FLD-CFG-095..105`（TunModeItem）、`FLD-CFG-046..051`（KcpItem）、`SET-16/17/19/20`、`ROOT-07/09`。

必读上游文件、符号和固定 commit（`7d6a967c18c697f28dc6917122ed3a4993fcf336`）：
- `v2rayN/v2rayN/Views/OptionSettingWindow.xaml`（TabItem 49/619/739/1193/1264/1464；KCP 页 619 起注释）与 `.xaml.cs`
- `v2rayN/v2rayN.Desktop/Views/OptionSettingWindow.axaml`（同 5 个 TabItem：38/547/934/1027/1191）
- `v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs`（字段属性 + `SaveSettingAsync` 端口/分片校验 + `needReboot`）
- `v2rayN/ServiceLib/ViewModels/DNSSettingViewModel.cs`（FakeIP / EnableHappyEyeballs 属 DNS 窗口，不在 OptionSettingWindow）
- `v2rayN/ServiceLib/Global.cs:758`（`RootCertProviders = ["system","chrome","mozilla"]`）
- 本仓库 `crates/domain/src/settings_timing.rs`（apply_timing 权威表）

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：整个 `guiNConfig.json` 文档的深拷贝草稿；控件按上游 group.key 编辑。
- 输出：`saveDocument` 走已有 `saveSettingsJson`（乐观 revision），未知字段保留；无新增 IPC。
- 错误：端口非 1..65535 或分片 Lengths/Delays/MaxSplit 非法时本地拦截并提示（对齐上游 `FillLocalListeningPort` / `FillFragmentParameterError`）；保存失败窗口不关、不报成功。
- 取消：`取消` 丢弃草稿，绝不写 Run 键（沿用 FIX-08）。
- 权限：仅本机 UI + FRB/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：`guiNConfig.json`（上游 SQLite 同构树），重开一致。
- 生效：语言 → `app.dart` `MaterialApp.locale`；字体族/字号 → `buildAppTheme`（`ThemeData.textTheme`）；其余按 `settings_timing` 在内核/应用重启时读取。

允许修改的模块：`apps/desktop/lib/features/settings/{option_setting_window.dart,settings_controller.dart,settings_fields.dart,settings_defaults.dart,theme_setting_dialog.dart}`、`apps/desktop/lib/features/profiles/ui_state_store.dart`（尺寸状态）、新增 `apps/desktop/lib/app/locale_config.dart`、`apps/desktop/lib/app/app.dart`（仅 locale/supportedLocales 接线）、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-16/**`、本卡。未改任何生成文件、未改 `hotkeys.dart`/`main_shell.dart`/`features/subs|runtime|monitor|update|backup`/`crates/**`。

禁止改变的已有行为：FIX-08 的草稿保存语义、`settings-apply`/`settings-save`/`settings-local-port`/`autorun-toggle` 键、编辑器与节点表、菜单结构；不删入口、不降分母、不伪造平台效果。

测试夹具和原版预期：`SyntheticBridgePort`（内存设置文档，revision 递增）；原版预期：5 个 TabItem、Core 页含入站/日志/指纹/绑定/Mux/Hysteria/Fragment、N 页含 UI/托盘/字体语言/测速/资源。

本次必须通过的命（实际运行见「本轮实际结果」）：
- `dart format --output=none --set-exit-if-changed lib test integration_test`
- `flutter analyze`
- 逐文件 `flutter test`（目标文件；偶发 exit 79 重试）
- 真实窗口：`flutter test integration_test/... -d windows`（本卡未运行）

证据文件位置：`docs/evidence/UX-PARITY-FIX-16/field-matrix.md`、`observations.json`、`README.md`、本机测试日志。

完成条件：分组与上游 5 页一致；缺字段补齐；候选/联动正确；语言/字体实际消费者可验证；其余字段逐行登记后续卡；门禁通过。已达成（真实窗口集成测试除外）。

接口缺口处理（登记，不自行削减）：
- Geo/SRS/RouteRules/SubConvert 的下拉候选与下载链在订阅/更新模块，不属本卡所有权 → FIX-16B。
- ClashUIItem 刷新/排序消费者在 Clash UI（未所有权）→ FIX-16C。
- UiItem.WindowSizeItem TypeName 窗口尺寸恢复需要窗口管理（`desktop_integration`/runner），本卡只在 `ui_state_store` 侧统一状态源与迁移 → FIX-16D。
- 完整本地化资源（`flutter_localizations` + ARB）需改 `pubspec.yaml`（非所有权）→ FIX-16B。
- `flutter_localizations` 依赖与其它子代理在改的 `hotkeys.dart`/`desktop_integration.dart` 报错属并行冲突，不属本卡。

本轮实际结果：
- `option_setting_window.dart` 重构为上游 5 页；补齐 EnableHWA、RootCertProvider（system/chrome/mozilla）、KeepOlderDedupl、CurrentLanguage、SubConvertUrl、GeoSourceUrl、SrsSourceUrl、RouteRulesTemplateSourceUrl、EnableCacheFile4Sbox、CustomSystemProxyScriptPath、MacOSShowInDock（macOS 门控）；KCP/DNS/Clash 保留为带标注的「历史保留」段。
- 修复重开丢值根因：草稿此前在 load 完成前由默认值填充，`_draft.isEmpty` 判断失效 → 改为 `_draftInit` 一次性从持久化文档播种。
- 新增 `app/locale_config.dart`；`app.dart` 接入 `localeForLanguage(shell.language)` + `supportedLocales`。
- 测试：`test/fix16_settings_field_test.dart`（3 项）与改写后的 `test/t12a_option_window_test.dart` 全过；`fix08_option_apply/cancel/error`、`t12a_settings_storage` 单跑通过。
- 门禁：`dart format` 0 改变；`flutter analyze` 仅剩并行子代理文件的报错（`desktop_integration.dart:90 _onHotkey` / 早前 `t13_hotkey_test.dart`），与本卡无关且不在所有权内。

## 后续卡（逐行登记，未在第一批完成）

| 卡 | 范围 | 未完成原因 |
|---|---|---|
| FIX-16B | 区域/Geo/SRS/路由源 URL + SubConvert 下载链；`flutter_localizations`+ARB 完整语言包 | 消费者在 `crates/subscriptions|core_adapters` 与 `pubspec.yaml`，非本卡所有权 |
| FIX-16C | ClashUIItem 刷新/排序（ProxiesAutoRefresh/ProxiesSorting/ProxiesRefreshInterval/Connections*） | 消费者在 Clash/monitor UI，非本卡所有权 |
| FIX-16D | `UiItem.WindowSizeItem`/`MainGirdHeight1/2` 按 TypeName 恢复窗口尺寸并与 `ui_state.json` 统一 | 需接 `desktop_integration`/runner 窗口管理，非本卡所有权 |
| FIX-16E | RootCertProvider 证书安装平台效果（chrome/mozilla/system） | T13/CertPemManager，平台测试 |
| FIX-16F | 真实 Windows 窗口集成测试（打开/保存/重开 + 字体不重叠/按钮可达/DPI） | 环境抖动，留给根代理统一跑 |
