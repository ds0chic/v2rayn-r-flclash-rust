// T10: groups panel lists subscriptions, filters the table, gates gen keys.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('groups panel filters by subscription', (tester) async {
    final container = await pumpApp(tester, rows: 200);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    // Seed one subscription matching the synthetic `sub-000` rows.
    bridge.saveSubItem(
      const c.SubItemDto(
        id: 'sub-000',
        remarks: 'demo',
        url: 'https://example.com/sub',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 1,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    container.read(profilesControllerProvider.notifier).reload();
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('groups-title')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-filter-all')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-filter-sub-000')), findsOneWidget);

    final before = readState(container).visible.length;
    await tester.tap(find.byKey(const ValueKey('group-filter-sub-000')));
    await tester.pumpAndSettle();
    final after = readState(container).visible.length;
    expect(readState(container).groupSubId, 'sub-000');
    expect(after, lessThan(before));
    expect(after, greaterThan(0));

    // Back to all.
    await tester.tap(find.byKey(const ValueKey('group-filter-all')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, isNull);
    expect(readState(container).visible.length, before);
  });

  testWidgets('group filter compares sub ids, not display remarks', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 200);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    // Trap: sub A's display remarks collide with sub B's id. Filtering by
    // A must still resolve by id (`setGroupSubId(sub.id)`), never by the
    // display name.
    bridge.saveSubItem(
      const c.SubItemDto(
        id: 'sub-000',
        remarks: 'sub-001',
        url: 'https://example.com/a',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 1,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    bridge.saveSubItem(
      const c.SubItemDto(
        id: 'sub-001',
        remarks: 'other',
        url: 'https://example.com/b',
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 2,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    container.read(profilesControllerProvider.notifier).reload();
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('group-filter-sub-000')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, 'sub-000');
    // rows 0..99 carry sub-000; rows 100..199 carry sub-001.
    expect(readState(container).visible.length, 100);
    for (final row in readState(container).visible) {
      expect(row.subRemarks, 'sub-000');
    }
  });
}
