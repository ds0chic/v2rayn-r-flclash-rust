"""Audit inventory only. Does not edit production sources or compatibility ledgers."""
from pathlib import Path
import csv
import json
import re
from collections import Counter

import yaml

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
GROUPS = {
    "CoreBasicItem", "InItem", "KcpItem", "GrpcItem", "TunModeItem",
    "SpeedTestItem", "RoutingBasicItem", "Mux4RayItem", "Mux4SboxItem",
    "HysteriaItem", "Fragment4RayItem", "SimpleDNSItem", "HappyEyeballs4RayItem",
}
CONSUMERS = {
    "CoreBasicItem": "crates/config_codegen/src/xray/log.rs:7; xray/outbound.rs:669; xray/config.rs:99,313,363; crates/config_codegen/src/singbox/log.rs:8; singbox/outbound.rs:870,880,944; singbox/config.rs:101,118; singbox/stat.rs:16",
    "InItem": "crates/application/src/engine.rs:2744 runtime_codegen_options; crates/config_codegen/src/xray/inbound.rs:14; crates/config_codegen/src/singbox/inbound.rs:13; singbox/routing.rs:125",
    "KcpItem": "crates/config_codegen/src/xray/outbound.rs:760 (network=kcp branch)",
    "GrpcItem": "crates/config_codegen/src/xray/outbound.rs:824 (network=grpc branch)",
    "TunModeItem": "crates/config_codegen/src/xray/inbound.rs:59; crates/config_codegen/src/singbox/inbound.rs:27; singbox/routing.rs:103; crates/application/src/tun_plan.rs:183; crates/application/src/engine.rs:2838 pre_socks_of",
    "SpeedTestItem": "apps/desktop/lib/features/profiles/profiles_controller.dart:482,1391; crates/bridge_api/src/api/speedtest.rs:402; crates/application/src/speedtest.rs:934,953,1028,1083",
    "RoutingBasicItem": "crates/config_codegen/src/xray/routing.rs:23; crates/config_codegen/src/singbox/routing.rs (routing_basic + active profile override)",
    "Mux4RayItem": "crates/config_codegen/src/xray/outbound.rs:577 fill_outbound_mux",
    "Mux4SboxItem": "crates/config_codegen/src/singbox/outbound.rs:805 fill_multiplex",
    "HysteriaItem": "crates/config_codegen/src/xray/outbound.rs:882 fill_hysteria; crates/config_codegen/src/singbox/outbound.rs:627 fill_hysteria2",
    "Fragment4RayItem": "crates/config_codegen/src/xray/config.rs:237 fragment_mask; crates/application/src/settings.rs:91 validate_settings",
    "SimpleDNSItem": "crates/config_codegen/src/xray/dns.rs:177 build_dns; crates/config_codegen/src/singbox/dns.rs:39 build_dns; singbox/routing.rs:25,156",
    "HappyEyeballs4RayItem": "crates/config_codegen/src/xray/dns.rs:349 set_sockopt_domain_strategy (switch gate missing)",
}
SPIKES = {
    "CoreBasicItem": "R4-13.S05", "InItem": "R4-13.S13", "KcpItem": "R4-13.S14",
    "GrpcItem": "R4-13.S09", "TunModeItem": "R4-13.S22", "SpeedTestItem": "R4-13.S20",
    "RoutingBasicItem": "R4-13.S18", "Mux4RayItem": "R4-13.S16", "Mux4SboxItem": "R4-13.S17",
    "HysteriaItem": "R4-13.S12", "Fragment4RayItem": "R4-13.S07",
    "SimpleDNSItem": "R4-13.S19", "HappyEyeballs4RayItem": "R4-13.S11",
}
SPECIAL = {
    "SimpleDNSItem.EnableHappyEyeballs": ("identified", "AUD-ENG-01", "switch is serialized/projected but never read; off/on produce identical Xray config"),
    "Fragment4RayItem.MaxSplit": ("identified", "AUD-ENG-02", "valid upstream range 1-3 rejected by both UI and Rust save validator"),
    "TunModeItem.EnableLegacyProtect": ("identified", "AUD-ENG-03", "sidecar decision exists; protected core executable context is always empty in production assembly"),
    "TunModeItem.EnableIPv6Address": ("identified", "AUD-ENG-04", "address reaches TUN; production global IPv6 context stays false; Xray generated system routes omit ::/0"),
    "TunModeItem.IPv6Address": ("identified", "AUD-ENG-04", "address reaches TUN; route effect needs actual IPv6 context and isolated verification"),
    "TunModeItem.RouteExcludeAddress": ("identified", "AUD-ENG-06", "UI split retains empty entries; trailing comma yields invalid empty CIDR"),
    "RoutingBasicItem.RoutingIndexId": ("identified", "AUD-ENG-07", "only serialized; frozen upstream one-time selected-route migration has no production consumer here"),
}
for leaf in ("TryDelayMs", "PrioritizeIPv6", "Interleave", "MaxConcurrentTry"):
    SPECIAL[f"HappyEyeballs4RayItem.{leaf}"] = (
        "identified", "AUD-ENG-01", "parameters reach Xray output even when EnableHappyEyeballs is false",
    )

def snake(name):
    replacements = {"IPv4Address": "ipv4_address", "IPv6Address": "ipv6_address",
                    "EnableIPv6Address": "enable_ipv6_address", "XudpProxyUDP443": "xudp_proxy_udp443",
                    "IPAPIUrl": "ipapi_url", "UdpTestTarget": "udp_test_target",
                    "DomainStrategy4Singbox": "domain_strategy4_singbox", "MaxSplit": "max_split",
                    "EnableCacheFile4Sbox": "enable_cache_file4_sbox", "TryDelayMs": "try_delay_ms",
                    "FakeIP": "fake_ip", "GlobalFakeIp": "global_fake_ip", "FakeIPRange": "fake_ip_range",
                    "BlockAAAAQuery": "block_aaaa_query", "DirectDNS": "direct_dns",
                    "RemoteDNS": "remote_dns", "BootstrapDNS": "bootstrap_dns",
                    "NewPort4LAN": "new_port4_lan", "AllowLANConn": "allow_lan_conn"}
    if name in replacements:
        return replacements[name]
    name = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    name = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    return name.lower()

def refs(file, literal, first=1, last=10**8):
    lines = (ROOT / file).read_text(encoding="utf-8").splitlines()
    hits = [f"{file}:{n}" for n, line in enumerate(lines, 1)
            if first <= n <= last and literal in line]
    return "; ".join(hits[:3])

items = yaml.safe_load((ROOT / "compat/fields.settings.yaml").read_text(encoding="utf-8"))["items"]
rows = []
for item in items:
    path = item["source_symbol"]
    group, leaf = path.split(".", 1)
    if group not in GROUPS:
        continue
    status, issue, note = SPECIAL.get(path, ("implemented", "", "static consumer mapped; end-to-end effect not independently verified in this audit"))
    if group == "TunModeItem" and path not in SPECIAL:
        status = "blocked"
        note = "configuration/helper projection exists; real auto-route/TUN/IPv6/cleanup effect needs isolated OS verification"
    if leaf in {"Length", "Interval"} and group == "Fragment4RayItem":
        status = "implemented"
        note = "legacy-only migration fallback on load when modern Lengths/Delays missing; not a current UI control"
    if path == "InItem.Protocol":
        note = "internal SOCKS/SOCKS2/SOCKS3 identities map to mixed listener; no user-visible protocol selector upstream"
    if group in {"KcpItem", "GrpcItem", "Mux4RayItem", "HappyEyeballs4RayItem"}:
        note += "; Xray-specific, and matching transport/mux/domain strategy must be active"
    if group == "Mux4SboxItem":
        note += "; sing-box-specific and node mux must be enabled"
    if group == "HysteriaItem":
        note += "; Hysteria transport/protocol only; node-level nonnegative values override global defaults"
    if path in {"TunModeItem.AutoRoute", "TunModeItem.StrictRoute", "TunModeItem.Stack", "TunModeItem.IcmpRouting"}:
        note += "; frozen upstream these fields are consumed by sing-box TUN, not Xray TUN; Xray writes autoSystemRoutingTable separately"
    if path in {"InItem.UdpEnabled", "InItem.DestOverride", "InItem.RouteOnly"}:
        note += "; Xray consumes directly; sing-box parity uses sniff routing action and does not consume all Xray-only inbound options"
    if path == "SimpleDNSItem.BlockBindingQuery":
        note += "; sing-box query-type binding rule only, same frozen upstream core scope"
    ui_file = "apps/desktop/lib/features/settings/option_setting_window.dart"
    ui_refs = refs(ui_file, f"'{leaf}'", 480)
    if group == "SimpleDNSItem":
        ui_refs = "apps/desktop/lib/features/routing/dns_window.dart:751; apps/desktop/lib/features/routing/dns_controller.dart:94; " + ui_refs
    if not ui_refs:
        ui_refs = "no original dedicated OptionSettingWindow control; typed setting preserved/editable through config or relevant window"
    projection = refs("crates/application/src/codegen.rs", f".{snake(leaf)}", 363, 450)
    if group == "SimpleDNSItem":
        projection = refs("crates/application/src/codegen.rs", f".{snake(leaf)}", 543, 580)
    if group == "SpeedTestItem":
        projection = refs("apps/desktop/lib/features/profiles/profiles_controller.dart", f"'{leaf}'", 482, 520)
    if path == "RoutingBasicItem.RoutingIndexId":
        projection = "crates/domain/src/settings.rs:358 (serialization only; no application reads)"
    if leaf in {"Length", "Interval"} and group == "Fragment4RayItem":
        projection = "crates/domain/src/settings.rs:1074 (apply_load_defaults legacy fallback)"
    if path == "TunModeItem.EnableLegacyProtect":
        projection = "crates/application/src/engine.rs:2838 pre_socks_of"
    evidence = f"{projection}; docs/evidence/repair/{SPIKES[group]}/README.md (historical evidence, not fresh real-OS validation)"
    if issue in {"AUD-ENG-01", "AUD-ENG-02", "AUD-ENG-04", "AUD-ENG-06"}:
        evidence += "; docs/evidence/tun-settings-audit-2026-10-05/engine-probe/locked-replay.log (fresh pure reproduction)"
    effect = json.dumps(item.get("generated_into", {}), ensure_ascii=False)
    if path == "RoutingBasicItem.RoutingIndexId":
        effect = "expected one-time legacy selected-route migration; absent; normal current route selected via RoutingProfile.is_active"
    rows.append({
        "id": item["id"], "group": "Inbound" if group == "InItem" else group,
        "path": path, "ui_control": f"{item['ui_control']}; {ui_refs}",
        "consumer": CONSUMERS[group], "effect": effect, "effect_time": item["apply_timing"],
        "status": status, "issue": issue, "evidence": evidence,
        "actual_effect_verified": "false", "notes": note,
        "upstream": item["source_file"] + "::" + item["source_symbol"],
    })

assert len(rows) == 89, len(rows)
fields = ["id", "group", "path", "ui_control", "consumer", "effect", "effect_time", "status", "issue", "evidence", "actual_effect_verified", "notes", "upstream"]
with (OUT / "engine-fields.csv").open("w", encoding="utf-8-sig", newline="") as handle:
    writer = csv.DictWriter(handle, fieldnames=fields)
    writer.writeheader()
    writer.writerows(rows)
print(json.dumps({"rows": len(rows), "groups": dict(Counter(row["group"] for row in rows)), "statuses": dict(Counter(row["status"] for row in rows))}, ensure_ascii=False))
