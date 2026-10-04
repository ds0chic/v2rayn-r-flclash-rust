// R3-PROF-01 widget regression (scenario A): a multi-selection right-click
// "set active" acts on the row under the pointer without collapsing the batch.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('right-click set-active targets the captured main row', (
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
    final ids = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();

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
  });
}
