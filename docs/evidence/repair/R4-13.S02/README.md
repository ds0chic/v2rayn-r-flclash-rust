# R4-13.S02 evidence (ClashUIItem)

- 状态：implemented（配置链映射）；blocked（真实 mihomo 内核接线）。
- 字段：FLD-CFG-129..136（EnableIPv6、EnableMixinContent、ProxiesSorting、ProxiesAutoRefresh、ProxiesRefreshInterval、ConnectionsAutoRefresh、ConnectionsRefreshInterval、ConnectionsColumnItem）。
- 真实缺口（对照冻结上游 `CoreConfigClashService.cs:94/166`）：`EnableIPv6`/`EnableMixinContent` 在本仓库无任何可达消费者；`crates/application/src/mixin.rs` 的 `generate_mihomo`/`MixinOptions` 只在测试里被调用。
- 修复：`crates/application/src/codegen.rs::mixin_options_from_app` 将 ClashUIItem/IPv6/Mixin 开关与 inbound.allow_lan、TunModeItem.enable_tun、CoreBasicItem.Loglevel 映射到 `MixinOptions`。
- 消费者断言：`codegen::tests::r4_13_s02_clash_ui_item_reaches_mihomo_merge`，证明 `EnableIPv6` 写入 `ipv6: true`，`EnableMixinContent=false` 跳过 Mixin 合并。
- Dart 合同：`test/r4_13_s02_contract_test.dart`（2 例，pass）。
- blocked：engine 生成 mihomo 时的 `mixin_options_from_app` 调用点位于另一代理修改中的 `engine.rs`，本批未接线；Refresh/Column 字段由 Clash 视图消费（UI 刷新偏好），未做视图级实测。
