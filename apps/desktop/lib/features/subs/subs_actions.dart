import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';

/// ACT-MAIN-016: import share links from the clipboard (F-IMPORT-001/002/005).
///
/// The import runs through the R4-16 parse/preview + single-commit pipeline
/// ([_importPipeline]); when the payload is a subscription URL instead of a
/// share link the user is offered the real subscription add/update path.
Future<void> importFromClipboard(BuildContext context, WidgetRef ref) async {
  // Snapshot the current group before any await: upstream passes
  // `_config.SubIndexId` at command time, so a group switch during parsing
  // must not retarget the imported nodes.
  final groupSubId = ref.read(profilesControllerProvider).groupSubId;
  final data = await Clipboard.getData(Clipboard.kTextPlain);
  final text = data?.text ?? '';
  if (text.trim().isEmpty) {
    _toast(ref, '剪贴板为空，没有可导入的分享链接');
    return;
  }
  if (!context.mounted) return;
  await _importPipeline(
    context,
    ref,
    text,
    groupSubId: groupSubId,
    sourceLabel: '剪贴板',
  );
}

/// ACT-MAIN-016 fallback: import from a pasted multi-line text dialog.
Future<void> importFromTextDialog(BuildContext context, WidgetRef ref) async {
  final controller = TextEditingController();
  final text = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('import-paste-dialog'),
      title: const Text('从文本导入节点', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 480,
        height: 240,
        child: TextField(
          key: const ValueKey('import-paste-field'),
          controller: controller,
          maxLines: null,
          expands: true,
          decoration: const InputDecoration(
            hintText: '粘贴分享链接 / Base64 / v2rayn:// 内部 URI',
            border: OutlineInputBorder(),
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('import-paste-cancel'),
          onPressed: () => Navigator.pop(context),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('import-paste-ok'),
          onPressed: () => Navigator.pop(context, controller.text),
          child: const Text('导入'),
        ),
      ],
    ),
  );
  if (text == null || text.trim().isEmpty) return;
  if (!context.mounted) return;
  await importShareText(context, ref, text);
}

/// Shared share-text import pipeline (ACT-MAIN-016 paste dialog, FIX-05 image
/// QR scan). Snapshots the group, runs the parse/preview phase and then the
/// single commit, refreshes the node table, and falls back to the
/// subscription-add offer when the payload is only subscription URLs.
Future<void> importShareText(
  BuildContext context,
  WidgetRef ref,
  String text, {
  String sourceLabel = '剪贴板',
}) async {
  if (text.trim().isEmpty) return;
  // Same contract as the clipboard entry: snapshot the group once at command
  // start so paste/scan both inherit the visible group.
  final groupSubId = ref.read(profilesControllerProvider).groupSubId;
  await _importPipeline(
    context,
    ref,
    text,
    groupSubId: groupSubId,
    sourceLabel: sourceLabel,
  );
}

/// Parse/preview then a single commit (R4-16).
///
/// Phase 1 [previewImport] decodes the payload without persisting anything; on
/// a usable preview phase 2 [commitImport] binds the frozen [groupSubId] and
/// writes the whole batch exactly once. A preview that only yields standalone
/// subscription URLs is offered to the subscription add path, and an
/// unrecognisable payload surfaces a classified failure.
Future<void> _importPipeline(
  BuildContext context,
  WidgetRef ref,
  String text, {
  required String? groupSubId,
  required String sourceLabel,
}) async {
  final bridge = ref.read(bridgePortProvider);
  final preview = await previewImport(bridge, text);
  if (preview.ok) {
    final persisted = await commitImport(
      bridge,
      text,
      preview,
      subid: groupSubId,
    );
    ref.read(profilesControllerProvider.notifier).reload();
    _toast(
      ref,
      _importSuccessToast(persisted, preview, sourceLabel: sourceLabel),
    );
    return;
  }
  final urls = extractSubscriptionUrls(text);
  if (urls.isNotEmpty) {
    if (!context.mounted) return;
    await _offerAddSubscription(context, ref, urls);
    return;
  }
  _toast(ref, describeImportFailure(preview.result));
}

String _importSuccessToast(
  PersistImportedResult persisted,
  ImportPreview preview, {
  String sourceLabel = '剪贴板',
}) {
  final parts = <String>['已从$sourceLabel导入 ${persisted.saved} 个节点'];
  if (persisted.hasFailures) {
    parts.add('${persisted.failed} 个保存失败（${persisted.firstErrorCode ?? "未知"}）');
  }
  if (preview.errors.isNotEmpty) {
    parts.add('${preview.errors.length} 行未识别');
  }
  return parts.join('，');
}

/// Clipboard payload is a subscription URL: explain the mismatch and, on
/// confirmation, add it through the existing subscription add/update bridge so
/// the subscription list and node table reflect real downloaded nodes.
Future<void> _offerAddSubscription(
  BuildContext context,
  WidgetRef ref,
  List<String> urls,
) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('sub-url-import-dialog'),
      title: const Text('检测到订阅链接', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 480,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const Text('剪贴板内容看起来是订阅链接，而不是分享链接。'),
            const SizedBox(height: 8),
            for (final url in urls.take(5))
              Text(
                url,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: Colors.grey),
              ),
            if (urls.length > 5)
              Text('…共 ${urls.length} 条', style: const TextStyle(fontSize: 12)),
            const SizedBox(height: 12),
            const Text('是否将其作为订阅添加并立即更新？'),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('sub-url-import-cancel'),
          onPressed: () => Navigator.pop(context, false),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('sub-url-import-add'),
          onPressed: () => Navigator.pop(context, true),
          child: const Text('作为订阅添加'),
        ),
      ],
    ),
  );
  if (confirmed != true) {
    _toast(ref, '已取消：未添加订阅');
    return;
  }
  final subs = ref.read(subsControllerProvider.notifier);
  var added = 0;
  for (final url in urls) {
    final saved = subs.save(
      c.SubItemDto(
        id: '',
        remarks: _subRemarksFor(url),
        url: url,
        moreUrl: '',
        enabled: true,
        userAgent: '',
        sort: 0,
        autoUpdateInterval: 0,
        updateTime: 0,
      ),
    );
    if (saved.ok) added++;
  }
  if (added == 0) {
    _toast(ref, '订阅添加失败：未能保存任何订阅');
    return;
  }
  _toast(ref, '已添加 $added 个订阅，正在更新…');
  final update = await subs.update();
  ref.read(profilesControllerProvider.notifier).reload();
  if (update.ok) {
    final nodes = update.entries.fold<int>(
      0,
      (sum, entry) => sum + (entry.added ?? 0),
    );
    _toast(ref, '订阅已添加并更新：新增 $nodes 个节点');
  } else {
    final detail =
        update.error?.code ??
        (update.entries.isNotEmpty
            ? update.entries.first.message ?? update.entries.first.code
            : null);
    _toast(ref, '订阅已添加，但更新失败${detail == null ? '' : '（$detail）'}');
  }
}

String _subRemarksFor(String url) {
  final host = Uri.tryParse(url)?.host ?? '';
  return host.isEmpty ? '订阅 URL' : '订阅 $host';
}

/// F-IMPORT-009: export the selection to a share list / base64 clipboard text.
Future<void> exportProfiles(
  BuildContext context,
  WidgetRef ref, {
  required String kind,
}) async {
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) {
    _toast(ref, '请先选择要导出的节点');
    return;
  }
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.exportProfiles(state.selected.toList(), kind);
  if (!result.ok) {
    _toast(ref, '导出失败：${result.error?.messageKey ?? "无导出项"}');
    return;
  }
  await Clipboard.setData(ClipboardData(text: result.text));
  _toast(ref, '已导出 ${result.count} 个节点到剪贴板');
}

/// F-IMPORT-008: write the URI text to a file (kernel-config export is T10+).
Future<void> exportProfilesToFile(BuildContext context, WidgetRef ref) async {
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) {
    _toast(ref, '请先选择要导出的节点');
    return;
  }
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.exportProfiles(state.selected.toList(), 'share');
  if (!result.ok) {
    _toast(ref, '导出失败：${result.error?.messageKey ?? "无导出项"}');
    return;
  }
  final dir = Directory.systemTemp.path;
  final path =
      '$dir${Platform.pathSeparator}'
      'v2rayn-export-${DateTime.now().millisecondsSinceEpoch}.txt';
  final written = bridge.writeExportFile(path, result.text);
  _toast(ref, written.ok ? '已导出到 $path（仅分享URI文本）' : '写入文件失败');
}

/// F-IMPORT-010: show a single node's share URI as a QR code.
Future<void> shareProfilesQr(BuildContext context, WidgetRef ref) async {
  final state = ref.read(profilesControllerProvider);
  final ids = state.selected.toList();
  if (ids.length != 1) {
    _toast(ref, ids.isEmpty ? '请先选择节点' : '请选择单个节点后分享');
    return;
  }
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.exportProfiles(ids, 'share');
  if (!result.ok) {
    _toast(ref, '无法生成分享链接');
    return;
  }
  if (!context.mounted) return;
  await showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('profile-share-qr'),
      title: const Text('分享节点', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 256,
        height: 256,
        child: ColoredBox(
          color: Colors.white,
          child: QrImageView(
            data: result.text,
            version: QrVersions.auto,
            size: 240,
            backgroundColor: Colors.white,
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    ),
  );
}

/// ACT-MAIN-019: open the subscription settings window.
Future<void> openSubSettings(BuildContext context, WidgetRef ref) async {
  ref.read(subsControllerProvider.notifier).reload();
  await showSubSettingWindow(context, ref);
}

/// ACT-MAIN-020/021: update every subscription (direct / via proxy).
Future<void> updateAllSubscriptions(
  BuildContext context,
  WidgetRef ref, {
  required bool viaProxy,
}) async {
  final subs = ref.read(subsControllerProvider.notifier);
  ref.read(subsControllerProvider.notifier).reload();
  final result = await subs.update(viaProxy: viaProxy);
  ref.read(profilesControllerProvider.notifier).reload();
  _toast(ref, subsUpdateSummary(result, viaProxy: viaProxy));
  if (hasSubUpdateFailures(result) && context.mounted) {
    await _showSubUpdateDetails(context, result);
  }
}

String subsUpdateSummary(c.SubUpdateResult result, {required bool viaProxy}) {
  if (result.cancelled) return '订阅更新已取消';
  if (result.entries.isEmpty) {
    // "Via proxy" without a local endpoint never falls back to direct:
    // say so instead of a generic failure.
    if (result.error?.code == 'E_PROXY_UNAVAILABLE') {
      return '经代理更新失败：本地代理不可用（E_PROXY_UNAVAILABLE），已保留旧节点';
    }
    final detail = result.error?.code;
    return detail == null ? '订阅更新未成功，旧节点已保留' : '订阅更新未成功（$detail），旧节点已保留';
  }
  // Reuse the real per-group report so a partial failure is never summarized as
  // an all-success (R3-SET-06).
  final updated = result.entries.where((e) => e.status == 'updated').length;
  final preserved = result.entries
      .where((e) => e.status.startsWith('preserved'))
      .length;
  final skipped = result.entries.where((e) => e.status == 'skipped').length;
  final failed = result.entries
      .where((e) => e.status == 'failed' || e.status == 'preserved_error')
      .length;
  final parts = <String>['成功 $updated'];
  if (preserved > 0) parts.add('保留 $preserved');
  if (failed > 0) parts.add('失败 $failed');
  if (skipped > 0) parts.add('跳过 $skipped');
  return '订阅更新完成：${parts.join('，')}';
}

bool hasSubUpdateFailures(c.SubUpdateResult result) => result.entries.any(
  (e) => e.status == 'failed' || e.status == 'preserved_error',
);

/// Show the per-group failure rows so the main-menu toast never hides a partial
/// subscription failure behind an "all success" line (R3-SET-06).
Future<void> _showSubUpdateDetails(
  BuildContext context,
  c.SubUpdateResult result,
) async {
  final failed = result.entries
      .where((e) => e.status == 'failed' || e.status == 'preserved_error')
      .toList();
  if (failed.isEmpty) return;
  await showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('subs-update-failures'),
      title: const Text('订阅更新失败详情', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 520,
        child: ListView(
          shrinkWrap: true,
          children: <Widget>[
            for (final entry in failed)
              ListTile(
                dense: true,
                title: Text(
                  entry.remarks.isEmpty ? entry.subId : entry.remarks,
                ),
                subtitle: Text(
                  '${entry.status}'
                  '${entry.code == null ? '' : ' / ${entry.code}'}'
                  '${entry.message == null ? '' : ' — ${entry.message}'}',
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    ),
  );
}

/// ACT-MAIN-022/023: update the current subscription group.
///
/// The target is the node page's current group (`_config.SubIndexId`), frozen
/// before any await; the subscription-settings window's own selected row must
/// not hijack it (D12 / UF-PROF-05). The All group passes an empty SubIndexId,
/// so the backend updates every valid subscription (upstream
/// `SubscriptionHandler.UpdateProcess` only narrows when the id is non-empty).
Future<void> updateCurrentGroup(
  BuildContext context,
  WidgetRef ref, {
  required bool viaProxy,
}) async {
  final groupSubId = ref.read(profilesControllerProvider).groupSubId;
  final subs = ref.read(subsControllerProvider.notifier);
  subs.reload();
  final result = (groupSubId == null || groupSubId.isEmpty)
      ? await subs.update(viaProxy: viaProxy)
      : await subs.update(subIds: <String>[groupSubId], viaProxy: viaProxy);
  ref.read(profilesControllerProvider.notifier).reload();
  _toast(
    ref,
    '${_currentGroupLabel(ref, groupSubId)}：'
    '${subsUpdateSummary(result, viaProxy: viaProxy)}',
  );
  if (hasSubUpdateFailures(result) && context.mounted) {
    await _showSubUpdateDetails(context, result);
  }
}

/// Human label for the captured current group: its real remarks when the
/// subscription row still exists, otherwise the All fallback.
String _currentGroupLabel(WidgetRef ref, String? groupSubId) {
  if (groupSubId == null || groupSubId.isEmpty) return '全部订阅';
  for (final item in ref.read(subsControllerProvider).items) {
    if (item.id == groupSubId) return '订阅“${item.remarks}”';
  }
  return '当前订阅组';
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}
