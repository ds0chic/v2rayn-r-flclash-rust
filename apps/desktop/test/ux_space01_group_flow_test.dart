// UX-SPACE-01: top group chips drive the required node-table flow
// (select A -> filter -> clear -> switch B -> back to all). One page build per
// process because the locked flutter_tester leaks native resources.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

c.SubItemDto _sub(String id, String remarks, int sort) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/$id',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: sort,
  autoUpdateInterval: 0,
  updateTime: 0,
);

void main() {
  testWidgets('top group chips filter, clear and return to all', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 200, height: 800);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveSubItem(_sub('sub-000', '演示订阅A', 1));
    bridge.saveSubItem(_sub('sub-001', '演示订阅B', 2));
    container.read(profilesControllerProvider.notifier).reload();
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('group-filter-all')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-filter-sub-000')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-filter-sub-001')), findsOneWidget);

    // Select A.
    await tester.tap(find.byKey(const ValueKey('group-filter-sub-000')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, 'sub-000');
    expect(readState(container).visible.length, 100);

    // Filter inside A, then clear; the group selection must survive.
    final filter = find.byKey(const ValueKey('filter-field'));
    await tester.enterText(filter, 'Synthetic-00001');
    await tester.pump();
    final filtered = readState(container).visible.length;
    expect(filtered, lessThan(100));
    expect(readState(container).groupSubId, 'sub-000');

    await tester.enterText(filter, '');
    await tester.pump();
    expect(readState(container).visible.length, 100);
    expect(readState(container).groupSubId, 'sub-000');

    // Switch to B, then back to all.
    await tester.tap(find.byKey(const ValueKey('group-filter-sub-001')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, 'sub-001');
    expect(
      readState(container).visible.every((r) => r.subRemarks == 'sub-001'),
      isTrue,
    );

    await tester.tap(find.byKey(const ValueKey('group-filter-all')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, isNull);
    expect(readState(container).visible.length, 200);
  });
}
