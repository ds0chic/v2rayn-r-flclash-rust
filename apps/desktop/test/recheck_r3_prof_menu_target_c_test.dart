// R3-PROF-02 widget regression (scenario C): a filter that hides the captured
// row refuses the command instead of acting on an invisible object.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('hidden captured target refuses the command', (tester) async {
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
