import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/profiles_harness.dart';
import 'support/subs_harness.dart';

/// Controllable job bridge for the widget shell: the update stays running
/// until cancelled (deterministic progress), then reports cancelled so the
/// controller settles and no poll timer outlives the test.
class _HangingJobBridge extends SeededSubsBridge {
  JobState _state = JobState.running;

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    return const c.SubUpdateResult(
      ok: true,
      success: 0,
      cancelled: false,
      entries: <c.SubUpdateEntryDto>[],
      jobId: 'job-ui-1',
    );
  }

  @override
  c.JobDto? jobView(String jobId) {
    return c.JobDto(
      jobId: jobId,
      kind: 'update_subscription',
      state: _state,
    );
  }

  @override
  c.CancelResult cancelJob(String jobId) {
    _state = JobState.cancelled;
    return const c.CancelResult(outcome: CancelOutcome.requested);
  }
}

/// End-to-end wiring of the primary update entry: tapping it starts a real
/// controller update (visible in the new global row), and cancelling through
/// the new global entry settles the job so no poll timer outlives the test.
void main() {
  testWidgets('primary entry starts, shows and settles an update', (
    tester,
  ) async {
    final container = await pumpApp(
      tester,
      rows: 0,
      bridge: _HangingJobBridge(),
    );
    await tester.tap(find.byKey(const ValueKey('menu-订阅分组')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-更新订阅')));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();
    expect(container.read(subsControllerProvider).busy, isTrue);
    expect(find.byKey(const ValueKey('subs-global-progress')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('subs-global-cancel')));
    await tester.pumpAndSettle();
    expect(container.read(subsControllerProvider).busy, isFalse);
    expect(find.byKey(const ValueKey('subs-global-progress')), findsNothing);
  });
}
