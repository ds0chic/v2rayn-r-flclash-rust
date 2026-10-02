import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/widgets.dart';

/// S1 spike: a scripted-scroll frame-time measurement skeleton. It records
/// UI (build) and raster frame durations while scrolling a virtualized table
/// and writes raw samples plus a p50/p95 summary. It deliberately does not
/// decide pass/fail.
///
/// The switch is a compile-time constant (`--dart-define=T01_BENCH=true`), not
/// a `Platform.environment` read: synchronous dart:io calls during a widget
/// build run inside the test FakeAsync zone and crash this Flutter build.
class PerfHarness {
  static const bool enabled = bool.fromEnvironment('T01_BENCH');

  static const String outputDir = String.fromEnvironment('T01_BENCH_DIR');

  static Future<void> run({
    required ScrollController vertical,
    required int rowCount,
    int scrollSteps = 600,
    String? outputFile,
    double budgetMs = 16.7,
    String label = 'S1',
  }) async {
    final samples = <Map<String, int>>[];
    void callback(List<FrameTiming> timings) {
      for (final timing in timings) {
        samples.add(<String, int>{
          'build_us': timing.buildDuration.inMicroseconds,
          'raster_us': timing.rasterDuration.inMicroseconds,
        });
      }
    }

    SchedulerBinding.instance.addTimingsCallback(callback);
    try {
      for (var step = 0; step < scrollSteps; step++) {
        await Future<void>.delayed(const Duration(milliseconds: 8));
        if (!vertical.hasClients) continue;
        final max = vertical.position.maxScrollExtent;
        if (!max.isFinite) continue;
        final target = max * (step / (scrollSteps - 1));
        vertical.jumpTo(target.clamp(0.0, max));
      }
      await Future<void>.delayed(const Duration(milliseconds: 300));
    } finally {
      SchedulerBinding.instance.removeTimingsCallback(callback);
    }

    final build = samples.map((s) => s['build_us']!).toList()..sort();
    final raster = samples.map((s) => s['raster_us']!).toList()..sort();
    final total = samples.map((s) => s['build_us']! + s['raster_us']!).toList()
      ..sort();
    final budgetUs = (budgetMs * 1000).round();
    final dropped = total.where((us) => us > budgetUs).length;
    final payload = <String, dynamic>{
      'label': label,
      'method': 'SchedulerBinding.addTimingsCallback',
      'build_mode': kReleaseMode
          ? 'release'
          : (kProfileMode ? 'profile' : 'debug'),
      'row_count': rowCount,
      'scroll_steps': scrollSteps,
      'budget_ms': budgetMs,
      'samples': samples,
      'summary': <String, dynamic>{
        'sample_count': samples.length,
        'build_p50_us': _percentile(build, 50),
        'build_p95_us': _percentile(build, 95),
        'raster_p50_us': _percentile(raster, 50),
        'raster_p95_us': _percentile(raster, 95),
        'total_p50_us': _percentile(total, 50),
        'total_p95_us': _percentile(total, 95),
        'dropped_frames': dropped,
        'dropped_rate': samples.isEmpty ? 0.0 : dropped / samples.length,
      },
    };

    final path = outputFile ?? _defaultPath(rowCount);
    final file = File(path);
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(const JsonEncoder.withIndent('  ').convert(payload));
    debugPrint(
      'perf harness wrote $path samples=${samples.length} dropped=$dropped',
    );
  }

  static String _defaultPath(int rowCount) {
    final dir = outputDir.isEmpty
        ? 'benchmarks${Platform.pathSeparator}T01'
        : outputDir;
    return '$dir${Platform.pathSeparator}frame_times.json';
  }

  static int _percentile(List<int> sorted, int percentile) {
    if (sorted.isEmpty) return 0;
    final index = ((percentile / 100) * (sorted.length - 1)).round();
    return sorted[index];
  }
}
