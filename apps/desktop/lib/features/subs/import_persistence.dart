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
/// The desired revision is re-read before every save so the optimistic
/// revision contract holds for the whole batch.
PersistImportedResult persistImportedProfiles(
  BridgePort bridge,
  List<c.ProfileDto> profiles,
) {
  var saved = 0;
  var failed = 0;
  String? firstErrorCode;
  for (final profile in profiles) {
    final result = bridge.saveImportedProfile(
      profile,
      bridge.profileRevision(),
    );
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
