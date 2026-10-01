import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

/// ACT-MAIN-025: open the routing settings window.
Future<void> openRoutingSettings(BuildContext context, WidgetRef ref) async {
  ref.read(routingControllerProvider.notifier).reload();
  await showRoutingSettingWindow(context, ref);
}

/// ACT-MAIN-026: open the DNS settings window.
Future<void> openDnsSettings(BuildContext context, WidgetRef ref) async {
  await showDnsSettingWindow(context, ref);
}

/// Import rule JSON from a file path (read with dart:io, no picker dep).
Future<void> importRulesFromFile(
  BuildContext context,
  WidgetRef ref,
  String routingId,
) async {
  final pathController = TextEditingController();
  var replace = false;
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => StatefulBuilder(
      builder: (context, setState) => AlertDialog(
        key: const ValueKey('routing-import-file-dialog'),
        title: const Text('从文件导入规则', style: TextStyle(fontSize: 15)),
        content: SizedBox(
          width: 440,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                key: const ValueKey('routing-import-file-path'),
                controller: pathController,
                decoration: const InputDecoration(
                  hintText: '规则 JSON 文件完整路径',
                  border: OutlineInputBorder(),
                ),
              ),
              CheckboxListTile(
                value: replace,
                onChanged: (v) => setState(() => replace = v ?? false),
                title: const Text(
                  '替换现有规则（不勾选则追加）',
                  style: TextStyle(fontSize: 12),
                ),
                controlAffinity: ListTileControlAffinity.leading,
              ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('routing-import-file-ok'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('导入'),
          ),
        ],
      ),
    ),
  );
  if (confirmed != true || !context.mounted) return;
  final path = pathController.text.trim();
  if (path.isEmpty) return;
  String text;
  try {
    text = await File(path).readAsString();
  } catch (_) {
    _toast(ref, '读取文件失败：$path');
    return;
  }
  final result = ref
      .read(routingControllerProvider.notifier)
      .importRules(routingId, text, replace: replace);
  _toast(
    ref,
    result.ok
        ? '已导入 ${result.ruleCount} 条规则'
        : '导入失败：${result.error?.messageKey}',
  );
}

/// Import rule JSON from the clipboard.
Future<void> importRulesFromClipboard(
  BuildContext context,
  WidgetRef ref,
  String routingId,
) async {
  final data = await Clipboard.getData(Clipboard.kTextPlain);
  if (!context.mounted) return;
  final text = data?.text ?? '';
  if (text.trim().isEmpty) {
    _toast(ref, '剪贴板为空');
    return;
  }
  final replace = await _askReplace(context);
  if (replace == null || !context.mounted) return;
  final result = ref
      .read(routingControllerProvider.notifier)
      .importRules(routingId, text, replace: replace);
  _toast(
    ref,
    result.ok
        ? '已导入 ${result.ruleCount} 条规则'
        : '导入失败：${result.error?.messageKey}',
  );
}

/// Import rule JSON from a URL (loopback-friendly, 15 s timeout).
Future<void> importRulesFromUrl(
  BuildContext context,
  WidgetRef ref,
  String routingId,
  String url,
) async {
  if (url.trim().isEmpty) {
    _toast(ref, '请先填写规则 URL');
    return;
  }
  String text;
  try {
    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 15);
    final request = await client.getUrl(Uri.parse(url.trim()));
    final response = await request.close().timeout(const Duration(seconds: 15));
    if (response.statusCode < 200 || response.statusCode >= 300) {
      _toast(ref, '下载失败：HTTP ${response.statusCode}');
      client.close();
      return;
    }
    text = await response.transform(utf8.decoder).join();
    client.close();
  } catch (_) {
    _toast(ref, '下载失败，请检查 URL');
    return;
  }
  if (!context.mounted) return;
  final replace = await _askReplace(context);
  if (replace == null || !context.mounted) return;
  final result = ref
      .read(routingControllerProvider.notifier)
      .importRules(routingId, text, replace: replace);
  _toast(
    ref,
    result.ok
        ? '已导入 ${result.ruleCount} 条规则'
        : '导入失败：${result.error?.messageKey}',
  );
}

/// Export the selected (or all) rules to the clipboard as JSON.
Future<void> exportSelectedRules(
  BuildContext context,
  WidgetRef ref,
  String routingId,
  List<String> ids,
) async {
  final result = ref
      .read(routingControllerProvider.notifier)
      .exportRules(routingId, ids);
  if (!result.ok || result.text.isEmpty) {
    _toast(ref, '导出失败：${result.error?.messageKey ?? '无选中规则'}');
    return;
  }
  await Clipboard.setData(ClipboardData(text: result.text));
  _toast(ref, '已导出 ${result.ruleCount} 条规则到剪贴板');
}

Future<bool?> _askReplace(BuildContext context) => showDialog<bool>(
  context: context,
  builder: (context) => AlertDialog(
    key: const ValueKey('routing-import-mode-dialog'),
    title: const Text('导入规则', style: TextStyle(fontSize: 15)),
    content: const Text('追加到现有规则，还是替换全部规则？'),
    actions: <Widget>[
      TextButton(
        key: const ValueKey('routing-import-append'),
        onPressed: () => Navigator.pop(context, false),
        child: const Text('追加'),
      ),
      FilledButton(
        key: const ValueKey('routing-import-replace'),
        onPressed: () => Navigator.pop(context, true),
        child: const Text('替换'),
      ),
    ],
  ),
);

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}
