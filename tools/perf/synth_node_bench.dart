// SP-31 synthetic micro-benchmark (pure Dart, no Flutter/prod imports).
// Run: dart run tools/perf/synth_node_bench.dart --out benchmarks/
// Deterministic PRNG seed; synthetic nodes only, no user secrets.
import 'dart:convert';
import 'dart:io';
import 'dart:math';

class SynthNode {
  final String id;
  final String remark;
  final String protocol;
  final String group;
  final int latencyMs;
  SynthNode(this.id, this.remark, this.protocol, this.group, this.latencyMs);
}

List<SynthNode> genNodes(int n, int seed) {
  final r = Random(seed);
  const protos = ['vless', 'vmess', 'trojan', 'ss', 'hysteria2'];
  final out = <SynthNode>[];
  for (var i = 0; i < n; i++) {
    final p = protos[r.nextInt(protos.length)];
    out.add(SynthNode('node-$i', 'sg-$p-${r.nextInt(500)}-x$i', p,
        'group-${r.nextInt(8)}', r.nextInt(900)));
  }
  return out;
}

// Nearest-rank percentiles on sorted data (documented in sp31_method.md).
Map<String, double> pct(List<double> sorted, List<int> ps) {
  final n = sorted.length;
  final m = <String, double>{};
  for (final p in ps) {
    final rank = ((p / 100 * n).ceil()).clamp(1, n);
    m['p$p'] = sorted[rank - 1];
  }
  return m;
}

void main(List<String> args) {
  var outDir = 'benchmarks';
  for (var i = 0; i < args.length - 1; i++) {
    if (args[i] == '--out') outDir = args[i + 1];
  }
  const nodeCount = 10000;
  const repeats = 30;
  final nodes = genNodes(nodeCount, 20261006);
  final raw = <String, List<double>>{
    'filter_10k_ms': [],
    'sort_10k_ms': [],
    'logbatch_1k_ms': [],
  };
  final sw = Stopwatch();
  for (var k = 0; k < repeats; k++) {
    final q = 'sg-vless-${k % 500}';
    sw.reset(); sw.start();
    final hit = nodes.where((e) => e.remark.contains(q)).toList();
    if (hit.isEmpty && q.isEmpty) print('unreachable');
    sw.stop();
    raw['filter_10k_ms']!.add(sw.elapsedMicroseconds / 1000.0);

    sw.reset(); sw.start();
    final cp = List<SynthNode>.from(nodes)
      ..sort((a, b) => a.remark.compareTo(b.remark));
    if (cp.isEmpty) print('unreachable');
    sw.stop();
    raw['sort_10k_ms']!.add(sw.elapsedMicroseconds / 1000.0);

    sw.reset(); sw.start();
    final log = <String>[];
    for (var i = 0; i < 1000; i++) {
      log.add('2026-10-06T00:00:00Z [INFO] conn $i ${nodes[i].id} ${nodes[i].protocol}');
    }
    final joined = log.join('\n');
    if (joined.isEmpty) print('unreachable');
    sw.stop();
    raw['logbatch_1k_ms']!.add(sw.elapsedMicroseconds / 1000.0);
  }
  final summary = <String, dynamic>{
    'tool': 'synth_node_bench.dart',
    'seed': 20261006,
    'node_count': nodeCount,
    'repeats': repeats,
    'percentile': 'nearest-rank',
    'scenarios': <String, dynamic>{},
  };
  for (final e in raw.entries) {
    final s = List<double>.from(e.value)..sort();
    summary['scenarios'][e.key] = {
      'n': s.length,
      'min_ms': s.first,
      'max_ms': s.last,
      ...pct(s, [50, 95, 99]),
    };
  }
  final stamp = DateTime.now().toUtc().toIso8601String().replaceAll(':', '-').split('.').first;
  final jsonPath = '$outDir/sp31_baseline_${stamp}Z.json';
  final csvPath = '$outDir/sp31_baseline_${stamp}Z_raw.csv';
  File(jsonPath).writeAsStringSync(
      const JsonEncoder.withIndent('  ').convert(summary));
  final sb = StringBuffer('scenario,sample_ms\n');
  raw.forEach((k, v) { for (final x in v) { sb.writeln('$k,$x'); } });
  File(csvPath).writeAsStringSync(sb.toString());
  print(jsonEncode(summary['scenarios']));
  print('wrote $jsonPath + $csvPath');
}
