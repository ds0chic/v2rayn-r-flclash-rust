import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';

/// ACT-MAIN-016: import share links from the clipboard (F-IMPORT-001/002/005).
Future<void> importFromClipboard(BuildContext context, WidgetRef ref) async {
  final data = await Clipboard.getData(Clipboard.kTextPlain);
  final text = data?.text ?? '';
  if (text.trim().isEmpty) {
    _toast(ref, '剪贴板为空，没有可导入的分享链接');
    return;
  }
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.importFromText(text, deduplicate: true);
  if (result.ok) {
    ref.read(profilesControllerProvider.notifier).reload();
    _toast(ref, '已从剪贴板导入 ${result.imported} 个节点');
  } else {
    final detail = result.errors.isNotEmpty
        ? '；首个错误：第 ${(result.errors.first.itemIndex ?? 0) + 1} 项'
        : '';
    _toast(ref, '导入失败$detail');
  }
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
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.importFromText(text, deduplicate: true);
  if (result.ok) {
    ref.read(profilesControllerProvider.notifier).reload();
    _toast(ref, '已导入 ${result.imported} 个节点');
  } else {
    _toast(ref, '导入失败：未找到有效分享链接');
  }
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
  _toast(
    ref,
    result.ok
        ? '订阅更新完成：成功 ${result.success}'
        : (result.cancelled ? '订阅更新已取消' : '订阅更新未成功，旧节点已保留'),
  );
}

/// ACT-MAIN-022/023: update the current subscription group.
Future<void> updateCurrentGroup(
  BuildContext context,
  WidgetRef ref, {
  required bool viaProxy,
}) async {
  final subsState = ref.read(subsControllerProvider);
  final selected = subsState.selected;
  if (selected == null) {
    _toast(ref, '请先在订阅设置中选择一个订阅');
    return;
  }
  final result = await ref
      .read(subsControllerProvider.notifier)
      .update(subIds: <String>[selected.id], viaProxy: viaProxy);
  ref.read(profilesControllerProvider.notifier).reload();
  _toast(
    ref,
    result.ok
        ? '订阅“${selected.remarks}”更新完成'
        : (result.cancelled ? '已取消' : '更新未成功，旧节点已保留'),
  );
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}
