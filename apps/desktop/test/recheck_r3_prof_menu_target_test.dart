// R3-PROF-01/02 widget regression: the context menu acts on the immutable
// row/selection captured when it opened, not the live selection.
//
// One `pumpApp` build covers three sub-scenarios so the locked flutter_tester
// resource leak (see support/profiles_harness.dart) is not multiplied.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('context menu commands target the captured main row', (
    tester,
  ) async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          SystemChannels.platform,
          (call) async => null,
        );
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null),
    );

    final container = await pumpApp(tester, rows: 20);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();

    // A) multi-select, right-click one row, "设为活动" acts on that row.
    await tapRow(tester, ValueKey('cell-${ids[0]}-Remarks'));
    await pressWithCtrlTap(tester, ValueKey('cell-${ids[1]}-Remarks'));
    await tester.tapAt(
      tester.getCenter(find.byKey(ValueKey('cell-${ids[0]}-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();
    expect(container.read(profilesControllerProvider).selected, <String>{
      ids[0],
      ids[1],
    });
    await tester.tap(find.byKey(const ValueKey('ctx-设为活动')));
    await tester.pumpAndSettle();
    expect(
      bridge.getActiveProfile(),
      ids[0],
      reason: 'the row under the pointer is the command target',
    );
    expect(container.read(profilesControllerProvider).selected, <String>{
      ids[0],
      ids[1],
    }, reason: 'batch selection is not collapsed by a single-object command');

    // B) full-config export uses the captured target after the selection drifts.
    await tapRow(tester, ValueKey('cell-${ids[3]}-Remarks'));
    await tester.tapAt(
      tester.getCenter(find.byKey(ValueKey('cell-${ids[3]}-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();
    controller.selectRow(ids[4]); // drift the live selection
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('ctx-导出')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('ctx-导出所选完整配置至剪贴板')));
    await tester.pumpAndSettle();
    expect(
      bridge.exportedClientConfigIds.last,
      ids[3],
      reason: 'export reads the immutable captured target, not live selection',
    );

    // C) a filter that hides the captured row refuses the command.
    controller.clearSelection();
    await tester.pump();
    await tapRow(tester, ValueKey('cell-${ids[0]}-Remarks'));
    await tester.tapAt(
      tester.getCenter(find.byKey(ValueKey('cell-${ids[0]}-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();
    final activeBefore = bridge.getActiveProfile();
    controller.setFilter('Synthetic-00005'); // hides ids[0]
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('ctx-设为活动')));
    await tester.pumpAndSettle();
    expect(
      bridge.getActiveProfile(),
      activeBefore,
      reason: 'hidden captured target must be refused',
    );
  });
}
