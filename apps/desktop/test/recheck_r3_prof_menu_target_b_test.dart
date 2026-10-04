// R3-PROF-02 widget regression (scenario B): the full-config export uses the
// immutable captured target even after the live selection drifts.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('full-config export reads the captured target after drift', (
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
  });
}
