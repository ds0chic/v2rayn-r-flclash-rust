import 'dart:convert';
import 'dart:io';

import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/profiles.dart' as rust;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';

/// T18 performance/stability benchmark switch.
///
/// Enabled only by `V2RAYN_R_T18_BENCH=1` in a release build. It never changes
/// production behaviour (the env var is absent in normal runs) and never
/// fabricates a value: every number written here is a timestamp or a frame
/// duration produced by the running release process.
class T18Bench {
  static const String _dirDefault = 'benchmarks/T18';

  static String _env(String key) => Platform.environment[key] ?? '';

  static bool get enabled =>
      const bool.fromEnvironment('V2RAYN_R_SMOKE_ARMED', defaultValue: false) &&
      _env('V2RAYN_R_T18_BENCH') == '1';

  static String get scenario => _env('V2RAYN_R_T18_SCENARIO');

  static int get rows {
    final value = int.tryParse(_env('V2RAYN_R_T18_ROWS'));
    return value == null || value < 0 ? 10000 : value;
  }

  static int get scrollSteps {
    final value = int.tryParse(_env('V2RAYN_R_T18_STEPS'));
    return value == null || value < 1 ? 600 : value;
  }

  static String get tag => _env('V2RAYN_R_T18_TAG');

  static String get dir {
    final value = _env('V2RAYN_R_T18_DIR');
    return value.isEmpty ? _dirDefault : value;
  }

  /// Wall-clock instant recorded at the very top of `main()`.
  static int _mainEpochUs = DateTime.now().microsecondsSinceEpoch;

  static int get mainEpochUs => _mainEpochUs;

  /// Record the benchmark start instant. Must be called at the top of `main()`.
  static void markMainEntry() {
    _mainEpochUs = DateTime.now().microsecondsSinceEpoch;
  }

  static String get _baseName =>
      tag.isEmpty ? _fileBase() : '${_fileBase()}_$tag';

  static String _fileBase() => switch (scenario) {
    'startup' => 'startup',
    'scroll' => 'scroll_$rows',
    _ => 'bench_$scenario',
  };

  /// Called on the first rendered frame. Writes a `first_frame` marker for the
  /// startup scenario (the runner measures process-launch to marker time) and
  /// a lightweight `ready` marker for the hold scenario.
  static void onFirstFrame() {
    if (!enabled) return;
    final now = DateTime.now().microsecondsSinceEpoch;
    final payload = <String, dynamic>{
      'marker': 'first_frame',
      'scenario': scenario,
      'rows': rows,
      'pid': pid,
      'main_epoch_us': mainEpochUs,
      'first_frame_epoch_us': now,
      'main_to_first_frame_ms': (now - mainEpochUs) / 1000.0,
    };
    if (scenario == 'startup') {
      writeJson('$_baseName.json', payload);
      exit(0);
    }
    writeJson('ready.json', payload);
  }

  /// Writes [payload] as pretty JSON under [dir]. Returns the full path.
  static String writeJson(String name, Map<String, dynamic> payload) {
    final directory = Directory(dir)..createSync(recursive: true);
    final file = File('${directory.path}${Platform.pathSeparator}$name');
    file.writeAsStringSync(const JsonEncoder.withIndent('  ').convert(payload));
    debugNote('T18 wrote ${file.path}');
    return file.path;
  }

  static void debugNote(String message) {
    stderr.writeln('[t18] $message');
  }
}

/// Bridge decorator used only by the benchmark run: it projects synthetic rows
/// directly onto the node table so the scroll scenario measures the real
/// virtualized table without touching the user's database or read path.
class T18BenchBridgePort extends FrbBridgePort {
  const T18BenchBridgePort(this.rowCount);

  final int rowCount;

  @override
  List<ProfileSummary> fetchSummaries(int count) =>
      rust.generateProfiles(count: rowCount);

  @override
  int rustProfileCount() => rowCount;

  @override
  List<c.ProfileDto> queryAllProfiles() => const <c.ProfileDto>[];

  @override
  String? getActiveProfile() => null;

  /// SP-21 async page seam: the profiles controller walks pages through this
  /// entry now, so the benchmark must serve its synthetic rows here too or the
  /// table renders empty and the scroll scenario measures nothing.
  @override
  Future<c.ProfilePageDto> queryProfilesPageAsync({
    required c.ProfileFilterDto filter,
    required c.ProfileSortDto sort,
    required int cursor,
    required int pageSize,
    required int requestGeneration,
  }) async {
    final rows = rust.generateProfiles(count: rowCount);
    final start = cursor < 0 ? 0 : cursor;
    if (start >= rows.length) {
      return c.ProfilePageDto(
        items: const <c.ProfileDto>[],
        total: BigInt.from(rows.length),
        nextCursor: null,
        datasetRevision: BigInt.zero,
        requestGeneration: BigInt.from(requestGeneration),
      );
    }
    final end = (start + pageSize).clamp(start, rows.length);
    return c.ProfilePageDto(
      items: rows.sublist(start, end).map(_benchDto).toList(growable: false),
      total: BigInt.from(rows.length),
      nextCursor: end < rows.length ? BigInt.from(end) : null,
      datasetRevision: BigInt.zero,
      requestGeneration: BigInt.from(requestGeneration),
    );
  }

  /// Minimal DTO for one synthetic bench row; the table only needs the
  /// identity/display fields, never a real credential.
  c.ProfileDto _benchDto(ProfileSummary s) => c.ProfileDto(
    indexId: s.id,
    configType: s.configType,
    coreType: s.coreType,
    configVersion: 4,
    subid: s.subRemarks,
    isSub: false,
    displayLog: false,
    remarks: s.remarks,
    address: s.address,
    port: s.port,
    password: '',
    username: '',
    network: s.network,
    security: const c.SecurityDto(),
    protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
    transportExtra: const c.TransportExtraDto(extraJson: '{}'),
    extraJson: '{}',
  );
}
