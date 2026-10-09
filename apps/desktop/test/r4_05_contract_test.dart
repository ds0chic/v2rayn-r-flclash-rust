// R4-05 统一退出与快速重开 contract tests.
//
// These cover the bounded real-exit sequence (stop -> drain/flush -> platform
// restore), failure isolation and budget/timeout bounding, the "hide is not a
// stop" close contract, and the self-update hand-off exit. Real OS effects
// (system proxy/PAC, core stop, host exit) require an authorized isolated
// machine and are not asserted here; the runtime/host side is covered by the
// Rust workspace tests.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// Build one recorded shutdown step, optionally throwing / delaying / hanging.
MapEntry<String, ShutdownStep> _step(
  String name,
  List<String> log, {
  bool throws = false,
  Duration delay = Duration.zero,
  Completer<void>? gate,
}) => MapEntry(name, () async {
  if (gate != null) {
    await gate.future;
    return;
  }
  if (delay > Duration.zero) {
    await Future<void>.delayed(delay);
  }
  if (throws) {
    throw StateError('boom-$name');
  }
  log.add(name);
});

/// Minimal lifecycle double: records the hand-off exit, no plugins.
class _FakeLifecycle implements DesktopLifecycle {
  int updateExits = 0;
  int plainExits = 0;
  bool removed = false;

  @override
  Future<void> hideToTray() async {}

  @override
  Future<void> exitApp() async {
    plainExits++;
  }

  @override
  Future<bool> exitForUpdate() async {
    updateExits++;
    return true;
  }

  @override
  void removeListener() {
    removed = true;
  }
}

void main() {
  group('bounded real-exit sequence', () {
    test('runs stop -> flush -> platform restore in contract order', () async {
      final log = <String>[];
      final report = await runBoundedShutdown(<MapEntry<String, ShutdownStep>>[
        _step('stop_runtime', log),
        _step('flush_stats', log),
        _step('restore_platform', log),
        _step('unregister_hotkeys', log),
      ]);

      expect(report.ok, isTrue);
      expect(log, <String>[
        'stop_runtime',
        'flush_stats',
        'restore_platform',
        'unregister_hotkeys',
      ]);
      expect(report.completed, log);
      expect(report.failures, isEmpty);
    });

    test('a failed stop does not skip platform restore', () async {
      final log = <String>[];
      final report = await runBoundedShutdown(<MapEntry<String, ShutdownStep>>[
        _step('stop_runtime', log, throws: true),
        _step('flush_stats', log),
        _step('restore_platform', log),
      ]);

      expect(report.ok, isFalse);
      expect(report.failures.keys, contains('stop_runtime'));
      // Ownership restore still ran, so the proxy is not left armed.
      expect(log, <String>['flush_stats', 'restore_platform']);
    });

    test('a hanging step is bounded by the per-step timeout', () async {
      final gate = Completer<void>();
      final watch = Stopwatch()..start();
      final report = await runBoundedShutdown(<MapEntry<String, ShutdownStep>>[
        _step('stop_runtime', <String>[], gate: gate),
      ], stepTimeout: const Duration(milliseconds: 40));
      watch.stop();

      expect(report.failures['stop_runtime'], contains('timeout'));
      expect(watch.elapsedMilliseconds, lessThan(2000));
    });

    test('the total budget skips remaining steps instead of hanging', () async {
      final log = <String>[];
      final report = await runBoundedShutdown(
        <MapEntry<String, ShutdownStep>>[
          _step('stop_runtime', log, delay: const Duration(milliseconds: 60)),
          _step('restore_platform', log),
        ],
        stepTimeout: const Duration(seconds: 1),
        totalBudget: const Duration(milliseconds: 40),
      );

      expect(log, <String>['stop_runtime']);
      expect(report.failures['restore_platform'], contains('budget'));
    });
  });

  group('hide is not a stop', () {
    test('Windows close always hides; opt-in hides elsewhere', () {
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: true,
          hide2TrayWhenClose: false,
        ),
        isTrue,
      );
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: false,
          hide2TrayWhenClose: true,
        ),
        isTrue,
      );
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: false,
          hide2TrayWhenClose: false,
        ),
        isFalse,
      );
    });
  });

  group('self-update hand-off', () {
    test('prefers the bounded desktop lifecycle when present', () async {
      final fake = _FakeLifecycle();
      final holder = DesktopIntegrationHolder()..value = fake;
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
          desktopIntegrationProvider.overrideWithValue(holder),
        ],
      );
      addTearDown(container.dispose);

      final controller = container.read(updateControllerProvider.notifier);
      await controller.handoffExit();

      expect(fake.updateExits, 1);
      expect(fake.plainExits, 0);
    });

    test('falls back to the injected exit without an integration', () async {
      var exits = 0;
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
          updateControllerProvider.overrideWith(
            () => UpdateController(exitApp: () => exits++),
          ),
        ],
      );
      addTearDown(container.dispose);

      final controller = container.read(updateControllerProvider.notifier);
      await controller.handoffExit();

      expect(exits, 1);
    });
  });
}
