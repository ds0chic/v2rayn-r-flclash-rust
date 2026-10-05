// R4-34 contract tests: the persisted resource sources + global
// `GuiItem.AutoUpdateInterval` form a schedulable plan (D32), and the single
// scheduler entry that a normal launch starts/stop remains idempotent with no
// residual timer (FIX-09D semantics must not regress).
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/settings/resource_auto_update.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

/// Models the real scheduler running flag so idempotency/cleanup can be
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

Map<String, dynamic> settingsWith({
  int interval = 4,
  String? geo = 'https://mirror.example/{0}.dat',
  String? srs = 'https://mirror.example/rule-set/{1}.srs',
  String? subConvert,
  String? route,
}) => <String, dynamic>{
  'GuiItem': <String, dynamic>{'AutoUpdateInterval': interval},
  'ConstItem': <String, dynamic>{
    'GeoSourceUrl': geo,
    'SrsSourceUrl': srs,
    'SubConvertUrl': subConvert,
    'RouteRulesTemplateSourceUrl': route,
  },
};

void main() {
  group('resource plan (D32)', () {
    test('a saved source + global period is schedulable', () {
      final plan = resourceAutoUpdatePlan(settingsWith(interval: 4));
      expect(plan.enabled, isTrue);
      expect(plan.sources, hasLength(2));
      expect(plan.active, isTrue);
      expect(resourcePassDue(plan, 0), isFalse, reason: 'startup tick');
      expect(resourcePassDue(plan, 4), isTrue);
      expect(resourcePassDue(plan, 8), isTrue);
      expect(resourcePassDue(plan, 5), isFalse);
    });

    test('interval 0 disables the task even with sources configured', () {
      final plan = resourceAutoUpdatePlan(settingsWith(interval: 0));
      expect(plan.enabled, isFalse);
      expect(plan.active, isFalse);
      expect(resourcePassDue(plan, 1), isFalse);
    });

    test('a configured route template alone is a resource source', () {
      final plan = resourceAutoUpdatePlan(
        settingsWith(
          geo: null,
          srs: null,
          route: 'https://mirror.example/template.json',
        ),
      );
      expect(plan.hasSources, isTrue);
      expect(plan.sources.single, 'https://mirror.example/template.json');
    });

    test('blank sources are ignored, so nothing downloads silently', () {
      final plan = resourceAutoUpdatePlan(settingsWith(geo: '  ', srs: null));
      expect(plan.hasSources, isFalse);
      expect(plan.active, isFalse);
    });
  });

  group('scheduler entry (FIX-09D must not regress)', () {
    test('start is idempotent and stop leaves no timer behind', () {
      final bridge = SchedulerBridge();
      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);

      controller.startScheduler();
      controller.startScheduler();
      expect(bridge.starts, 1, reason: 'second start is a no-op');
      expect(bridge.running, isTrue);

      controller.stopScheduler();
      controller.stopScheduler();
      expect(bridge.stops, 1, reason: 'stop is idempotent');
      expect(bridge.running, isFalse);
    });

    test('a scheduler already running is not restarted', () {
      final bridge = SchedulerBridge()..running = true;
      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);

      controller.startScheduler();
      expect(
        bridge.starts,
        0,
        reason: 'running scheduler must not be restarted',
      );
      expect(bridge.running, isTrue);
    });
  });
}
