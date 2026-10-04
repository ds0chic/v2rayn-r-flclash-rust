# R3-WPF-Option-Labels — 参数设置窗口可见结构/文案对齐

状态：`implemented`（UI 结构与文案已按冻结上游 XAML/resx 对齐并由 widget 测试覆盖；未在本机重跑真实 Windows 独立窗口对照，窗口形态未改，故不写 `verified`）。

任务 ID：R3-WPF-Option-Labels

本次唯一目标：按冻结原版 WPF `OptionSettingWindow.xaml`（+`.cs`）与 `ServiceLib/Resx/ResUI.zh-Hans.resx` 对齐 RC 参数设置对话框的可见结构/文案：五页 Tab 标题、字段标签（消除裸存储 key）、原版存在而 RC 缺失的字段（认证用户名/密码、默认 TLS 指纹）、状态区裸 key `settings.saved`、按钮集（确定/取消）。窗口形态（RC 内嵌 showDialog vs 原版独立窗口）本轮不改，登记为 `R3-WPF-WINDOW-FORM`。

前置任务及已验证证据：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/README.md`（§4.2 O1–O8）与截图 `original-option.png` / `rc-option.png`；FIX-08（草稿语义）、FIX-16/16B/16C/16D/16E、T12A、T21E 既有测试。开始 HEAD `f9ebe29`，工作树干净。

对应 feature / field / layout ID：`LAY-OPTSET-001/002`（OptionSettingWindow，见 `option_setting_window.dart` 头注释与 `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/v2rayN/Views/OptionSettingWindow.xaml`（TabItem `TbSettingsCore`/`TbSettingsN`/`TbSettingsSystemproxy`/`TbSettingsTunMode`/`TbSettingsCoreType`；Core 页行 0–24；底部 `btnSave`=TbConfirm / `btnCancel`=TbCancel）、`OptionSettingWindow.xaml.cs`（绑定 user/pass 与 NewPort4LAN 的 IsEnabled，而非隐藏）、`ServiceLib/Resx/ResUI.zh-Hans.resx`（`TbSettings*` 文案）、`ServiceLib/Resx/ResUI.resx`（zh-Hans 缺失键的英文回退）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：窗口内可编辑草稿 `_draft`（`SettingsController.draft()` 深拷贝）。
- 输出：`确定` 调 `_save(applyAfter: true)`：校验 → `saveDocument(_draft)` → 成功则 `applyActive()` 并关闭；失败/校验不通过则留在窗口显示错误，不报成功、不应用陈旧文档。
- 取消：`取消` 仅 `Navigator.pop`，丢弃草稿，不落盘（FIX-08 语义保留）。
- 权限/平台：仅 UI + FRB/Rust；系统代理/TUN 实际写读仍属 T13，本页仅保存配置。
- 持久化：SQLite/`guiNConfig.json`（既有 `saveSettingsJson` 乐观 revision 检查）。
- 生效：`_applyImmediate` 即时子集 + `applyActive` 真实计划。

允许修改的模块（本轮实际修改）：`apps/desktop/lib/features/settings/option_setting_window.dart`、`apps/desktop/test/t12a_option_window_test.dart`、`fix08_option_apply_test.dart`、`fix08_option_cancel_test.dart`、`fix15c_autostart_timing_test.dart`、`fix16b_settings_source_test.dart`、`fix16_settings_field_test.dart`、`fix16d_layout_reachability_test.dart`、本卡、`docs/evidence/recheck-fixes/R3-WPF-OPTION/**`、`compat/actions.yaml`（仅追加）。未改 `main_shell.dart`、`app.dart`、frb、`lib/bridge/api/**`、其它 feature、`crates/**`。

禁止改变的已有行为：FIX-08 草稿/取消不落盘；FIX-16B/16C/16D/16E 已提交语义（source URL、ClashUIItem、窗口状态、根证书归一化）；不删入口或降分母。

测试夹具和原版预期：复用既有合成 `SyntheticBridgePort`/`FakePlatformBridge`（无真实节点/订阅/证书、无端口监听、无系统代理/注册表/TUN）。原版预期：Tab 标题/字段文案取自冻结 resx；Core 页始终渲染认证用户名/密码与默认 TLS 指纹；无裸 key；按钮为确定/取消。

本次实际运行的命令与结果：
- `dart format`（8 个改动文件）：0 changed。
- `flutter analyze`（8 个文件）：No issues found。
- `flutter test test/t12a_option_window_test.dart test/fix08_option_apply_test.dart test/fix08_option_cancel_test.dart test/fix08_option_error_test.dart test/fix15c_autostart_timing_test.dart test/fix16b_settings_source_test.dart test/fix16_settings_field_test.dart test/fix16d_layout_reachability_test.dart`：`+12 All tests passed!`。
- `flutter test test/t21e_dialogs_responsive_test.dart test/fix16c_clash_ui_config_test.dart test/fix16e_cert_provider_test.dart test/fix16d_ui_state_store_test.dart`：首跑一次 flake（`t21e` 单条 did not complete），单独与重跑均 `All tests passed!`（`+20`）。

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-OPTION/README.md`。

完成条件：五页 Tab 标题、字段标签、缺失字段、按钮集、裸 key 与上游一致；相关既有设置测试更新并通过；FIX-08 语义不回退；窗口形态不变并登记后续卡。未做真实独立窗口重跑，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 窗口形态缺口（登记，后续卡 `R3-WPF-WINDOW-FORM`）：RC 仍为内嵌 `showDialog`，原版为独立 1000×700 窗口；本轮只对齐内容，未做窗口几何/独立窗口对照。
- 保留分区缺口（登记）：RC 保留 `Core: 基础设置` 页内上游注释掉的 KCP 页与 DNS FakeIP/HappyEyeballs、ClashUIItem 等「历史保留」可编辑区；这些不属于冻结 `OptionSettingWindow` 的可见控件，保留是为了配置不丢失，已显式标注，未静默混入。
- 集成测试缺口（登记，未在允许修改范围）：`apps/desktop/integration_test/ux_parity_fix08_test.dart:191` 与 `integration_test/user_journey_review_test.dart:484` 仍按已移除的 `settings-apply` 键点击；本轮允许范围仅 `apps/desktop/test/**`，未改 integration_test。真实 Windows 集成跑之前需将其改为 `settings-save`（或 `确定` 文案）。

本轮实际结果：`option_setting_window.dart` 的 Tab 标题改为 `Core: 基础设置 / v2rayN 设置 / 系统代理设置 / Tun 模式设置 / Core 类型设置`；对话框标题改为 `设置`；按钮改为 `确定`/`取消`（`确定`=`settings-save` 键，保存草稿+应用+关闭）；Core 页字段改用冻结 resx 文案，补 `认证用户名/认证密码`、`默认 TLS 指纹 (fingerprint)`、端口提示 `Pac 端口 = +3；…`，并移除嗅探/AllowLAN/NewPort4LAN 的显示门控以匹配上游「始终渲染」；所有页面字段标签去除裸 key；`_statusText` 本地化，消除顶部 `settings.saved` 裸 key。修正 R3-WPF-COMPARE O6：Mux/Hysteria2/Fragment 等确属上游 Core 页（XAML 行 17–24），仅 KCP 页被注释，故保留并登记。
