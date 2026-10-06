import json
from pathlib import Path

import yaml

root = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
upstream = root / 'work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN'
baseline = 'a7aa0a5b1ecd523bc60d201e0696ff6051cddf26'
features = yaml.safe_load((root / 'compat/features.yaml').read_text(encoding='utf-8'))['features']
actions = yaml.safe_load((root / 'compat/actions.yaml').read_text(encoding='utf-8'))['items']
layouts = yaml.safe_load((root / 'compat/layouts.yaml').read_text(encoding='utf-8'))
fields = yaml.safe_load((root / 'compat/fields.settings.yaml').read_text(encoding='utf-8'))['items']
entities = yaml.safe_load((root / 'compat/fields.entities.yaml').read_text(encoding='utf-8'))['items']

scope = {
    'baseline': baseline,
    'upstream_commit': '7d6a967c18c697f28dc6917122ed3a4993fcf336',
    'meaning': 'Referenced inventory scope only; does not assert that each ID has passed a complete user flow or real desktop verification.',
    'status': 'identified',
    'selected_synthetic_contracts': {'executed': 7, 'correct_expectation_failed': 7, 'initial_suite_load_and_fixture_errors_counted_as_product_failures': 0, 'all_referenced_ids_executed': False},
    'features': [x['id'] for x in features if x.get('family') in {'profile', 'subscription', 'import-export', 'routing', 'dns', 'monitor', 'test', 'desktop'}] + ['F-APP-001', 'F-APP-002', 'F-APP-003'],
    'actions': [x['id'] for x in actions if x.get('domain') in {'profiles', 'subscriptions', 'routing', 'dns', 'testing', 'hotkey', 'statusbar'}],
    'layouts': [x['id'] for x in layouts['main_layouts']] + [x['id'] for x in layouts['items'] if str(x['id']).startswith(('LAY-MAIN-', 'LAY-PROFILES-', 'LAY-CLASH', 'LAY-MSG-', 'LAY-STATUSBAR-', 'LAY-ROUTING', 'LAY-DNS', 'LAY-THEME'))],
    'settings_fields': [x['id'] for x in fields if any(v in str(x.get('source_symbol', '')) for v in ('Config.SubIndexId', 'UIItem.', 'MsgUIItem.', 'ClashUIItem.', 'WindowSizeItem.'))],
    'entity_fields': [x['id'] for x in entities if x.get('source_symbol') in {'ProfileItem.IndexId', 'ProfileItem.Subid', 'ProfileItem.IsSub', 'SubItem.Id', 'SubItem.ConvertTarget', 'SubItem.PrevProfile', 'SubItem.NextProfile', 'SubItem.PreSocksPort'}],
    'unverified': ['native HWND/event parity', 'real FRB/SQLite reopen flow', 'remote TLS/protocol/user traffic', 'OS hotkeys/tray DPI', 'release latency at 100k+ profiles/10k connections', 'all individual 158 entity field chains'],
}
(out / 'coverage.json').write_text(json.dumps(scope, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

items = [
    ('UI-01', 1, 'route read failure/invalid snapshot becomes writable empty data', [('apps/desktop/lib/features/routing/routing_actions.dart', 53, 64), ('apps/desktop/lib/features/routing/routing_windows.dart', 1570, 1585), ('apps/desktop/lib/features/routing/routing_windows.dart', 1743, 1749)], [('ServiceLib/ViewModels/RoutingSettingViewModel.cs', 84, 105)], ['native_reply_loss_test.dart']),
    ('UI-02', 1, 'partial routing commits plus stale whole draft can resurrect/remove committed rows', [('apps/desktop/lib/features/routing/routing_windows.dart', 2100, 2115), ('apps/desktop/lib/features/routing/routing_windows.dart', 2190, 2199), ('apps/desktop/lib/features/routing/routing_actions.dart', 112, 132)], [('ServiceLib/ViewModels/RoutingSettingViewModel.cs', 110, 115), ('ServiceLib/ViewModels/RoutingSettingViewModel.cs', 153, 163)], ['routing_partial_commit_test.dart']),
    ('UI-03', 1, 'independent settings/routing save can wait forever after lost reply', [('apps/desktop/lib/features/settings/settings_window_host.dart', 157, 170), ('apps/desktop/lib/features/routing/routing_windows.dart', 1753, 1786), ('apps/desktop/windows/runner/option_window_host.cpp', 71, 77), ('apps/desktop/windows/runner/routing_window_host.cpp', 72, 78)], [], ['native_reply_loss_test.dart']),
    ('UI-04', 2, 'current subscription group is neither persisted nor restored', [('apps/desktop/lib/features/profiles/profiles_controller.dart', 365, 384), ('apps/desktop/lib/features/profiles/profiles_controller.dart', 866, 874)], [('ServiceLib/ViewModels/ProfilesViewModel.cs', 334, 338)], ['group_reopen_test.dart']),
    ('UI-05', 2, 'bottom bar depends on horizontal scrolling at upstream minimum viewport', [('apps/desktop/lib/app/shell/status_bar_view.dart', 298, 316)], [('v2rayN/Views/MainWindow.xaml', 14, 16), ('v2rayN/Views/StatusBarView.xaml', 22, 103)], ['status_viewport_test.dart']),
    ('UI-06', 2, 'actual running node identity and original status test input are missing', [('apps/desktop/lib/features/runtime/runtime_bridge.dart', 40, 77), ('apps/desktop/lib/app/shell/status_bar_view.dart', 163, 170)], [('ServiceLib/ViewModels/StatusBarViewModel.cs', 270, 271), ('v2rayN/Views/StatusBarView.xaml.cs', 94, 96)], []),
    ('UI-07', 2, 'add/edit subscription buttons both open total list', [('apps/desktop/lib/features/profiles/profiles_page.dart', 118, 128)], [('ServiceLib/ViewModels/ProfilesViewModel.cs', 862, 881)], []),
    ('UI-08', 2, 'independent windows do not inherit parent theme/font/locale', [('apps/desktop/lib/features/settings/option_setting_window_entry.dart', 19, 23), ('apps/desktop/lib/features/routing/routing_windows.dart', 1844, 1848)], [], []),
    ('UI-09', 2, 'connection table lacks column layout/menus and row virtualization', [('apps/desktop/lib/features/monitor/connections_view.dart', 228, 274)], [('v2rayN/Views/ClashConnectionsView.xaml', 64, 79), ('v2rayN/Views/ClashConnectionsView.xaml.cs', 70, 125)], []),
    ('UI-10', 2, 'old platform result shadows newer business feedback', [('apps/desktop/lib/app/shell/status_bar_view.dart', 276, 287)], [], []),
    ('UI-11', 1, 'All/ungrouped import uses synchronous per-row saves and production preview materializes files despite generated batch/preview APIs', [('apps/desktop/lib/features/subs/import_persistence.dart', 88, 128), ('apps/desktop/lib/features/subs/import_persistence.dart', 47, 57), ('apps/desktop/lib/bridge/bridge_port.dart', 596, 600), ('crates/bridge_api/src/api/subs.rs', 438, 466), ('crates/bridge_api/src/api/subs.rs', 602, 603)], [('ServiceLib/Handler/ConfigHandler.cs', 1697, 1702)], ['ungrouped_import_failure_test.dart']),
]
findings = []
for ident, priority, title, current, original, tests in items:
    findings.append({
        'id': ident, 'priority': priority, 'status': 'identified', 'title': title,
        'source_evidence': [{'path': (root / p).as_posix(), 'start': a, 'end': b} for p, a, b in current],
        'upstream_evidence': [{'path': (upstream / p).as_posix(), 'start': a, 'end': b} for p, a, b in original],
        'synthetic_tests': [(out / p).as_posix() for p in tests],
        'real_desktop': '未验证',
    })
(out / 'findings.json').write_text(json.dumps({'baseline': baseline, 'findings': findings}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'{len(findings)} current findings; referenced scope: ' + ', '.join(f'{k}={len(scope[k])}' for k in ['features', 'actions', 'layouts', 'settings_fields', 'entity_fields']))
