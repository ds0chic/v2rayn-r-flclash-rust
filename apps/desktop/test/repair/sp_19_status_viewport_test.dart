// SP-19: 800px-wide main-window status bar shows every key control in the
// initial viewport at 100/125/150/200% DPI (no horizontal scroll to find the
// rates), keeps the original font sizes readable, and ellipsizes long labels
// instead of pushing controls off screen.
//
// Correct contract (CP-14 / UI-05):
// - left/right partitions bounded + flexible middle; long labels ellipsis with
//   tooltip, never a horizontal scroller as the substitute for same-screen.
// - fonts/spacing follow the original tokens; nothing shrinks to unreadable.
// - every key control is visible AND tappable without scrolling first.
//
// Red first: the current strip is one horizontal SingleChildScrollView, so the
// no-scroller assertion and the same-screen geometry fail before the fix.
// One pumpWidget per file: the locked flutter_tester segfaults when several
// page builds share one process (see support profiles_harness.dart note).
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/counting_runtime_bridge.dart';
import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';

const _scales = <double>[1.0, 1.25, 1.5, 2.0];

/// Key controls that must share the initial 800px viewport (audit UI-05:
// menu-adjacent selectors, running summary, both rate lines, ports, TUN, today,
// details entry). Keys follow the existing status_bar_view.dart contracts.
const _sameScreenKeys = <String>[
  'status-inbound',
  'tun-toggle',
  'system-proxy-selector',
  'routing-mode-selector',
  'routing-selector',
  'running-node',
  'running-info',
  'running-availability-test',
  'status-proxy-speed',
  'status-direct-speed',
  'status-today-traffic',
  'status-details',
];

void main() {
  testWidgets('status bar fits 800px at 100-200% DPI without scrolling', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final monitor = FakeMonitorBridge();
    addTearDown(monitor.disposeStreams);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 3)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(3),
        platformBridgeProvider.overrideWithValue(
          FakePlatformBridge(initialMode: SysProxyMode.unchanged),
        ),
        runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
        monitorBridgeProvider.overrideWithValue(monitor),
      ],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: StatusBarView())),
      ),
    );
    await tester.pump(const Duration(milliseconds: 30));

    for (final scale in _scales) {
      final label = '${(scale * 100).round()}%';
      // Logical 800px wide (upstream minimum) at this DPI scale.
      tester.view.devicePixelRatio = scale;
      tester.view.physicalSize = Size(800 * scale, 800 * scale);
      await tester.pump(const Duration(milliseconds: 30));
      expect(
        tester.takeException(),
        isNull,
        reason: 'layout exception at 800px @$label',
      );

      // No horizontal scroller may remain in the bar: same-screen is required,
      // scrolling to reach the rates is not a substitute (UI-05 strict bound).
      final scrollers = find.descendant(
        of: find.byType(StatusBarView),
        matching: find.byWidgetPredicate(
          (w) =>
              w is SingleChildScrollView &&
              w.scrollDirection == Axis.horizontal,
        ),
      );
      expect(
        scrollers,
        findsNothing,
        reason:
            'status bar must not rely on horizontal scroll at 800px @$label',
      );

      // Every key control is inside the initial viewport (no pre-scroll).
      for (final key in _sameScreenKeys) {
        final finder = find.byKey(ValueKey<String>(key));
        expect(finder, findsWidgets, reason: 'missing $key at 800px @$label');
        final rect = tester.getRect(finder.first);
        expect(
          rect.left,
          greaterThanOrEqualTo(-0.5),
          reason: '$key clipped left at 800px @$label: $rect',
        );
        expect(
          rect.right,
          lessThanOrEqualTo(800.5),
          reason: '$key clipped right at 800px @$label: $rect',
        );
      }

      // Selectors/entries stay tappable in place (no scroll-into-view first).
      for (final key in <String>[
        'system-proxy-selector',
        'routing-mode-selector',
        'routing-selector',
        'running-availability-test',
        'status-details',
        'tun-toggle',
      ]) {
        expect(
          find.byKey(ValueKey<String>(key)).hitTestable(),
          findsWidgets,
          reason: '$key not tappable at 800px @$label',
        );
      }

      // Readability floor: the original compact sizes stay, never shrunk to
      // unreadable to force the fit (11px is the smallest upstream token).
      for (final key in <String>[
        'status-inbound',
        'status-proxy-speed',
        'status-direct-speed',
        'running-node',
        'running-info',
      ]) {
        final text = tester.widget<Text>(
          find.byKey(ValueKey<String>(key)).first,
        );
        final size = text.style?.fontSize ?? 0;
        expect(
          size,
          greaterThanOrEqualTo(11),
          reason: '$key font shrunk to $size at 800px @$label',
        );
        expect(
          text.overflow,
          TextOverflow.ellipsis,
          reason: '$key must ellipsize long labels at 800px @$label',
        );
      }
    }
  });
}
