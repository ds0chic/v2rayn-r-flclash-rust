import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

/// UX1-11 tray tail (U-B1 P12): the tray gains a restart-service leaf that
/// reuses the existing ACT-MAIN-035 use case (verified: reload() refreshes and
/// applies the active plan, i.e. start-or-reapply — there is no stop ACT, and
/// the mixed-test ACT-PROF-015 has no tray delegate path, so both are dropped
/// with a report note instead of inventing new actions), plus an over-limit
/// hint entry so the servers submenu explains itself instead of opening empty.
void main() {
  test('restart-service leaf reuses ACT-MAIN-035 (no new action)', () {
    expect(sharedCommandForTrayAction('ACT-TRAY-016'), 'ACT-MAIN-035');
    final items = trayMenuModel(
      currentMode: SysProxyMode.unchanged,
      pacVisible: true,
    );
    final restart = items.firstWhere((i) => i.actionId == 'ACT-TRAY-016');
    expect(restart.label, '重启服务');
    // Sits with the session items, exit stays last with its separator.
    final ids = items.map((i) => i.actionId).toList();
    expect(ids.indexOf('ACT-TRAY-016'), ids.indexOf('ACT-TRAY-012') + 1);
    expect(ids.last, 'ACT-TRAY-013');
    expect(TrayAction.fromMenuItem(restart).sharedCommand, 'ACT-MAIN-035');
  });

  test('over-limit servers submenu shows a hint, not an empty menu', () {
    final items = trayMenuModel(
      currentMode: SysProxyMode.unchanged,
      pacVisible: false,
      nodes: <TraySubEntry>[
        for (var i = 0; i < 21; i++) TraySubEntry(id: '$i', label: 'n$i'),
      ],
      serversLimit: 20,
    );
    final servers = items.firstWhere((i) => i.actionId == 'ACT-TRAY-007');
    expect(servers.children, hasLength(1));
    final hint = servers.children.single;
    expect(hint.id, trayServersOverflowHintId);
    expect(isTrayServersOverflowHint(hint.id), isTrue);
    expect(isTrayServersOverflowHint('3'), isFalse);
    expect(hint.label, contains('21'));
    expect(hint.label, contains('20'));
  });

  test('under-limit servers submenu still passes nodes through', () {
    final items = trayMenuModel(
      currentMode: SysProxyMode.unchanged,
      pacVisible: false,
      nodes: const <TraySubEntry>[
        TraySubEntry(id: 'n1', label: 'HK-1'),
        TraySubEntry(id: 'n2', label: 'US-1'),
      ],
      serversLimit: 10,
    );
    final servers = items.firstWhere((i) => i.actionId == 'ACT-TRAY-007');
    expect(servers.children.map((c) => c.id), <String>['n1', 'n2']);
  });
}
