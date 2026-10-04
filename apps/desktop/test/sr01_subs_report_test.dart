// SR-01: the subscription update UI must reflect the backend per-group report
// verbatim (A success, B failure, C empty-URL group skipped), never treat a
// Done job as "every target updated".
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

/// Delivers the terminal report exactly like the Rust bridge does: the true
/// job id first, then the report parked on the job's stage key.
class PartialReportBridge extends SeededSubsBridge {
  PartialReportBridge({required this.reportJson, this.jobId = 'job-sr01'});

  final String jobId;
  final String reportJson;
  JobState state = JobState.running;
  int _views = 0;

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
    return c.JobDto(
      jobId: jobId,
      kind: 'update_subscription',
      state: state,
      stageKey: state == JobState.done
          ? '${SubsController.reportStagePrefix}$reportJson'
          : null,
    );
  }
}

void main() {
  const report =
      '{"success":1,"cancelled":false,"entries":['
      '{"sub_id":"s-a","remarks":"A 订阅","status":"updated","added":2},'
      '{"sub_id":"s-b","remarks":"B 订阅","status":"preserved_error",'
      '"code":"E_UNAVAILABLE","message":"error.download_failed"},'
      '{"sub_id":"s-c","remarks":"C 普通分组","status":"skipped",'
      '"message":"error.url_required"}'
      ']}';

  testWidgets('per-group report: A success, B failure, C empty URL skipped', (
    tester,
  ) async {
    final bridge = PartialReportBridge(reportJson: report);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(subsControllerProvider.notifier);

    final future = controller.update(viaProxy: false);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 150));
    final result = await future;
    await tester.pump();

    // Real per-group outcomes, not a single "all updated" verdict.
    expect(result.success, 1);
    expect(result.entries, hasLength(3));
    final a = result.entries.firstWhere((e) => e.subId == 's-a');
    final b = result.entries.firstWhere((e) => e.subId == 's-b');
    final cc = result.entries.firstWhere((e) => e.subId == 's-c');
    expect(a.status, 'updated');
    expect(a.added, 2);
    expect(b.status, 'preserved_error');
    expect(cc.status, 'skipped');

    // The summary must count only the real success and disclose the rest.
    final settled = container.read(subsControllerProvider);
    expect(settled.status!.isSuccess, isTrue);
    expect(settled.status!.message, contains('成功 1'));
    expect(settled.status!.message, contains('跳过 1'));
    expect(settled.status!.message, contains('失败 1'));
    expect(settled.status!.message, isNot(contains('成功 3')));
  });
}
