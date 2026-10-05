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
/// T21-E root cause: `subs.importFromText` (Rust) only writes profiles when a
/// non-empty `subid` is supplied; a clipboard import passes none, so the real
/// bridge reports `ok` / `imported = N` while SQLite stays empty and the node
/// table never updates. The import pipeline therefore persists each parsed
/// profile through `saveImportedProfile` (FIX-04), which accepts the empty
/// remarks/address that share URIs legitimately carry — unlike the editor
/// draft contract enforced by `saveProfile`.
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
class ImportPreview {
  const ImportPreview(this.result);

  /// The raw backend parse result (profiles plus located per-line issues).
  final c.ImportResult result;

  bool get ok => result.ok && result.profiles.isNotEmpty;
  List<c.ProfileDto> get profiles => result.profiles;
  List<c.ParseIssueDto> get errors => result.errors;
  int get imported => result.imported;
}

/// Parse/preview phase (R4-16): nothing is persisted.
///
/// The backend is called with no group so the shared `import_from_text` path
/// never runs `replace_sub_profiles`; a preview therefore never touches SQLite.
/// `deduplicate: false` mirrors upstream `AddBatchServersCommon`, which only
/// collapses duplicates when `isSub` (`arrData.Distinct()` is guarded by
/// `if (isSub)`); a manual paste/scan must keep duplicate entries.
Future<ImportPreview> previewImport(BridgePort bridge, String text) async =>
    ImportPreview(
      await bridge.importFromText(text, subid: null, deduplicate: false),
    );

/// Commit phase (R4-16): persist the batch exactly once.
///
/// With a non-empty group snapshot the backend batch path binds every row to
/// the group and inserts them in one transaction; the Dart side must **not**
/// re-save each row, otherwise the import is written twice (batch + per-row,
/// UF-PROF-08).
///
/// Without a group there is currently no single-transaction batch entry point
/// (a `commit_import_text` binding is pending FRB regeneration), so the only
/// available primitive is the FIX-04 `saveImportedProfile` per row; the rows
/// stay ungrouped.
Future<PersistImportedResult> commitImport(
  BridgePort bridge,
  String text,
  ImportPreview preview, {
  String? subid,
}) async {
  final groupSubId = (subid != null && subid.isNotEmpty) ? subid : null;
  if (groupSubId != null) {
    final result = await bridge.importFromText(
      text,
      subid: groupSubId,
      deduplicate: false,
    );
    if (result.ok && result.profiles.isNotEmpty) {
      return PersistImportedResult(saved: result.imported, failed: 0);
    }
    // The backend transaction either fully applies or not at all; a failed
    // commit leaves no partial rows (upstream `InsertAllAsync`).
    return PersistImportedResult(
      saved: 0,
      failed: preview.profiles.length,
      firstErrorCode: result.error?.code,
    );
  }
  return persistImportedProfiles(bridge, preview.profiles);
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
