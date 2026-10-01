import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/profiles.dart' as rust;

/// Thin, testable seam over the flutter_rust_bridge generated API.
///
/// The application always uses [FrbBridgePort]; [SyntheticBridgePort] only
/// exists so widget tests can run without loading the native library.
abstract class BridgePort {
  Future<void> init();

  List<ProfileSummary> generate(int count);

  int rustProfileCount();

  int pingProfile(String id);

  Stream<rust.ProgressEvent> progressStream(int count);

  rust.UiEventAck echoEvent(int seq, String kind);

  Future<int> simulateBlocking(int ms);

  // -- T06a profile repository surface -----------------------------------

  /// Rows for the node table. Synthetic data in tests; real query in
  /// production.
  List<ProfileSummary> fetchSummaries(int count);

  /// Every profile currently stored, used by the editor and batch actions.
  List<c.ProfileDto> queryAllProfiles();

  /// One full profile by stable id, if present.
  c.ProfileDto? getProfile(String id);

  /// Current desired revision for optimistic saves.
  int profileRevision();

  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision);

  c.DeleteProfilesResult deleteProfiles(List<String> ids);

  c.CopyProfilesResult copyProfiles(List<String> ids);

  c.SaveProfileResult setProfileRemarks(String id, String remarks);

  c.SimpleResult setActiveProfile(String? id);

  String? getActiveProfile();

  /// Point the engine at an explicit data directory (tests/portable). No-op
  /// once the engine is live.
  c.SimpleResult initEngine(String? dataDir);
}

class FrbBridgePort implements BridgePort {
  const FrbBridgePort();

  @override
  Future<void> init() => RustBridgeInit.init();

  @override
  List<ProfileSummary> generate(int count) =>
      rust.generateProfiles(count: count);

  @override
  int rustProfileCount() => rust.rustProfileCount();

  @override
  int pingProfile(String id) => rust.pingProfile(id: id);

  @override
  Stream<rust.ProgressEvent> progressStream(int count) =>
      rust.progressStream(count: count);

  @override
  rust.UiEventAck echoEvent(int seq, String kind) =>
      rust.echoUiEvent(seq: BigInt.from(seq), kind: kind);

  @override
  Future<int> simulateBlocking(int ms) async {
    final elapsed = await rust.simulateBlocking(ms: BigInt.from(ms));
    return elapsed.toInt();
  }

  @override
  List<ProfileSummary> fetchSummaries(int count) =>
      queryAllProfiles().map(dtoToSummary).toList();

  @override
  List<c.ProfileDto> queryAllProfiles() {
    final page = engine.queryProfiles(
      filter: const c.ProfileFilterDto(
        text: null,
        configTypes: [],
        subid: null,
      ),
      sort: c.ProfileSortDto.indexId,
      cursor: BigInt.zero,
      pageSize: 100000,
    );
    return page.items;
  }

  @override
  c.ProfileDto? getProfile(String id) => engine.getProfile(indexId: id);

  @override
  int profileRevision() => engine.profileRevision().toInt();

  @override
  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision) =>
      engine.saveProfile(
        draft: draft,
        expectedRevision: BigInt.from(expectedRevision),
      );

  @override
  c.DeleteProfilesResult deleteProfiles(List<String> ids) =>
      engine.deleteProfiles(ids: ids);

  @override
  c.CopyProfilesResult copyProfiles(List<String> ids) =>
      engine.copyProfiles(ids: ids);

  @override
  c.SaveProfileResult setProfileRemarks(String id, String remarks) =>
      engine.setProfileRemarks(indexId: id, remarks: remarks);

  @override
  c.SimpleResult setActiveProfile(String? id) =>
      engine.setActiveProfile(indexId: id);

  @override
  String? getActiveProfile() => engine.getActiveProfile();

  @override
  c.SimpleResult initEngine(String? dataDir) =>
      engine.initEngine(dataDir: dataDir);
}

/// Map a stored profile DTO onto the node-table summary shape. Traffic/delay
/// fields stay at their "unknown" defaults (never fabricated).
ProfileSummary dtoToSummary(c.ProfileDto dto) => ProfileSummary(
  id: dto.indexId,
  configType: dto.configType,
  remarks: dto.remarks,
  address: dto.address,
  port: dto.port,
  network: dto.network,
  streamSecurity: dto.security.streamSecurity ?? '',
  subRemarks: dto.subid,
  delay: -1,
  speed: '-',
  todayUp: BigInt.zero,
  ipInfo: '-',
  todayDown: BigInt.zero,
  totalUp: BigInt.zero,
  totalDown: BigInt.zero,
  coreType: dto.coreType ?? CoreType.xray,
);

/// Loads the FRB dynamic library; isolated so tests can avoid touching it.
class RustBridgeInit {
  static Future<void> Function()? _delegate;

  static void configure(Future<void> Function() delegate) =>
      _delegate = delegate;

  static Future<void> init() async {
    final delegate = _delegate;
    if (delegate == null) {
      throw StateError('Rust bridge init delegate is not configured');
    }
    await delegate();
  }
}

/// Deterministic Dart-side generator used only by tests.
class SyntheticBridgePort implements BridgePort {
  SyntheticBridgePort({this.count = 10000});

  final int count;
  List<ProfileSummary>? _rows;
  final List<c.ProfileDto> _profiles = <c.ProfileDto>[];
  bool _seeded = false;
  int _revision = 0;
  String? _active;
  int _newId = 0;

  @override
  Future<void> init() async {}

  @override
  List<ProfileSummary> generate(int count) {
    _rows = List<ProfileSummary>.generate(count, _build);
    return _rows!;
  }

  @override
  int rustProfileCount() => _rows?.length ?? 0;

  @override
  int pingProfile(String id) {
    final rows = _rows ?? const <ProfileSummary>[];
    final row = rows.firstWhere((r) => r.id == id, orElse: () => _build(0));
    return row.delay < 0 ? 0 : row.delay;
  }

  @override
  Stream<rust.ProgressEvent> progressStream(int count) async* {
    var done = 0;
    var seq = 0;
    while (done < count) {
      final step = done + 1000 <= count ? 1000 : count - done;
      done += step;
      seq++;
      yield rust.ProgressEvent(seq: seq, done: done, total: count);
    }
  }

  @override
  rust.UiEventAck echoEvent(int seq, String kind) => rust.UiEventAck(
    seq: BigInt.from(seq),
    kind: kind,
    rustProfileCount: rustProfileCount(),
  );

  @override
  Future<int> simulateBlocking(int ms) async => ms;

  void _ensureProfiles() {
    if (_seeded) return;
    _seeded = true;
    // The editor store is seeded with a small deterministic window; the table
    // summary path keeps generating the full `count`.
    final seed = count < 500 ? count : 500;
    for (var i = 0; i < seed; i++) {
      final row = _build(i);
      _profiles.add(
        c.ProfileDto(
          indexId: row.id,
          configType: row.configType,
          coreType: row.coreType,
          configVersion: 4,
          subid: row.subRemarks,
          isSub: true,
          preSocksPort: null,
          displayLog: true,
          remarks: row.remarks,
          address: row.address,
          port: row.port,
          password: '',
          username: '',
          network: row.network,
          muxEnabled: null,
          finalmask: null,
          security: c.SecurityDto(
            streamSecurity: row.streamSecurity == 'none'
                ? null
                : row.streamSecurity,
          ),
          protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
          transportExtra: const c.TransportExtraDto(extraJson: '{}'),
          extraJson: '{}',
        ),
      );
    }
  }

  @override
  List<ProfileSummary> fetchSummaries(int count) => generate(count);

  @override
  List<c.ProfileDto> queryAllProfiles() {
    _ensureProfiles();
    return List<c.ProfileDto>.of(_profiles);
  }

  @override
  c.ProfileDto? getProfile(String id) {
    _ensureProfiles();
    for (final p in _profiles) {
      if (p.indexId == id) return p;
    }
    return null;
  }

  @override
  int profileRevision() => _revision;

  @override
  c.SaveProfileResult saveProfile(c.ProfileDto draft, int expectedRevision) {
    _ensureProfiles();
    if (expectedRevision != _revision) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_REVISION_STALE',
          messageKey: 'error.revision_stale',
          retryable: false,
        ),
      );
    }
    if (draft.remarks.trim().isEmpty) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    var saved = draft;
    if (draft.indexId.trim().isEmpty) {
      saved = _withId(draft, 'syn-new-${_newId++}');
    }
    final index = _profiles.indexWhere((p) => p.indexId == saved.indexId);
    if (index >= 0) {
      _profiles[index] = saved;
    } else {
      _profiles.add(saved);
    }
    _revision += 1;
    return c.SaveProfileResult(
      ok: true,
      profile: saved,
      newRevision: BigInt.from(_revision),
    );
  }

  @override
  c.DeleteProfilesResult deleteProfiles(List<String> ids) {
    _ensureProfiles();
    final before = _profiles.length;
    _profiles.removeWhere((p) => ids.contains(p.indexId));
    final removed = before - _profiles.length;
    if (removed > 0) _revision += 1;
    return c.DeleteProfilesResult(ok: true, removed: BigInt.from(removed));
  }

  @override
  c.CopyProfilesResult copyProfiles(List<String> ids) {
    _ensureProfiles();
    final copies = <c.ProfileDto>[];
    for (final id in ids) {
      final source = getProfile(id);
      if (source == null) continue;
      final copy = _withId(
        source,
        'syn-copy-${_newId++}',
        remarks: '${source.remarks} (副本)',
      );
      _profiles.add(copy);
      copies.add(copy);
    }
    if (copies.isNotEmpty) _revision += 1;
    return c.CopyProfilesResult(ok: true, copies: copies);
  }

  @override
  c.SaveProfileResult setProfileRemarks(String id, String remarks) {
    _ensureProfiles();
    if (remarks.trim().isEmpty) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.remarks_required',
          fieldPath: 'remarks',
          retryable: false,
        ),
      );
    }
    final index = _profiles.indexWhere((p) => p.indexId == id);
    if (index < 0) {
      return const c.SaveProfileResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final updated = _withId(_profiles[index], id, remarks: remarks);
    _profiles[index] = updated;
    _revision += 1;
    return c.SaveProfileResult(ok: true, profile: updated);
  }

  @override
  c.SimpleResult setActiveProfile(String? id) {
    _active = id;
    return const c.SimpleResult(ok: true);
  }

  @override
  String? getActiveProfile() => _active;

  @override
  c.SimpleResult initEngine(String? dataDir) => const c.SimpleResult(ok: true);

  c.ProfileDto _withId(c.ProfileDto p, String id, {String? remarks}) =>
      c.ProfileDto(
        indexId: id,
        configType: p.configType,
        coreType: p.coreType,
        configVersion: p.configVersion,
        subid: p.subid,
        isSub: p.isSub,
        preSocksPort: p.preSocksPort,
        displayLog: p.displayLog,
        remarks: remarks ?? p.remarks,
        address: p.address,
        port: p.port,
        password: p.password,
        username: p.username,
        network: p.network,
        muxEnabled: p.muxEnabled,
        finalmask: p.finalmask,
        security: p.security,
        protoExtra: p.protoExtra,
        transportExtra: p.transportExtra,
        extraJson: p.extraJson,
      );

  ProfileSummary _build(int index) {
    final h = (index * 2654435761) & 0x7fffffff;
    const blocks = <String>['192.0.2', '198.51.100', '203.0.113'];
    const domains = <String>['example.com', 'example.org', 'example.net'];
    final block = blocks[h % blocks.length];
    final octet = 1 + (h ~/ 7) % 254;
    final isDomain = h.isEven;
    final address = isDomain
        ? 'node${index.toString().padLeft(5, '0')}.${domains[h % domains.length]}'
        : '$block.$octet';
    final delay = h % 11 == 0 ? -1 : h % 400;
    final gb = BigInt.from(1073741824);
    return ProfileSummary(
      id: 'syn-${index.toString().padLeft(6, '0')}',
      configType: ConfigType.values[h % ConfigType.values.length],
      remarks: 'Synthetic-${index.toString().padLeft(5, '0')}',
      address: address,
      port: 10000 + (h % 50000),
      network: const ['tcp', 'ws', 'grpc', 'http'][h % 4],
      streamSecurity: const ['none', 'tls', 'reality'][h % 3],
      subRemarks: 'sub-${(index ~/ 100).toString().padLeft(3, '0')}',
      delay: delay,
      speed: delay < 0 ? '-' : '${((h % 5000) / 100).toStringAsFixed(2)} MB/s',
      todayUp: gb * BigInt.from(h % 50) ~/ BigInt.from(100),
      ipInfo: h % 5 == 0 ? '-' : '$block.$octet',
      todayDown: gb * BigInt.from(h % 500) ~/ BigInt.from(10),
      totalUp: gb * BigInt.from(h % 400),
      totalDown: gb * BigInt.from(h % 4000),
      coreType: CoreType.values[h % CoreType.values.length],
    );
  }
}
