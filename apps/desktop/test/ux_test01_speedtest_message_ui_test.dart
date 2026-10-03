// UX-TEST-01: the settled-test message must be visible in the profile toolbar
// (not only in the event log). One page build per file per the locked Flutter
// harness note in test/support/profiles_harness.dart.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('settled speedtest shows a visible message and failure state', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 5);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final controller = container.read(profilesControllerProvider.notifier);
    final id = readState(container).visible.first.id;

    controller.selectRow(id);
    bridge.seedSpeedResult(id, -1, 0, message: 'error.port_conflict');
    controller.emitAction(ProfileAction.realping);
    await tester.pump(const Duration(milliseconds: 300));

    final message = find.byKey(const ValueKey('speedtest-message'));
    expect(message, findsOneWidget);
    expect(tester.widget<Text>(message).data, contains('测速完成'));
    // The node that failed must read 失败 instead of the never-tested '-'.
    expect(find.text('失败'), findsWidgets);
  });
}
