# R4-11 字段表单和默认值 — 证据

- HEAD：`b0ce431`（卡内基线）；应用基线 `77c74ed`。`armed=false`。
- 范围：settings 五页字段表单、默认值回填与字段联动；profiles 编辑器 TLS/Reality 联动；routing/DNS 字段表单最小改动。
- 夹具：`SyntheticBridgePort` + `MemoryUiStateStore`；纯 widget/内存桥，不加载原生库、不启核、不监听端口、不写宿主代理/TUN/路由/注册表、不读用户数据。
- 上游：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`（冻结 `7d6a967`）。

## 改动
- `apps/desktop/lib/features/settings/settings_defaults.dart`
  - 新增 `mergeWithSettingsDefaults(document)`：按 `defaultSettingsJson()` 逐字段回填缺失/`null` 且原生默认非 null 的标量；对象递归合并；present 非空值优先；列表与 null-默认字段保持原样。对齐 `ConfigHandler.LoadConfig` + `OptionSettingViewModel.Init` 的初始读取层。
- `apps/desktop/lib/features/settings/option_setting_window.dart`
  - host 快照与非 host 草稿均经 `mergeWithSettingsDefaults` 播种，避免部分快照渲染成 CLR 零值/空值或把字面 `null` 写回。
  - Mock IP：`启用 FakeIP`/`全局 FakeIP` 增加可见联动键 `fakeip-toggle`/`global-fakeip-toggle`，GlobalFakeIp 仅在 FakeIP 勾选时出现（对齐 `DNSSettingWindow`）。
  - 认证用户名/密码按 `NewPort4LAN` 联动禁用（对齐 `OptionSettingWindow.xaml.cs:70-72` 的 `togNewPort4LAN -> txtuser/txtpass.IsEnabled`）。
- `apps/desktop/lib/features/settings/settings_fields.dart`
  - `SettingsTextField` 增加 `enabled` 参数，未改值语义。
- `compat/fields.settings.yaml` / `compat/fields.entities.yaml`
  - 仅追加 `r4_11_evidence` 键到已核字段（不删行、不改分母）：Loglevel、EnableCacheFile4Sbox、TrayMenuServersLimit、LocalPort、NewPort4LAN、FakeIP、GlobalFakeIp、StreamSecurity、PublicKey。

## 上游对照
- `ConfigItems.cs`：`CoreBasicItem.EnableCacheFile4Sbox = true`、`GUIItem.TrayMenuServersLimit = 20`、`HysteriaItem.HopInterval = 30`、`TunModeItem.AutoRoute/StrictRoute/EnableLegacyProtect = true`、`SystemProxyItem.NotProxyLocalAddress = true`、`CheckUpdateItem.UpdateViaProxy = true`、`SimpleDNSItem` 各 `??=`。
- `OptionSettingViewModel.Init/SaveSettingAsync`：缺省读取与保存映射；端口范围 `1..Global.MaxPort(=65536)`；分片范围校验；`needReboot` 字段集合。
- `OptionSettingWindow.xaml.cs`：`togNewPort4LAN` 绑定 `txtuser/txtpass.IsEnabled`；下拉候选来源 `Global`。
- `Global.cs`：`Fingerprints`、`LogLevels`、`TunStacks/Mtus`、`RootCertProviders`、`destOverrideProtocols` 等候选与默认。
- profiles：`profile_fields.dart securityFields` 仅在 `streamSecurity==reality` 时包含 PublicKey/ShortId/SpiderX/ML-DSA；`profile_editor_dialog.dart` Reality 时 PublicKey 必填。

## 断言：先失败后通过
- `apps/desktop/test/repair/r4_11_repro_test.dart`
  - 用例1 `GlobalFakeIp only visible when FakeIP is on`：修复前因缺 `fakeip-toggle` 键失败（`Found 0 widgets with key`）；修复后通过。
  - 用例2 `null/empty group falls back to upstream defaults`：临时还原 `_draft = document`（禁用 merge）后失败于 `EnableCacheFile4Sbox isTrue`（line 140），`GuiItem` 空组拿不到 `TrayMenuServersLimit=20`；恢复 merge 后通过。
- `apps/desktop/test/r4_11_contract_test.dart`（新增合同）
  - `mergeWithSettingsDefaults`：present 优先、缺失回填、null 默认保持。
  - 编辑端口 11888 → 确定 → 保存重开一致；取消不落库（revision 0，仍 10808）；端口 70000 校验可见且不关窗、不落库；User/Pass 随 NewPort4LAN 联动。

## 状态
`implemented`。仅合成 widget/内存桥合同测试；未做真实 Windows 窗口、真实 FRB/SQLite、真实内核/平台效果、DPI/字体矩阵实测 → 未 `verified`。
