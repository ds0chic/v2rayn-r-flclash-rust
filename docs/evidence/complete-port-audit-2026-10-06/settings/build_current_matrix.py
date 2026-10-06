import csv
import json
from collections import Counter
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
old = list(csv.DictReader((ROOT / 'docs/evidence/tun-settings-audit-2026-10-05/settings-effectiveness-180.csv').open(encoding='utf-8-sig')))
spec = yaml.safe_load((ROOT / 'compat/fields.settings.yaml').read_text(encoding='utf-8'))
assert {r['id'] for r in old} == {r['id'] for r in spec['items']}

group_consumers = {
    'CoreBasicItem': 'crates/application/src/codegen.rs:363; crates/config_codegen/src/xray/{log,outbound}.rs; crates/config_codegen/src/singbox/{log,outbound}.rs',
    'InItem': 'crates/application/src/codegen.rs:409; crates/application/src/engine.rs runtime_codegen_options; crates/config_codegen/src/{xray,singbox}/inbound.rs',
    'KcpItem': 'crates/application/src/codegen.rs:386; crates/config_codegen/src/xray/outbound.rs',
    'GrpcItem': 'crates/application/src/codegen.rs:392; crates/config_codegen/src/{xray,singbox}/outbound.rs',
    'GuiItem': 'crates/application/src/engine.rs:2040,3261; apps/desktop/lib/features/profiles/profiles_controller.dart:64; apps/desktop/lib/app/shell/desktop_integration.dart',
    'MsgUIItem': 'apps/desktop/lib/features/monitor/logs_view.dart has local controls, no persisted MsgUIItem consumer',
    'UiItem': 'apps/desktop/lib/app/shell/ui_shell_controller.dart:189; apps/desktop/lib/features/profiles/profiles_controller.dart:41; apps/desktop/lib/app/shell/desktop_integration.dart',
    'ConstItem': 'crates/application/src/engine.rs resource_requests/build_codegen_input; crates/application/src/subs.rs conversion request',
    'KeyEventItem': 'apps/desktop/lib/features/settings/hotkeys.dart + hotkey_manager OS plugin; OS dispatch not verified here',
    'CoreTypeItem': 'crates/application/src/engine.rs:2780 resolve_target_core',
    'TunModeItem': 'crates/application/src/codegen.rs:421; crates/application/src/tun_plan.rs; services/net_host/src/session.rs; helper effects incomplete/unverified',
    'SpeedTestItem': 'apps/desktop/lib/features/profiles/profiles_controller.dart:480; crates/application/src/speedtest.rs:96; crates/bridge_api/src/api/speedtest.rs:408',
    'RoutingBasicItem': 'crates/application/src/codegen.rs:436; crates/config_codegen/src/{xray,singbox}/routing.rs',
    'ColumnItem': 'profiles_controller column_layout UI cache; canonical MainColumnItem has no UI consumer',
    'Mux4RayItem': 'crates/application/src/codegen.rs:375; crates/config_codegen/src/xray/outbound.rs',
    'Mux4SboxItem': 'crates/application/src/codegen.rs:382; crates/config_codegen/src/singbox/outbound.rs',
    'HysteriaItem': 'crates/application/src/codegen.rs:396; crates/config_codegen/src/singbox/outbound.rs',
    'ClashUIItem': 'apps/desktop/lib/features/monitor/{proxies_view,connections_view}.dart; codegen Mixin projection only',
    'SystemProxyItem': 'apps/desktop/lib/features/settings/platform_controller.dart:247; crates/application/src/platform_service.rs; native OS outcome not verified here',
    'WebDavItem': 'apps/desktop/lib/features/backup/backup_controller.dart; crates/application/src/webdav.rs',
    'CheckUpdateItem': 'apps/desktop/lib/features/update/update_controller.dart:122,175; crates/bridge_api/src/api/t16.rs:850',
    'Fragment4RayItem': 'crates/application/src/codegen.rs:443; crates/application/src/settings.rs:91; crates/config_codegen/src/xray/outbound.rs',
    'WindowSizeItem': 'native INI geometry cache; canonical WindowSizeItem has no native UI consumer',
    'SimpleDNSItem': 'crates/application/src/codegen.rs:548; crates/config_codegen/src/{xray,singbox}/dns.rs',
    'HappyEyeballs4RayItem': 'crates/application/src/codegen.rs:399; crates/config_codegen/src/xray/dns.rs:374; enable switch ignored',
}

def number(row):
    return int(row['id'].rsplit('-', 1)[1])

issue_override = {
    1: ('identified', 'CP-SET-04', 'active_index_id and canonical IndexId diverge; upstream ZIP activation uses IndexId'),
    2: ('identified', 'CP-SET-11', 'SubIndexId is retained/import-remapped; selected group currently uses another UI state source; full group roundtrip not verified'),
    56: ('identified', 'CP-SET-01', 'same-window retry fixed; reopening load() falsely confirms persisted desired AutoRun'),
    57: ('implemented', 'CP-SET-10', 'backend statistics generation has consumer; showStatistics member never read by table'),
    62: ('identified', 'CP-SET-07', 'EnableHWA has DTO/storage but no runner renderer selection'),
    63: ('identified', 'CP-SET-07', 'EnableLog application logging flag has no production reader'),
    64: ('identified', 'CP-SET-07', 'trust-source enum maps but HTTP clients ignore selected provider'),
    65: ('identified', 'CP-SET-07', 'local log keyword is not seeded/persisted from MainMsgFilter'),
    66: ('identified', 'CP-SET-07', 'local log autoRefresh is not seeded/persisted from MsgUIItem'),
    67: ('identified', 'CP-SET-10', 'autoAdjustColWidth state assigned but never read'),
    68: ('identified', 'CP-SET-10', 'split ratio comes from ui_state rather than canonical MainGirdHeight1'),
    69: ('identified', 'CP-SET-10', 'split ratio comes from ui_state rather than canonical MainGirdHeight2'),
    80: ('identified', 'CP-SET-12', 'macOS activation-policy reader absent; Windows platform not applicable, complete multi-platform scope unfinished'),
    81: ('identified', 'CP-SET-10', 'profile table uses column_layout cache rather than canonical MainColumnItem'),
    82: ('identified', 'CP-SET-10', 'runner uses INI rather than canonical WindowSizeItem'),
    83: ('identified', 'CP-SET-10', 'hideIpInfo state assigned but never read'),
    87: ('identified', 'CP-SET-08', 'configured URL makes import_builtin_routing return explicit unavailable; no async fetch path'),
    100: ('identified', 'CP-SET-13', 'projection exists, production global IPv6 context always false'),
    102: ('identified', 'CP-SET-13', 'protection context lacks production core executable paths'),
    103: ('identified', 'CP-SET-13', 'UI split retains empty trailing item, CIDR validation rejects upstream-accepted trailing comma'),
    105: ('identified', 'CP-SET-13', 'production global IPv6 context remains false'),
    116: ('identified', 'CP-SET-14', 'legacy RoutingIndexId -> IsActive migration missing; ordinary current default setter works'),
    129: ('identified', 'CP-SET-08', 'Mixin options projection exists only in tests; native Mihomo runtime bypasses merge'),
    130: ('identified', 'CP-SET-08', 'Mixin options projection exists only in tests; native Mihomo runtime bypasses merge'),
    136: ('identified', 'CP-SET-10', 'connections table DataColumns fixed; canonical column width/order ignored'),
    138: ('identified', 'CP-SET-09', 'OS consumer exists but sync key ignores changed bypass content'),
    139: ('identified', 'CP-SET-09', 'OS consumer exists but sync key ignores changed local bypass flag'),
    140: ('identified', 'CP-SET-09', 'OS consumer exists but sync key ignores changed advanced protocol'),
    141: ('identified', 'CP-SET-09', 'PAC sync key ignores custom path; nonexistent custom path is created rather than upstream fallback'),
    142: ('identified', 'CP-SET-12', 'OSX/Linux custom proxy script only path validation; execution consumer absent; Windows original does not use'),
    149: ('implemented', 'CP-SET-03', 'backend null-vs-empty fixed; option save failure still silently ignored'),
    153: ('identified', 'CP-SET-06', 'upstream-valid string range 1-3 still rejected by UI/Rust integer-only validation'),
}
for n in [117, 118, 119, 156, 157, 158]:
    issue_override[n] = ('identified', 'CP-SET-10', 'canonical column/geometry children stored but not consumed by actual UI')
for n in [131, 132, 134, 147, 148]:
    issue_override[n] = ('identified', 'CP-SET-03', 'local option changed before saveGroup; unsuccessful persistence not shown/rolled back')
for n in [71, 72, 73, 74, 75]:
    issue_override[n] = ('implemented', 'CP-SET-10', 'main-window consumer exists; independent option window hardcodes light theme and ignores locale/font')
for n in range(176, 181):
    issue_override[n] = ('identified', 'CP-SET-05', 'enable switch not consulted by happyEyeballs emission')

sources = []
for start in ['apps/desktop/lib/features', 'apps/desktop/lib/app', 'crates/application/src', 'crates/config_codegen/src', 'crates/platform/src', 'crates/subscriptions/src', 'crates/updater/src', 'apps/desktop/windows/runner']:
    for path in (ROOT / start).rglob('*'):
        if path.suffix in {'.dart', '.rs', '.cpp', '.h'} and 'frb_generated' not in path.name:
            sources.append((path.relative_to(ROOT).as_posix(), path.read_text(encoding='utf-8').splitlines()))

rows = []
for r in old:
    n = number(r)
    status = 'implemented' if r['row_kind'] == 'container' else r['status']
    issues, note = '', 'production consumer traced statically; no complete UI->Rust->persist->reopen->actual-effect acceptance in this audit'
    if n in issue_override:
        status, issues, note = issue_override[n]
    elif r['row_kind'] == 'container':
        note = 'container retained/persisted; child rows define actual effects; save-and-apply cross-cutting issues CP-SET-01/02/03 apply'
    elif r['status'] == 'blocked':
        status = 'implemented'
        note = 'actual OS consumer/codegen exists, platform effect remains unverified; TUN overall contains confirmed defects, see runtime report'
    property_name = r['path'].split('.')[-1]
    hits = []
    for path, lines in sources:
        for lineno, line in enumerate(lines, 1):
            if property_name in line and not line.strip().startswith(('//', '///')):
                hits.append(f'{path}:{lineno}')
                if len(hits) >= 8:
                    break
        if len(hits) >= 8:
            break
    rows.append({
        'id': r['id'], 'path': r['path'], 'group': r['group'], 'row_kind': r['row_kind'],
        'status': status, 'production_consumer': group_consumers.get(r['group'], 'crates/application/src/engine.rs:1612,1630,1679 settings tree storage'),
        'fresh_reference_candidates': ';'.join(hits),
        'cross_cutting_contract': 'CP-SET-01;CP-SET-02;CP-SET-03;CP-SET-15',
        'current_issue': issues, 'actual_effect_verified_this_audit': 'false',
        'evidence_level': 'current source trace; synthetic fault contracts for CP-SET-01/02/03; pure actual-Rust probe where cited',
        'current_note': note, 'upstream': r['upstream'],
        'historical_ui_ref': r['ui_control'], 'historical_test_ref': r['evidence'],
    })

assert len(rows) == 180 and len({r['id'] for r in rows}) == 180
with (OUT / 'settings-current-180.csv').open('w', encoding='utf-8-sig', newline='') as f:
    writer = csv.DictWriter(f, fieldnames=list(rows[0]))
    writer.writeheader()
    writer.writerows(rows)
summary = {
    'head': 'a7aa0a5b1ecd523bc60d201e0696ff6051cddf26', 'upstream': spec['source_commit'],
    'settings_ids': len(rows), 'unique_ids': len({r['id'] for r in rows}),
    'row_kinds': dict(Counter(r['row_kind'] for r in rows)),
    'static_statuses': dict(Counter(r['status'] for r in rows)),
    'verified_full_effect_this_audit': 0,
    'warning': 'statuses describe static implementation/identified defects, not a completion percentage; historical refs were not rerun individually here; fresh_reference_candidates include UI writers and are not consumer proof',
}
(OUT / 'settings-matrix-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(summary, ensure_ascii=False))
