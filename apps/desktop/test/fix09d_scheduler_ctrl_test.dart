// FIX-09D: the normal-launch scheduler hook must start the periodic updater
// exactly once (idempotent) and the shutdown hook must stop it so no timer is
// left behind. Neither hook is armed by any test environment variable.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

/// Models the real scheduler's running flag so idempotency/cleanup can be
/// asserted independently of the synthetic bridge's no-op default.
class SchedulerBridge extends SeededSubsBridge {
  bool running = false;
  int starts = 0;
  int stops = 0;

  @override
  c.SimpleResult startSubScheduler() {
    starts++;
    running = true;
    return const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult stopSubScheduler() {
    stops++;
    running = false;
    return const c.SimpleResult(ok: true);
  }

  @override
  bool subSchedulerRunning() => running;
}

void main() {
  test('startScheduler is idempotent and stopScheduler leaves no timer', () {
    final bridge = SchedulerBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(subsControllerProvider.notifier);

    controller.startScheduler();
    controller.startScheduler();
    expect(bridge.starts, 1, reason: 'second start must be a no-op');
    expect(bridge.running, isTrue);

    controller.stopScheduler();
    controller.stopScheduler();
    expect(bridge.stops, 1, reason: 'stop must be idempotent');
    expect(bridge.running, isFalse);
    expect(
      container.read(subsControllerProvider).status!.message,
      contains('停止'),
    );
  });
}
