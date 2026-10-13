import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

import 'support/fake_monitor_bridge.dart';

/// D3b-03: a Clash transport failure (throw, not an error DTO) must surface a
/// visible message and never escape as an unhandled async error.
void main() {
  Future<({ProviderContainer container, FakeMonitorBridge bridge})>
  harness() async {
    final bridge = FakeMonitorBridge(clashApiSupported: true)
      ..throwOnClashReads = true;
    addTearDown(bridge.disposeStreams);
    final container = ProviderContainer(
      overrides: [monitorBridgeProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    return (container: container, bridge: bridge);
  }

  test('clash read failures surface messages and never throw', () async {
    final zoneErrors = <Object>[];
    await runZonedGuarded(() async {
      final h = await harness();
      final controller = h.container.read(monitorControllerProvider.notifier);

      await controller.refreshProxies();
      await controller.refreshConnections();
      await controller.refreshClashMode();
      expect(
        h.container.read(monitorControllerProvider).proxiesMessage,
        isNotNull,
      );
      expect(
        h.container.read(monitorControllerProvider).connectionsMessage,
        isNotNull,
      );
      expect(
        h.container.read(monitorControllerProvider).modeMessage,
        isNotNull,
      );

      expect(await controller.setClashMode('Direct'), isFalse);
      expect(await controller.selectProxy('g', 'n'), isFalse);
      expect(await controller.testProxy('n'), -1);
      await controller.testGroup('g');
      expect(await controller.closeConnection('c1'), isFalse);
      expect(await controller.closeAllConnections(), isFalse);
    }, (Object e, StackTrace s) => zoneErrors.add(e));
    await Future<void>.delayed(Duration.zero);
    expect(
      zoneErrors,
      isEmpty,
      reason: 'no unhandled async errors: $zoneErrors',
    );
  });
}
