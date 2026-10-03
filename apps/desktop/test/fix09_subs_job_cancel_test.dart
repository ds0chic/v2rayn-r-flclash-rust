// FIX-09 (SET-03): the subscription update must hand back the real job id
// before the download completes so progress/cancel bind to that id; a
// cancellation must never report success or replace the old group.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

/// Models the real async contract: `updateSubscriptions` returns the true job
/// id immediately with no entries; `jobView` advances the bound job and
/// `cancelJob` is keyed by that id.
class StartedJobBridge extends SeededSubsBridge {
  final String jobId;
  JobState state = JobState.running;
  int _views = 0;

  StartedJobBridge({this.jobId = 'job-live-1'});

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    return c.SubUpdateResult(
      ok: true,
      success: 0,
      cancelled: false,
      entries: const <c.SubUpdateEntryDto>[],
      jobId: jobId,
    );
  }

  @override
  c.JobDto? jobView(String jobId) {
    _views++;
    if (state == JobState.running && _views >= 2) {
      state = JobState.done;
    }
    return c.JobDto(jobId: jobId, kind: 'update_subscription', state: state);
  }

  @override
  c.CancelResult cancelJob(String jobId) {
    cancelledJobs.add(jobId);
    state = JobState.cancelled;
    return const c.CancelResult(outcome: CancelOutcome.requested);
  }
}

void main() {
  testWidgets('binds the live job id before completion and cancels by id', (
    tester,
  ) async {
    final bridge = StartedJobBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(subsControllerProvider.notifier);

    final future = controller.update(viaProxy: false);
    await tester.pump();

    // The job id is known while the download is still in flight, and no
    // success is reported yet.
    final inFlight = container.read(subsControllerProvider);
    expect(inFlight.lastJobId, bridge.jobId);
    expect(inFlight.busy, isTrue);
    expect(inFlight.status!.isSuccess, isFalse);

    controller.cancel();
    await tester.pump();
    expect(bridge.cancelledJobs, contains(bridge.jobId));

    await tester.pump(const Duration(milliseconds: 150));
    final result = await future;
    await tester.pump();

    expect(result.cancelled, isTrue);
    expect(result.ok, isFalse);
    final settled = container.read(subsControllerProvider);
    expect(settled.busy, isFalse);
    expect(settled.status!.isSuccess, isFalse);
    expect(settled.status!.message, contains('取消'));
    // The old group is untouched (synthetic list still has its row).
    expect(settled.items, isNotEmpty);
  });

  testWidgets('reports a real success only after the bound job completes', (
    tester,
  ) async {
    final bridge = StartedJobBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(subsControllerProvider.notifier);

    final future = controller.update(viaProxy: false);
    await tester.pump();
    expect(container.read(subsControllerProvider).status!.isSuccess, isFalse);

    await tester.pump(const Duration(milliseconds: 150));
    final result = await future;
    await tester.pump();

    expect(result.ok, isTrue);
    expect(result.entries, isNotEmpty);
    expect(container.read(subsControllerProvider).status!.isSuccess, isTrue);
  });
}
