// Real Windows Flutter/FRB/SQLite regression for two normal profile flows.
// Use a fresh V2RAYN_R_DATA_DIR and synthetic values. No core, proxy or port
// is started, and no subscription URL is fetched.
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 500));

Future<void> _waitFor(WidgetTester tester, bool Function() ready) async {
  for (var i = 0; i < 100 && !ready(); i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  expect(
    ready(),
    isTrue,
    reason: 'Windows window did not reach expected state',
  );
}

Future<void> _menu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await _settle(tester);
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await _settle(tester);
}

c.SubItemDto _group(String remarks) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: '',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'new node belongs to shown group and switching clears selection',
    (tester) async {
      final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
      final evidenceDir =
          Platform.environment['V2RAYN_R_PARITY_RECHECK_EVIDENCE'];
      expect(dataDir, isNotNull);
      expect(evidenceDir, isNotNull);
      await Directory(evidenceDir!).create(recursive: true);
      final checks = <Map<String, Object?>>[];
      void record(String name, bool passed, Map<String, Object?> actual) {
        checks.add({'name': name, 'passed': passed, 'actual': actual});
        File('$evidenceDir/observations.json').writeAsStringSync(
          const JsonEncoder.withIndent('  ')
              .convert({'dataDirIsolated': dataDir != null, 'checks': checks}),
        );
      }

      RustBridgeInit.configure(RustLib.init);
      await RustBridgeInit.init();
      runApp(const ProviderScope(child: V2rayNRApp()));
      await _waitFor(
        tester,
        () => find.byKey(const ValueKey('menu-配置项')).evaluate().isNotEmpty,
      );
      final container = ProviderScope.containerOf(
        tester.element(find.byType(V2rayNRApp)),
      );
      final subs = container.read(subsControllerProvider.notifier);
      final groupA = subs.save(_group('合成-A')).item!;
      final groupB = subs.save(_group('合成-B')).item!;
      await _settle(tester);
      await tester.tap(find.byKey(ValueKey('group-filter-${groupA.id}')));
      await _settle(tester);
      expect(container.read(profilesControllerProvider).groupSubId, groupA.id);

      await _menu(tester, '配置项', '添加 [TUIC]');
      await tester.enterText(
        find.byKey(const ValueKey('field-remarks')),
        'synthetic-group-test',
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-address')),
        '192.0.2.77',
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-username')),
        '11111111-2222-3333-4444-555555555555',
      );
      await tester.enterText(
        find.byKey(const ValueKey('field-password')),
        'synthetic-password',
      );
      await tester.tap(find.byKey(const ValueKey('editor-save')));
      await _settle(tester);
      final profiles = container.read(profilesControllerProvider.notifier);
      final created = container
          .read(profilesControllerProvider)
          .profiles
          .singleWhere((p) => p.remarks == 'synthetic-group-test');
      final inheritedGroup = created.subid == groupA.id;
      record('new-node-inherits-current-group', inheritedGroup, {
        'currentGroupId': groupA.id,
        'savedSubId': created.subid,
        'visibleIds': container
            .read(profilesControllerProvider)
            .visible
            .map((r) => r.id)
            .toList(),
      });

      expect(
        profiles.moveProfilesToGroup([created.indexId], groupB.id),
        isTrue,
      );
      await tester.tap(find.byKey(ValueKey('group-filter-${groupB.id}')));
      await _settle(tester);
      await tester.tap(find.byKey(ValueKey('cell-${created.indexId}-Remarks')));
      await tester.pump(const Duration(milliseconds: 400));
      expect(
        container
            .read(profilesControllerProvider)
            .selected
            .contains(created.indexId),
        isTrue,
      );
      await tester.tap(find.byKey(ValueKey('group-filter-${groupA.id}')));
      await _settle(tester);
      final afterSwitch = container.read(profilesControllerProvider);
      final cleared = !afterSwitch.selected.contains(created.indexId);
      record('switch-group-clears-hidden-selection', cleared, {
        'currentGroupId': afterSwitch.groupSubId,
        'selectedIds': afterSwitch.selected.toList(),
        'visibleIds': afterSwitch.visible.map((r) => r.id).toList(),
      });

      expect(
        checks.where((c) => c['passed'] != true),
        isEmpty,
        reason: 'Frozen WPF profile flow gaps; see observations.json',
      );
    },
  );
}
