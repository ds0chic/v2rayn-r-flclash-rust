import 'dart:convert';

import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/profiles.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;

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

  // -- T09 subscription + import/export surface --------------------------

  c.SubsPageDto listSubItems();

  c.SubItemDto? getSubItem(String id);

  c.SubItemDtoResult saveSubItem(c.SubItemDto item);

  c.DeleteSubsResult deleteSubItems(List<String> ids);

  c.SubItemDtoResult setSubEnabled(String id, bool enabled);

  c.SimpleResult reorderSubItems(List<String> ids);

  c.SimpleResult validateSubItem(c.SubItemDto item);

  void setLocalProxyPort(int? port);

  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  );

  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy);

  c.SimpleResult startSubScheduler();

  c.SimpleResult stopSubScheduler();

  bool subSchedulerRunning();

  c.JobDto? jobView(String jobId);

  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  });

  c.UriParseResult parseShareUri(String line);

  Future<c.ShareExportResult> exportProfiles(List<String> ids, String kind);

  c.SimpleResult writeExportFile(String path, String text);
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

  @override
  c.SubsPageDto listSubItems() => subs.listSubItems();

  @override
  c.SubItemDto? getSubItem(String id) => subs.getSubItem(id: id);

  @override
  c.SubItemDtoResult saveSubItem(c.SubItemDto item) =>
      subs.saveSubItem(item: item);

  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) =>
      subs.deleteSubItems(ids: ids);

  @override
  c.SubItemDtoResult setSubEnabled(String id, bool enabled) =>
      subs.setSubEnabled(id: id, enabled: enabled);

  @override
  c.SimpleResult reorderSubItems(List<String> ids) =>
      subs.reorderSubItems(ids: ids);

  @override
  c.SimpleResult validateSubItem(c.SubItemDto item) =>
      subs.validateSubItem(item: item);

  @override
  void setLocalProxyPort(int? port) => subs.setLocalProxyPort(port: port);

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) => subs.updateSubscriptions(subIds: subIds, viaProxy: viaProxy);

  @override
  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy) =>
      subs.updateSubscription(subId: subId, viaProxy: viaProxy);

  @override
  c.SimpleResult startSubScheduler() => subs.startSubScheduler();

  @override
  c.SimpleResult stopSubScheduler() => subs.stopSubScheduler();

  @override
  bool subSchedulerRunning() => subs.subSchedulerRunning();

  @override
  c.JobDto? jobView(String jobId) => subs.jobView(jobId: jobId);

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) => subs.importFromText(text: text, subid: subid, deduplicate: deduplicate);

  @override
  c.UriParseResult parseShareUri(String line) => subs.parseShareUri(line: line);

  @override
  Future<c.ShareExportResult> exportProfiles(List<String> ids, String kind) =>
      subs.exportProfiles(ids: ids, kind: kind);

  @override
  c.SimpleResult writeExportFile(String path, String text) =>
      subs.writeExportFile(path: path, text: text);
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

  // -- T09 subscription + import/export (synthetic) ----------------------

  final List<c.SubItemDto> _subs = <c.SubItemDto>[];
  int _subSeq = 0;

  @override
  c.SubsPageDto listSubItems() {
    final items = List<c.SubItemDto>.of(_subs)
      ..sort((a, b) => a.sort.compareTo(b.sort));
    return c.SubsPageDto(items: items);
  }

  @override
  c.SubItemDto? getSubItem(String id) {
    for (final s in _subs) {
      if (s.id == id) return s;
    }
    return null;
  }

  @override
  c.SubItemDtoResult saveSubItem(c.SubItemDto item) {
    final invalid = _validateSub(item);
    if (invalid != null) {
      return c.SubItemDtoResult(ok: false, error: invalid);
    }
    var saved = item;
    if (item.id.trim().isEmpty) {
      saved = _withSubId(item, 'syn-sub-${_subSeq++}');
    }
    final index = _subs.indexWhere((s) => s.id == saved.id);
    if (index >= 0) {
      _subs[index] = saved;
    } else {
      _subs.add(saved);
    }
    return c.SubItemDtoResult(ok: true, item: saved);
  }

  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) {
    final before = _subs.length;
    _subs.removeWhere((s) => ids.contains(s.id));
    return c.DeleteSubsResult(
      ok: true,
      removed: BigInt.from(before - _subs.length),
    );
  }

  @override
  c.SubItemDtoResult setSubEnabled(String id, bool enabled) {
    final index = _subs.indexWhere((s) => s.id == id);
    if (index < 0) {
      return const c.SubItemDtoResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.not_found',
          retryable: false,
        ),
      );
    }
    final updated = _withSubId(_subs[index], id, enabled: enabled);
    _subs[index] = updated;
    return c.SubItemDtoResult(ok: true, item: updated);
  }

  @override
  c.SimpleResult reorderSubItems(List<String> ids) {
    final byId = <String, c.SubItemDto>{for (final s in _subs) s.id: s};
    final ordered = <c.SubItemDto>[];
    for (var i = 0; i < ids.length; i++) {
      final item = byId.remove(ids[i]);
      if (item != null) ordered.add(_withSubId(item, item.id, sort: i + 1));
    }
    ordered.addAll(byId.values);
    _subs
      ..clear()
      ..addAll(ordered);
    return const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult validateSubItem(c.SubItemDto item) {
    final invalid = _validateSub(item);
    return invalid == null
        ? const c.SimpleResult(ok: true)
        : c.SimpleResult(ok: false, error: invalid);
  }

  @override
  void setLocalProxyPort(int? port) {}

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    final targets = subIds.isEmpty
        ? _subs.where((s) => s.enabled).toList()
        : _subs.where((s) => subIds.contains(s.id) && s.enabled).toList();
    final entries = <c.SubUpdateEntryDto>[];
    for (final item in targets) {
      if (item.url.trim().isEmpty) {
        entries.add(
          c.SubUpdateEntryDto(
            subId: item.id,
            remarks: item.remarks,
            status: 'skipped',
            message: 'error.url_required',
          ),
        );
        continue;
      }
      // Synthetic success: 3 deterministic nodes per subscription.
      entries.add(
        c.SubUpdateEntryDto(
          subId: item.id,
          remarks: item.remarks,
          status: 'updated',
          added: 3,
          existing: 0,
        ),
      );
    }
    final success = entries.where((e) => e.status == 'updated').length;
    return c.SubUpdateResult(
      ok: success > 0,
      success: success,
      cancelled: false,
      entries: entries,
      jobId: 'syn-sub-job-${_subSeq++}',
    );
  }

  @override
  Future<c.SubUpdateResult> updateSubscription(String subId, bool viaProxy) =>
      updateSubscriptions(<String>[subId], viaProxy);

  @override
  c.SimpleResult startSubScheduler() => const c.SimpleResult(ok: true);

  @override
  c.SimpleResult stopSubScheduler() => const c.SimpleResult(ok: true);

  @override
  bool subSchedulerRunning() => false;

  @override
  c.JobDto? jobView(String jobId) => null;

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) async {
    final lines = text
        .split(RegExp(r'\r?\n'))
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty && l.contains('://'))
        .toList();
    final profiles = <c.ProfileDto>[];
    for (var i = 0; i < lines.length; i++) {
      final uri = lines[i];
      if (uri.startsWith('vmess://') ||
          uri.startsWith('vless://') ||
          uri.startsWith('ss://') ||
          uri.startsWith('trojan://') ||
          uri.startsWith('hysteria2://')) {
        profiles.add(
          c.ProfileDto(
            indexId: 'syn-import-${_subSeq++}',
            configType: ConfigType.vless,
            coreType: CoreType.xray,
            configVersion: 4,
            subid: subid ?? '',
            isSub: true,
            displayLog: true,
            remarks: uri.split('#').length > 1
                ? Uri.decodeComponent(uri.split('#').last)
                : 'import-${i + 1}',
            address: '192.0.2.${i + 1}',
            port: 443,
            password: '',
            username: '',
            network: 'raw',
            security: const c.SecurityDto(),
            protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
            transportExtra: const c.TransportExtraDto(extraJson: '{}'),
            extraJson: '{}',
          ),
        );
      }
    }
    return c.ImportResult(
      ok: profiles.isNotEmpty,
      imported: profiles.length,
      profiles: profiles,
      errors: const <c.ParseIssueDto>[],
      error: profiles.isEmpty
          ? const c.ErrorDto(
              code: 'E_FIELD_FORMAT',
              messageKey: 'error.import_nothing',
              retryable: false,
            )
          : null,
    );
  }

  @override
  c.UriParseResult parseShareUri(String line) {
    if (!line.contains('://')) {
      return const c.UriParseResult(
        ok: false,
        error: c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.invalid_uri',
          retryable: false,
        ),
      );
    }
    return c.UriParseResult(
      ok: true,
      profile: c.ProfileDto(
        indexId: 'syn-uri-${_subSeq++}',
        configType: ConfigType.vless,
        coreType: CoreType.xray,
        configVersion: 4,
        subid: '',
        isSub: false,
        displayLog: true,
        remarks: 'uri',
        address: '192.0.2.1',
        port: 443,
        password: '',
        username: '',
        network: 'raw',
        security: const c.SecurityDto(),
        protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
        transportExtra: const c.TransportExtraDto(extraJson: '{}'),
        extraJson: '{}',
      ),
    );
  }

  @override
  Future<c.ShareExportResult> exportProfiles(
    List<String> ids,
    String kind,
  ) async {
    _ensureProfiles();
    final selected = _profiles.where((p) => ids.contains(p.indexId)).toList();
    if (selected.isEmpty) {
      return const c.ShareExportResult(
        ok: false,
        text: '',
        count: 0,
        error: c.ErrorDto(
          code: 'E_NOT_FOUND',
          messageKey: 'error.no_profiles_selected',
          retryable: false,
        ),
      );
    }
    final lines = selected
        .map(
          (p) => 'vless://syn-${p.indexId}@${p.address}:${p.port}#${p.remarks}',
        )
        .toList();
    final joined = lines.join('\n');
    final text = kind == 'base64' ? base64Encode(utf8.encode(joined)) : joined;
    return c.ShareExportResult(ok: true, text: text, count: selected.length);
  }

  @override
  c.SimpleResult writeExportFile(String path, String text) =>
      const c.SimpleResult(ok: true);

  c.ErrorDto? _validateSub(c.SubItemDto item) {
    if (item.remarks.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.remarks_required',
        fieldPath: 'remarks',
        retryable: false,
      );
    }
    if (item.url.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.url_required',
        fieldPath: 'url',
        retryable: false,
      );
    }
    if (!item.url.startsWith('http://') && !item.url.startsWith('https://')) {
      return const c.ErrorDto(
        code: 'E_FIELD_FORMAT',
        messageKey: 'error.url_invalid',
        fieldPath: 'url',
        retryable: false,
      );
    }
    final headers = item.requestHeaders;
    if (headers != null && headers.trim().isNotEmpty) {
      if (!headers.trimLeft().startsWith('{')) {
        return const c.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.sub_headers_invalid',
          fieldPath: 'requestHeaders',
          retryable: false,
        );
      }
    }
    return null;
  }

  c.SubItemDto _withSubId(
    c.SubItemDto s,
    String id, {
    bool? enabled,
    int? sort,
  }) => c.SubItemDto(
    id: id,
    remarks: s.remarks,
    url: s.url,
    moreUrl: s.moreUrl,
    enabled: enabled ?? s.enabled,
    userAgent: s.userAgent,
    requestHeaders: s.requestHeaders,
    sort: sort ?? s.sort,
    filter: s.filter,
    autoUpdateInterval: s.autoUpdateInterval,
    updateTime: s.updateTime,
    convertTarget: s.convertTarget,
    prevProfile: s.prevProfile,
    nextProfile: s.nextProfile,
    preSocksPort: s.preSocksPort,
    memo: s.memo,
    customCoreType: s.customCoreType,
  );

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
