// R4-22 speedtest scope/cancel/generation test seam.
//
// A controllable synthetic bridge (no native library, no network, no ports):
// the real job registry/cancellation and the `ProfileExItem` result closure are
// modelled explicitly so the controller's generation/cancel contract is
// observable. It is not a stand-in for the real FRB/SQLite path.
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

class R422Bridge extends SyntheticBridgePort {
  R422Bridge({super.count = 8});

  final Map<String, speedtest.SpeedTestResultDto> _results =
      <String, speedtest.SpeedTestResultDto>{};

  /// Real job ids returned by [startSpeedTest], in order.
  final List<String> startedJobs = <String>[];

  /// Job ids passed to [cancelSpeedTest], in order.
  final List<String> cancelledRuns = <String>[];

  /// Orders written through [applyProfileOrder], in order.
  final List<List<String>> orders = <List<String>>[];

  int snapshotCalls = 0;
  int startCalls = 0;
  int activeJobs = 0;

  /// When set, the next [startSpeedTest] reports a structured failure.
  String? failStartCode;

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    snapshotCalls++;
    return super.fetchProfileSnapshot(count, text: text, subid: subid);
  }

  @override
  speedtest.SpeedTestStartDto startSpeedTest(int kind, List<String> indexIds) {
    startCalls++;
    final code = failStartCode;
    if (code != null) {
      failStartCode = null;
      return speedtest.SpeedTestStartDto(
        ok: false,
        jobId: null,
        total: 0,
        error: c.ErrorDto(
          code: code,
          messageKey: 'error.test_busy',
          retryable: true,
        ),
      );
    }
    // Model the Rust `begin_job_results` clear: a new run drops the targets'
    // stale results so an old success cannot be counted as this run's success.
    for (final id in indexIds) {
      _results.remove(id);
    }
    final jobId = 'r422-job-${startedJobs.length + 1}';
    startedJobs.add(jobId);
    return speedtest.SpeedTestStartDto(
      ok: true,
      jobId: jobId,
      total: indexIds.length,
    );
  }

  @override
  c.SimpleResult cancelSpeedTest(String jobId) {
    cancelledRuns.add(jobId);
    return const c.SimpleResult(ok: true);
  }

  @override
  List<speedtest.SpeedTestResultDto> speedTestResults() =>
      _results.values.toList();

  @override
  int speedTestActiveJobs() => activeJobs;

  @override
  c.SimpleResult applyProfileOrder(List<String> orderedIds) {
    orders.add(List<String>.of(orderedIds));
    return super.applyProfileOrder(orderedIds);
  }

  /// Publish one controlled result row (a measured delay or a `-1` failure).
  void setResult(String id, int delay, {double speed = 0.0}) {
    _results[id] = speedtest.SpeedTestResultDto(
      indexId: id,
      delay: delay,
      speed: speed,
      message: '',
      ipInfo: '',
    );
  }
}
