# UX-PARITY-FIX-16 evidence

FIX-16 第一批：设置页字段消费矩阵 + 冻结五页分组。

- `field-matrix.md`：101 个可见控件的 UI/类型/默认/校验 → bridge → domain(apply_timing) → 存储 → 消费者 → 平台效果，含状态口径与统计。
- `observations.json`：改动文件、实际命令与结果、状态统计、blockers、环境约束自证。

## 结论摘要

- 上游 WPF 与 Avalonia 的 `OptionSettingWindow` 都是 5 个有效页（Core / N / SystemProxy / TunMode / CoreType），KCP 页在 XAML 注释内；FakeIP/HappyEyeballs 属 `DNSSettingWindow`；`ClashUIItem` 在本窗口无控件。本卡把此前的 12 页收敛为 5 页，并把上述历史字段放进带标注的「历史保留」段（保留可编辑，不作为缺口）。
- 补齐缺失字段：EnableHWA、RootCertProvider、KeepOlderDedupl、CurrentLanguage、SubConvertUrl、GeoSourceUrl、SrsSourceUrl、RouteRulesTemplateSourceUrl、EnableCacheFile4Sbox、CustomSystemProxyScriptPath。
- 修复重开丢值根因（草稿在 load 完成前被默认值填充，`_draft.isEmpty` 判断失效）。
- 第一批实际生效链：语言（`UiItem.CurrentLanguage` → `MaterialApp.locale`）、全局字体（`CurrentFontFamily/Size` → `buildAppTheme`）；HWA/证书来源已持久化并按 apply_timing 接线。
- 统计：verified 3 / consumed 68 / registered 7 / blocked 17 / preserved_only 9。

## 未完成（后续卡）

FIX-16B（Geo/SRS/路由源/SubConvert + 完整本地化）、FIX-16C（Clash UI 刷新排序）、FIX-16D（TypeName 窗口尺寸统一）、FIX-16E（证书安装平台效果）、FIX-16F（真实窗口集成测试，环境抖动留给根代理）。
