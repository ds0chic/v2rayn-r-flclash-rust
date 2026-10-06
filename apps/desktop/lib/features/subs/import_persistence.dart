import 'dart:convert';

import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';

/// Outcome of persisting profiles that the bridge parsed but did not store.
class PersistImportedResult {
  const PersistImportedResult({
    required this.saved,
    required this.failed,
    this.firstErrorCode,
  });

  final int saved;
  final int failed;
  final String? firstErrorCode;

  bool get hasFailures => failed > 0;
}

/// Persists profiles returned by `importFromText` when the backend attached
/// them to no subscription.
///
/// Legacy per-row FIX-04 path (T21-E): kept for the callers that already hold
/// parsed profiles outside the SP-14 preview/commit seam (see
/// `t21e_import_real_bridge_test.dart`). New imports must use [previewImport]
/// + [commitImport], which write the whole batch in one transaction instead of
/// row-by-row.
///
/// [subid] is the group snapshot taken when the import command started. Any
/// returned profile that carries no owning subscription is rebound to it before
/// saving (upstream `AddBatchServers(..., _config.SubIndexId, ...)`), so a
/// paste/scan lands in the group that was visible when the command began. A
/// null/empty snapshot leaves the profile in the "no group" bucket.
///
/// The desired revision is re-read before every save so the optimistic
/// revision contract holds for the whole batch.
PersistImportedResult persistImportedProfiles(
  BridgePort bridge,
  List<c.ProfileDto> profiles, {
  String? subid,
}) {
  final groupSubId = (subid != null && subid.isNotEmpty) ? subid : null;
  var saved = 0;
  var failed = 0;
  String? firstErrorCode;
  for (final profile in profiles) {
    final target = (groupSubId != null && profile.subid.isEmpty)
        ? _withSubId(profile, groupSubId)
        : profile;
    final result = bridge.saveImportedProfile(target, bridge.profileRevision());
    if (result.ok) {
      saved++;
    } else {
      failed++;
      firstErrorCode ??= result.error?.code;
    }
  }
  return PersistImportedResult(
    saved: saved,
    failed: failed,
    firstErrorCode: firstErrorCode,
  );
}

/// A parse-only import result: the profiles are decoded but **nothing** is in
/// the database yet. The commit phase ([commitImport]) performs the single
/// write.
///
/// SP-14 binding: [previewToken] identifies the exact previewed content (FNV-1a
/// over the raw text, same algorithm as `application::import_batch`);
/// [expectedRevision] freezes the desired revision at preview time;
/// [mutationId] makes the commit idempotent. A commit carrying a different
/// token/revision is rejected before any write.
class ImportPreview {
  ImportPreview(this.result, {required String sourceText, String? mutationId})
    : previewToken = _previewTokenFor(sourceText),
      expectedRevision = _frozenRevision,
      mutationId = mutationId ?? _newMutationId();

  /// The raw backend parse result (profiles plus located per-line issues).
  final c.ImportResult result;

  /// Binds the exact previewed content (FNV-1a hex of the raw text).
  final String previewToken;

  /// Desired revision frozen when the preview was taken.
  final int expectedRevision;

  /// Idempotency key for the commit (SP-00 `mutationId`, SP-02 reuse).
  final String mutationId;

  bool get ok => result.ok && result.profiles.isNotEmpty;
  List<c.ProfileDto> get profiles => result.profiles;
  List<c.ParseIssueDto> get errors => result.errors;
  int get imported => result.imported;
}

/// Revision captured for the in-flight [ImportPreview].
///
/// Set by [previewImport] from the live bridge just before parsing, so tests
/// scripting a stale revision can observe the freeze without a native library.
int _frozenRevision = 0;
int _mutationSeq = 0;

/// Completed SP-14 commits keyed by `mutationId`: a retried commit returns the
/// same receipt without rewriting the batch.
final Map<String, PersistImportedResult> _completedMutations =
    <String, PersistImportedResult>{};

/// Test helper: drop cached SP-14 mutation receipts between cases.
void clearImportMutationCache() => _completedMutations.clear();

/// FNV-1a 64 hex over the UTF-8 bytes.
///
/// Same algorithm as `application::import_batch::preview_token`, so the Dart
/// preview token and the Rust commit binding compare equal for the same text.
/// The FRB `CommitImportRequest.preview_token` wiring lands with the
/// integrator; until then the Dart seam enforces the binding.
String _previewTokenFor(String text) {
  // Signed-64 wraparound has identical bits to the unsigned FNV-1a; format via
  // BigInt so the high bit renders as unsigned hex both layers agree on.
  var hash = 0xcbf29ce484222325;
  const prime = 0x100000001b3;
  for (final byte in utf8.encode(text)) {
    hash ^= byte;
    hash = hash * prime;
  }
  return BigInt.from(hash).toUnsigned(64).toRadixString(16).padLeft(16, '0');
}

String _newMutationId() =>
    'sp14-${DateTime.now().microsecondsSinceEpoch}-${_mutationSeq++}';

/// Parse/preview phase (SP-14): nothing is persisted.
///
/// The backend runs the pure `previewImportText` entry point (no group write,
/// no Custom file materialization, `deduplicate: false` mirroring upstream
/// `AddBatchServersCommon`, which only collapses duplicates when `isSub`).
/// Cancelling after this call leaves zero DB/file effects.
Future<ImportPreview> previewImport(
  BridgePort bridge,
  String text, {
  String? mutationId,
}) async {
  _frozenRevision = bridge.profileRevision();
  final result = bridge.previewImportText(text);
  return ImportPreview(result, sourceText: text, mutationId: mutationId);
}

/// Commit phase (SP-14): persist the previewed batch exactly once.
///
/// Every target group — including the All/no-group bucket — goes through the
/// single-transaction `commitImportText` entry point: no second parse (no new
/// id drift), no per-row fallback, no half batch. The call fails closed when:
/// - [previewToken] differs from the preview's token (content drift);
/// - the frozen [expectedRevision] is stale against the live revision;
/// - the backend reports a failure (the batch stays fully unapplied).
///
/// A repeated [mutationId] returns the first receipt without rewriting.
Future<PersistImportedResult> commitImport(
  BridgePort bridge,
  ImportPreview preview, {
  String? subid,
  String? previewToken,
  int? expectedRevision,
  String? mutationId,
}) async {
  final token = previewToken ?? preview.previewToken;
  if (token != preview.previewToken) {
    return PersistImportedResult(
      saved: 0,
      failed: preview.profiles.length,
      firstErrorCode: 'E_PREVIEW_MISMATCH',
    );
  }
  final mutation = mutationId ?? preview.mutationId;
  // Idempotent replay first: the same mutation already validated its revision
  // when it first executed, so a retry returns the recorded receipt even
  // though the live revision has since advanced.
  final cached = _completedMutations[mutation];
  if (cached != null) return cached;
  final revision = expectedRevision ?? preview.expectedRevision;
  if (revision != bridge.profileRevision()) {
    return PersistImportedResult(
      saved: 0,
      failed: preview.profiles.length,
      firstErrorCode: 'E_REVISION_STALE',
    );
  }

  final groupSubId = (subid != null && subid.isNotEmpty) ? subid : null;
  final result = bridge.commitImportText(preview.profiles, subid: groupSubId);
  final receipt = result.ok && result.profiles.isNotEmpty
      ? PersistImportedResult(saved: result.imported, failed: 0)
      : PersistImportedResult(
          saved: 0,
          failed: preview.profiles.length,
          firstErrorCode: result.error?.code ?? 'E_STORAGE',
        );
  _completedMutations[mutation] = receipt;
  return receipt;
}

/// Copy [profile] with its owning subscription replaced by [subid].
c.ProfileDto _withSubId(c.ProfileDto p, String subid) => c.ProfileDto(
  indexId: p.indexId,
  configType: p.configType,
  coreType: p.coreType,
  configVersion: p.configVersion,
  subid: subid,
  isSub: p.isSub,
  preSocksPort: p.preSocksPort,
  displayLog: p.displayLog,
  remarks: p.remarks,
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

final RegExp _subscriptionUrlLine = RegExp(
  r'^\s*https?://\S+\s*$',
  caseSensitive: false,
);

/// HTTP(S) subscription URLs found in pasted text, one per line.
///
/// Used to offer "作为订阅添加" instead of a generic parse failure. Share links
/// (`vless://`, `vmess://`, …) are not subscription URLs and are ignored here.
List<String> extractSubscriptionUrls(String text) {
  final urls = <String>[];
  for (final line in text.split(RegExp(r'\r?\n'))) {
    final trimmed = line.trim();
    if (trimmed.isEmpty) continue;
    if (_subscriptionUrlLine.hasMatch(trimmed)) urls.add(trimmed);
  }
  return urls;
}

/// True when the whole payload is subscription URLs (no share links mixed in).
bool looksLikeSubscriptionPayload(String text) {
  final lines = text
      .split(RegExp(r'\r?\n'))
      .map((l) => l.trim())
      .where((l) => l.isNotEmpty)
      .toList();
  if (lines.isEmpty) return false;
  return lines.every((l) => _subscriptionUrlLine.hasMatch(l));
}

/// Human-readable, actionable reason for an import parse failure.
///
/// Never a bare "导入失败": the caller gets a category plus the located item
/// when the backend reported one (F-IMPORT error contract).
String describeImportFailure(c.ImportResult result) {
  final error = result.errors.isNotEmpty ? result.errors.first : null;
  if (error == null) {
    final fallback = result.error;
    if (fallback == null) return '导入失败：未找到有效分享链接';
    return '导入失败：${_classifyImportError(fallback.code, fallback.messageKey)}'
        '（${fallback.code}）';
  }
  final where = error.itemIndex != null ? '第 ${error.itemIndex! + 1} 项' : '内容';
  return '导入失败：$where ${_classifyImportError(error.code, error.message)}'
      '（${error.code}）';
}

String _classifyImportError(String code, String message) {
  final upper = code.toUpperCase();
  if (upper.contains('EMPTY')) return '内容为空';
  if (upper.contains('UNSUPPORTED') || upper.contains('FORMAT')) {
    return '未识别为分享链接或订阅内容';
  }
  if (upper.contains('LIMIT') || upper.contains('TOO_LARGE')) return '内容超出大小限制';
  if (upper.contains('BASE64')) return 'Base64 解码失败';
  if (message.trim().isEmpty) return '解析失败';
  return message.trim();
}
