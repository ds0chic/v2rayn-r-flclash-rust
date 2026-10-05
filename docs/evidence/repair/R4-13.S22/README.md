# R4-13.S22 evidence (TunModeItem)

- 状态：implemented（存储链 + 生成链）；真实设备创建/路由 blocked。
- 字段：FLD-CFG-095..105（EnableTun、AutoRoute、StrictRoute、Stack、Mtu、EnableIPv6Address、IcmpRouting、EnableLegacyProtect、RouteExcludeAddress、IPv4Address、IPv6Address）。
- 生成链断言（Rust）：`crates/application/src/codegen.rs::tests::r4_13_s22_tun_fields_reach_generated_config`，证明 `TunModeItem` → `CodegenSettings.tun` → 真实 sing-box/xray TUN inbound（mtu/auto_route/strict_route/stack；IPv6/route_exclude）。
- Dart 合同：`test/r4_13_s22_contract_test.dart`（2 例，pass：11 字段保存→重开 + 缺省）。
- 复现护栏：`test/repair/r4_13_s22_repro_test.dart`（部分字段并入默认不回退，pass）。
- 真实 `tun_plan.rs` helper 断言已由既有 `crates/application/src/tun_plan.rs` 单测覆盖（mtu/ipv6/route_exclude/interface_index）。
- blocked：真实 TUN 设备创建、auto-route、路由/DNS/IPv6、崩溃/睡眠恢复需授权隔离 VM（R4-25），本环境不可实测。
