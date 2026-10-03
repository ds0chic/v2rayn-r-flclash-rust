// FIX-01 / ACT-PROF-007: the node-table "一键生成策略组 → 全部配置项" command
// generates for the subscription group selected when the menu opened. The menu
// closes (MenuItemButton) before the command runs, the command needs no
// selected node, and the real save path persists the policy group.
//
// One page build per file: the locked flutter_tester leaks native resources per
// pumpWidget and can crash after a second heavy MainShell build.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';

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
  testWidgets('empty selection with a valid group generates the all group', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 10, height: 900);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    bridge.saveSubItem(_sub('sub-000', '演示订阅A'));
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('group-filter-sub-000')));
    await tester.pumpAndSettle();
    expect(readState(container).groupSubId, 'sub-000');

    controller.clearSelection();
    await tester.pump();
    expect(readState(container).selected, isEmpty);

    final beforeIds = readState(container).profiles
        .map((p) => p.indexId)
        .toSet();

    // Right-click the empty area under the (10-row) table: no selected node at
    // all, only the current group.
    final rect = tester.getRect(find.byType(ProfilesTable));
    await _rightClick(tester, Offset(rect.right - 24, rect.bottom - 24));
    expect(find.byKey(const ValueKey('ctx-一键生成策略组')), findsOneWidget);

    // The submenu parent closes before the child command runs; the captured
    // command must still hold the group.
    await tester.tap(find.byKey(const ValueKey('ctx-一键生成策略组')));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.tap(find.byKey(const ValueKey('ctx-全部配置项')));
    await tester.pump(const Duration(milliseconds: 350));

    final added = readState(container).profiles
        .where((p) => !beforeIds.contains(p.indexId))
        .toList();
    expect(added, isNotEmpty);
    expect(
      added.every(
        (p) => p.configType == ConfigType.policyGroup && p.subid == 'sub-000',
      ),
      isTrue,
    );
    expect(container.read(uiShellControllerProvider).message, '已生成全部配置项策略组');
  });
}
