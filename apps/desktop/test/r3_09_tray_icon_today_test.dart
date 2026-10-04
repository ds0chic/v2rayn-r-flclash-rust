// R3-09: the tray icon must map each proxy/core/PAC state to a distinct
// resource (with a recognizable fallback when the asset is not packaged), and
// the status-bar "今日" text must use the per-node today counters instead of the
// session-cumulative proxy counter. Synthetic values only; no native library.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/profiles_harness.dart';

void main() {
  group('R3-09a tray icon resource mapping', () {
    test('every status maps to a distinct resource file', () {
      final names = <String>{
        for (final status in TrayIconStatus.values)
          trayIconResourceName(status),
      };
      expect(names, hasLength(TrayIconStatus.values.length));
      expect(trayIconResourceName(TrayIconStatus.normal), 'tray_icon.ico');
      expect(
        trayIconResourceName(TrayIconStatus.coreRunning),
        isNot(trayIconResourceName(TrayIconStatus.normal)),
      );
      expect(
        trayIconResourceName(TrayIconStatus.proxyActive),
        isNot(trayIconResourceName(TrayIconStatus.normal)),
      );
      expect(
        trayIconResourceName(TrayIconStatus.proxyPac),
        isNot(trayIconResourceName(TrayIconStatus.normal)),
      );
    });

    test('a recognizable fallback resource is declared', () {
      expect(trayIconFallbackName, isNotEmpty);
      expect(trayIconFallbackName, endsWith('.ico'));
    });
  });

  group('R3-09b today traffic semantics', () {
    test(
      'today aggregates node ServerStatItem values, not the session counter',
      () {
        final state = MonitorState(
          nodes: const <m.NodeTrafficDto>[
            m.NodeTrafficDto(
              indexId: 'a',
              totalUp: 1,
              totalDown: 2,
              todayUp: 100,
              todayDown: 200,
              dateNow: 1,
            ),
            m.NodeTrafficDto(
              indexId: 'b',
              totalUp: 3,
              totalDown: 4,
              todayUp: 5,
              todayDown: 6,
              dateNow: 1,
            ),
          ],
          proxyUp: BigInt.from(999999),
          proxyDown: BigInt.from(888888),
        );
        expect(state.hasTodayNodes, isTrue);
        expect(state.todayUp, BigInt.from(105));
        expect(state.todayDown, BigInt.from(206));
      },
    );

    test('no node rows => no today value (no session / disabled)', () {
      final state = MonitorState(
        proxyUp: BigInt.from(123),
        proxyDown: BigInt.from(456),
      );
      expect(state.hasTodayNodes, isFalse);
      expect(state.todayUp, BigInt.zero);
      expect(state.todayDown, BigInt.zero);
    });

    testWidgets('status bar shows the node today sum, not session cumulative', (
      tester,
    ) async {
      final monitor = FakeMonitorBridge();
      addTearDown(monitor.disposeStreams);
      final container = await pumpApp(tester, rows: 10, monitor: monitor);
      container
          .read(monitorControllerProvider.notifier)
          .configure(
            core: 2,
            statePort: 11809,
            statePort2: 0,
            enableStatistics: true,
            displayRealTimeSpeed: true,
            refreshIntervalMs: 1000,
          );
      monitor.emitTraffic(
        proxyUp: BigInt.from(999999),
        proxyDown: BigInt.from(888888),
        nodes: <m.NodeTrafficDto>[
          m.NodeTrafficDto(
            indexId: 'a',
            totalUp: 0,
            totalDown: 0,
            todayUp: 1024,
            todayDown: 2048,
            dateNow: 1,
          ),
        ],
      );
      await tester.pump();

      final text = tester
          .widget<Text>(find.byKey(const ValueKey('status-today-traffic')))
          .data!;
      expect(text.contains('1.00 KB'), isTrue);
      expect(text.contains('2.00 KB'), isTrue);
      expect(text.contains('999999'), isFalse);
    });
  });
}
