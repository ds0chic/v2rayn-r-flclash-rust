// FIX-01 / ACT-PROF-007: a group switched in while the context menu stays open
// must not retarget an already-captured generate command. The command keeps
// operating on the group it was opened on (immutable CommandContext), never the
// live selection.
//
// One page build per file: the locked flutter_tester leaks native resources per
// pumpWidget and can crash after a second heavy MainShell build.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

c.SubItemDto _sub(String id, String remarks) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/$id',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

Future<void> _rightClick(WidgetTester tester, Offset point) async {
  await tester.tapAt(
    point,
    kind: PointerDeviceKind.mouse,
    buttons: kSecondaryButton,
  );
  await tester.pump(const Duration(milliseconds: 350));
}

void main() {
  testWidgets('a group switch while the menu is open does not retarget it', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 200, height: 900);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveSubItem(_sub('sub-000', '演示订阅A'));
    bridge.saveSubItem(_sub('sub-001', '演示订阅B'));
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('group-filter-sub-000')));
    await tester.pumpAndSettle();
    final row = readState(container).visible.first;
    await tester.tap(find.byKey(ValueKey('cell-${row.id}-Remarks')));
    await tester.pump(const Duration(milliseconds: 400));

    await _rightClick(
      tester,
      tester.getCenter(find.byKey(ValueKey('cell-${row.id}-Remarks'))),
    );
    expect(find.byKey(const ValueKey('ctx-一键生成策略组')), findsOneWidget);

    final beforeIds = readState(container).profiles
        .map((p) => p.indexId)
        .toSet();

    // Switch the visible group while the menu stays open; the captured command
    // must keep targeting the group it was opened on.
    controller.setGroupSubId('sub-001');
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, 'sub-001');

    await tester.tap(find.byKey(const ValueKey('ctx-一键生成策略组')));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.tap(find.byKey(const ValueKey('ctx-全部配置项')));
    await tester.pump(const Duration(milliseconds: 350));

    final added = readState(container).profiles
        .where((p) => !beforeIds.contains(p.indexId))
        .toList();
    expect(added.where((p) => p.subid == 'sub-000'), isNotEmpty);
    expect(added.where((p) => p.subid == 'sub-001'), isEmpty);
    expect(container.read(uiShellControllerProvider).message, '已生成全部配置项策略组');
  });
}
