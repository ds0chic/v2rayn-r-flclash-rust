// SP-31 extension micro-benchmarks (pure Dart, no Flutter/prod imports).
// Run: dart run tools/perf/sp31_ext_bench.dart [--out docs/evidence/stable-port/SP-31]
//
// Covers features landed since the SP-31 baseline (92d46dd):
//   SP-21 pagewalk_10k/pagewalk_100k : frozen filter+stable sort (remark, id
//       tiebreak), page-500 cursor walk with per-page datasetRevision +
//       requestGeneration guards, id collection (select-all-across-pages shape).
//   SP-22 overlay_10k                : ID-keyed delay-map merge over 10k rows +
//       visible-window (200 rows) merge.
//   SP-22 logtail_merge              : 10k-line flood coalesced to 2000 cap +
//       tail merge into retained buffer.
//   SP-17 snapshot_assemble          : actual-descriptor view assembly over 10k
//       rows (frozen target + live facts -> summary/exit labels).
//   SP-27 update_flags               : per-operation flag freeze + with-flags vs
//       no-arg-default dispatch, 10k ops, no network.
//   SP-09 lease_renew                : virtual-time renew_due/lease_expired/
//       reconcile_due evaluation for 10k sidecars (15s/90s/3-fail bounds).
//
// These are Dart-side synthetic mirrors of the data-shape/contract logic with
// the same bounds as production (page 500, 2000-line cap, 15s/90s/3 fails).
// They are NOT Rust timings; Rust-side numbers live in SP-21/SP-22 evidence.
// Percentiles use nearest-rank (rank=ceil(p/100*n)). Synthetic data only,
// no sockets, no OS side effects.
import 'dart:convert';
import 'dart:io';
import 'dart:math';

class Row {
  final String id;
  final String remark;
  final String group;
  final int latency;
  Row(this.id, this.remark, this.group, this.latency);
}

List<Row> genRows(int n, int seed) {
  final r = Random(seed);
  const protos = ['vless', 'vmess', 'trojan', 'ss', 'hysteria2'];
  final out = <Row>[];
  for (var i = 0; i < n; i++) {
    final p = protos[r.nextInt(protos.length)];
    out.add(
      Row(
        'node-$i',
        'sg-$p-${r.nextInt(500)}-x$i',
        'group-${r.nextInt(8)}',
        r.nextInt(900),
      ),
    );
  }
  return out;
}

Map<String, double> pct(List<double> sorted, List<int> ps) {
  final n = sorted.length;
  final m = <String, double>{};
  for (final p in ps) {
    final rank = ((p / 100 * n).ceil()).clamp(1, n);
    m['p$p'] = sorted[rank - 1];
  }
  return m;
}

// SP-21: sort once (frozen dataset revision), then cursor-walk pages of 500
// with per-page revision/generation guards, collecting ids.
int pageWalk(List<Row> rows, int pageSize, int revision, int generation) {
  final sorted = List<Row>.from(rows)
    ..sort((a, b) {
      final c = a.remark.compareTo(b.remark);
      return c != 0 ? c : a.id.compareTo(b.id);
    });
  var collected = 0;
  var cursor = 0;
  while (cursor < sorted.length) {
    // Guard shape mirrors requestGeneration/datasetRevision checks.
    if (revision < 0 || generation < 0) throw StateError('stale');
    final end = min(cursor + pageSize, sorted.length);
    for (var i = cursor; i < end; i++) {
      collected += sorted[i].id.length;
    }
    cursor = end;
  }
  return collected;
}

// SP-22: ID-keyed delay overlay merge + visible-window merge.
int overlayMerge(List<Row> rows, Map<String, int> delays, int visible) {
  for (final r in rows) {
    delays[r.id] = r.latency;
  }
  var acc = 0;
  final n = min(visible, rows.length);
  for (var i = 0; i < n; i++) {
    acc += delays[rows[i].id] ?? -1;
  }
  return acc;
}

// SP-22: flood coalesce to cap, then tail-merge into retained buffer.
int logTailMerge(int flood, int cap) {
  final retained = List<String>.filled(cap, '');
  final tail = <String>[];
  for (var i = 0; i < flood; i++) {
    tail.add('2026-10-07T00:00:00Z [INFO] conn $i node-${i % 10000}');
    if (tail.length > cap) tail.removeRange(0, tail.length - cap);
  }
  for (var i = 0; i < cap; i++) {
    retained[i] = tail[i % tail.length];
  }
  return retained.length + tail.length;
}

// SP-17: assemble actual-descriptor view labels over rows.
int snapshotAssemble(
  List<Row> rows,
  String targetId,
  int generation,
  String coreVersion,
  int exitCode,
) {
  var acc = 0;
  for (final r in rows) {
    final label =
        'actual:$targetId gen=$generation core=$coreVersion node=${r.id}';
    final exit = 'pid=.. code=$exitCode';
    acc += label.length + exit.length + r.remark.length;
  }
  return acc;
}

// SP-27: freeze per-op flags, dispatch with-flags vs no-arg default.
int updateFlags(int ops, bool prereleaseLive, bool viaProxyLive) {
  var withFlags = 0;
  var withDefaults = 0;
  for (var i = 0; i < ops; i++) {
    final frozenPre = prereleaseLive;
    final frozenProxy = viaProxyLive;
    if (i % 7 == 6) {
      withDefaults++; // applyAppUpdateWithDefaults: no-arg safe default.
    } else {
      // check/apply share the same frozen pair.
      withFlags += (frozenPre ? 2 : 0) + (frozenProxy ? 1 : 0);
    }
  }
  return withFlags + withDefaults;
}

// SP-09: virtual-time lease bookkeeping (u64 ms, pure arithmetic).
int leaseRenew(int sidecars, int nowMs) {
  const intervalMs = 15000;
  const termMs = 90000;
  var renewals = 0;
  var reconciles = 0;
  for (var s = 0; s < sidecars; s++) {
    final lastRenew = (s * 7919) % 20000;
    final lastConfirmed = (s * 4799) % 100000;
    final failures = s % 5;
    final renewDue = nowMs - lastRenew >= intervalMs;
    final expired = nowMs - lastConfirmed >= termMs;
    final reconcileDue = failures >= 3;
    if (renewDue) renewals++;
    if (expired || reconcileDue) reconciles++;
  }
  return renewals + reconciles;
}

void main(List<String> args) {
  var outDir = 'docs/evidence/stable-port/SP-31';
  for (var i = 0; i < args.length - 1; i++) {
    if (args[i] == '--out') outDir = args[i + 1];
  }
  const seed = 20261007;
  final rows10k = genRows(10000, seed);
  final rows100k = genRows(100000, seed);
  final delays = <String, int>{};

  final raw = <String, List<double>>{
    'pagewalk_10k_ms': [],
    'pagewalk_100k_ms': [],
    'overlay_10k_ms': [],
    'logtail_merge_ms': [],
    'snapshot_assemble_10k_ms': [],
    'update_flags_10k_ms': [],
    'lease_renew_10k_ms': [],
  };
  final sw = Stopwatch();
  var sink = 0; // checksum so loops are consumed

  void sample(String key, void Function() fn, int reps) {
    for (var k = 0; k < reps; k++) {
      sw.reset();
      sw.start();
      fn();
      sw.stop();
      raw[key]!.add(sw.elapsedMicroseconds / 1000.0);
    }
  }

  sample('pagewalk_10k_ms', () => sink += pageWalk(rows10k, 500, 7, 3), 30);
  sample('pagewalk_100k_ms', () => sink += pageWalk(rows100k, 500, 7, 3), 10);
  sample(
    'overlay_10k_ms',
    () => sink += overlayMerge(rows10k, delays, 200),
    30,
  );
  sample('logtail_merge_ms', () => sink += logTailMerge(10000, 2000), 30);
  sample(
    'snapshot_assemble_10k_ms',
    () => sink += snapshotAssemble(rows10k, 'profile-1', 9, 'v1.2.3', 0),
    30,
  );
  sample(
    'update_flags_10k_ms',
    () => sink += updateFlags(10000, false, true),
    30,
  );
  sample('lease_renew_10k_ms', () => sink += leaseRenew(10000, 3600000), 30);

  final summary = <String, dynamic>{
    'tool': 'sp31_ext_bench.dart',
    'seed': seed,
    'percentile': 'nearest-rank',
    'note':
        'Dart-side synthetic mirrors, same bounds as production '
        '(page 500, 2000-line cap, 15s/90s/3 fails); not Rust timings; '
        'no GUI frames claimed',
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
  final stamp = DateTime.now()
      .toUtc()
      .toIso8601String()
      .replaceAll(':', '-')
      .split('.')
      .first;
  final jsonPath = '$outDir/sp31_ext_${stamp}Z.json';
  final csvPath = '$outDir/sp31_ext_${stamp}Z_raw.csv';
  File(jsonPath)
      .writeAsStringSync(const JsonEncoder.withIndent('  ').convert(summary));
  final sb = StringBuffer('scenario,sample_ms\n');
  raw.forEach((k, v) {
    for (final x in v) {
      sb.writeln('$k,$x');
    }
  });
  File(csvPath).writeAsStringSync(sb.toString());
  print('checksum=$sink');
  print(jsonEncode(summary['scenarios']));
  print('wrote $jsonPath + $csvPath');
}
