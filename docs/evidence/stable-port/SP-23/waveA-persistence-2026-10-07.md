# Wave A persistence acceptance — 2026-10-07
(`crates/persistence`, synthetic only)

Scope: Wave A persistence-package instances from
`acceptance-waves-2026-10-07.md` (Wave A rows 003-021, 106-113, 116 plus the
persistence-referencing rows 011/012/018/124/126/127/128/161).
Pre-existing coverage check: `crates/persistence/tests/edge_cases.rs` holds
only 5 generic tests (`empty_source_imports_zero_rows`,
`null_absent_and_empty_are_preserved_distinctly`,
`zero_port_is_a_warning_not_a_commit_blocker`, `all_fields_config_is_stored`,
`failed_candidate_never_touches_an_existing_target`). The ledger-cited
`edge_cases.rs:92-104/101/102/103/104/113` are JSON fragments inside the
single `all_fields_config_is_stored` smoke test — explicitly NOT passing
acceptance per the ledger §1. Nothing was reused verbatim; nothing was
duplicated. All sibling cases below are new in
`crates/persistence/tests/wave_a_groups.rs`.

Gate per case: canonical value persists; group 5-state retention
(present / absent / empty-object / explicit-null / partial, unknown keys
kept); patch isolation (every other top-level group byte-identical after a
targeted resubmission); independent reopen reads back the same document.
No sockets opened, no OS writes, no 10808 reference (asserted in-test).

Command (workspace root, only package run):
`cargo test -p persistence --locked`
Result: ok — 131 passed, 0 failed (unit 74 + edge_cases 5 + real_shape 8 +
sp01_config_text 5 + upstream_import 8 + wave_a_groups 31). Two
self-introduced failures were fixed before the final run (116 asserted
source row ids, but import remaps ids via id_map; then asserted exact row
count, but a changed config hash re-imports rows as a new batch — both
assertions were replaced with id/count-agnostic invariants; production code
untouched).

| instance | test name | result | assertion summary |
|---|---|---|---|
| FLD-CFG-003 | fld_003_core_basic_item_group_retain_patch_reopen | pass | CoreBasicItem LogEnabled/Loglevel roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-004 | fld_004_tun_mode_item_group_retain_patch_reopen | pass | TunModeItem EnableTun/Mtu roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-005 | fld_005_kcp_item_group_retain_patch_reopen | pass | KcpItem Mtu/Tti roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-006 | fld_006_grpc_item_group_retain_patch_reopen | pass | GrpcItem IdleTimeout roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-007 | fld_007_routing_basic_item_group_retain_patch_reopen | pass | RoutingBasicItem DomainStrategy roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-008 | fld_008_gui_item_group_retain_patch_reopen | pass | GuiItem EnableLog/TrayMenuServersLimit roundtrip; 5-state; siblings untouched; reopen (split ownership still needs ruling per ledger §4) |
| FLD-CFG-009 | fld_009_msg_ui_item_group_retain_patch_reopen | pass | MsgUIItem MainMsgFilter/AutoRefresh roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-010 | fld_010_ui_item_group_retain_patch_reopen | pass | UiItem CurrentTheme/FontSize roundtrip (canonical UiItem key); 5-state; siblings untouched; reopen |
| FLD-CFG-011 | fld_011_const_item_group_retain_patch_reopen | pass | ConstItem SubConvertUrl roundtrip; 5-state; 086/087-adjacent siblings unaffected; reopen |
| FLD-CFG-012 | fld_012_speed_test_item_group_retain_patch_reopen | pass | SpeedTestItem group roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-013 | fld_013_mux4ray_item_group_retain_patch_reopen | pass | Mux4RayItem Concurrency roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-014 | fld_014_mux4sbox_item_group_retain_patch_reopen | pass | Mux4SboxItem Protocol/MaxConnections roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-015 | fld_015_hysteria_item_group_retain_patch_reopen | pass | HysteriaItem Up/Down/Hop roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-016 | fld_016_clash_ui_item_group_retain_patch_reopen | pass | ClashUIItem EnableIPv6 roundtrip; 5-state; siblings untouched; reopen (split ownership still needs ruling per ledger §4) |
| FLD-CFG-017 | fld_017_system_proxy_item_group_retain_patch_reopen | pass | SystemProxyItem SysProxyType roundtrip (data only, no WinINET); 5-state; siblings untouched; reopen |
| FLD-CFG-018 | fld_018_webdav_item_group_retain_patch_reopen | pass | WebDavItem Url roundtrip with synthetic placeholders only; 5-state; siblings untouched; reopen |
| FLD-CFG-019 | fld_019_check_update_item_group_retain_patch_reopen | pass | CheckUpdateItem CheckPreReleaseUpdate roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-020 | fld_020_fragment_item_group_retain_patch_reopen | pass | Fragment4RayItem Packets/Lengths roundtrip; 5-state; siblings untouched; reopen |
| FLD-CFG-021 | fld_021_inbound_list_retain_patch_reopen | pass | Root Inbound list roundtrip (port 11911, never bound); absent/empty/null distinct; port patch leaves siblings untouched; reopen |
| FLD-CFG-106 | fld_106_speed_test_timeout_leaf_retain_patch_reopen | pass | SpeedTestTimeout 15→30 persists; sibling leaf survives; 5-state; reopen |
| FLD-CFG-107 | fld_107_speed_test_url_leaf_retain_patch_reopen | pass | SpeedTestUrl persists (stored only, never fetched); sibling leaf survives; 5-state; reopen |
| FLD-CFG-108 | fld_108_speed_ping_test_url_leaf_retain_patch_reopen | pass | SpeedPingTestUrl persists; sibling leaf survives; 5-state; reopen |
| FLD-CFG-109 | fld_109_mixed_concurrency_leaf_retain_patch_reopen | pass | MixedConcurrencyCount 4→1 persists; sibling leaf survives; 5-state; reopen |
| FLD-CFG-111 | fld_111_udp_test_target_leaf_retain_patch_reopen | pass | UdpTestTarget persists; sibling leaf survives; 5-state; reopen |
| FLD-CFG-113 | fld_113_speed_test_delay_interval_leaf_retain_patch_reopen | pass | SpeedTestDelayInterval 3→5 persists; sibling leaf survives; 5-state; reopen |
| FLD-CFG-116 | fld_116_routing_index_id_legacy_roundtrip_no_clobber | pass | Legacy RoutingIndexId stored verbatim for known + unknown ids; no import flips any RoutingItem.IsActive flag; reopen (one-way →IsActive migration producer G-14 still missing, owner SP-13) |
| FLD-CFG-124 | fld_124_mux_max_connections_leaf_retain_patch_reopen | pass | MaxConnections 4→16 persists; Protocol sibling survives; 5-state; reopen |
| FLD-CFG-126 | fld_126_hysteria_up_mbps_leaf_retain_patch_reopen | pass | UpMbps 250→100 persists; Down/Hop siblings survive; 5-state; reopen |
| FLD-CFG-127 | fld_127_hysteria_down_mbps_leaf_retain_patch_reopen | pass | DownMbps 500→100 persists; Up/Hop siblings survive; 5-state; reopen |
| FLD-CFG-128 | fld_128_hysteria_hop_interval_leaf_retain_patch_reopen | pass | HopInterval 45→30 persists; Up/Down siblings survive; 5-state; reopen |
| FLD-CFG-161 | fld_161_simple_dns_fake_ip_leaf_retain_patch_reopen | pass | SimpleDNSItem.FakeIP true→false persists; RemoteDNS sibling survives; 5-state; reopen |

Uncovered (in scope of the wave but not of this package gate):
- FLD-CFG-110, FLD-CFG-112 (Wave B): SP-29 formal-entry UI→FRB→save→reopen
  click-through required; persistence roundtrip pattern is identical but the
  gate needs the GUI surface first. Not written.
- No other Wave A persistence-package instance is uncovered. 008/016 are
  covered above at the data layer; only their split-ownership ruling
  (ledger §4) remains open and is unaffected by these tests.

Not edited: any `FLD-CFG-*.md`, the ledger CSV, manifests, production code.
No commit. 127.0.0.1:10808 untouched (no listener/harness uses it; tests
assert the serialized docs never contain "10808").
