import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
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

/// A parsed draft import: rules plus whether they replace the draft list.
/// Returning null means cancelled/failed; the caller's draft stays untouched.
typedef DraftRuleImport = ({List<r.RoutingRuleDto> rules, bool replace});

/// Read rule JSON from a file path, parse and ask append/replace.
/// Nothing is persisted; the caller merges the result into its draft.
Future<DraftRuleImport?> pickRulesFromFile(BuildContext context) async {
  final pathController = TextEditingController();
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('routing-import-file-dialog'),
      title: const Text('从文件导入规则', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 440,
        child: TextField(
          key: const ValueKey('routing-import-file-path'),
          controller: pathController,
          decoration: const InputDecoration(
            hintText: '规则 JSON 文件完整路径',
            border: OutlineInputBorder(),
          ),
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
          child: const Text('读取'),
        ),
      ],
    ),
  );
  if (confirmed != true || !context.mounted) return null;
  final path = pathController.text.trim();
  if (path.isEmpty) return null;
  String text;
  try {
    text = await File(path).readAsString();
  } catch (_) {
    if (!context.mounted) return null;
    _toast(context, '读取文件失败：$path');
    return null;
  }
  if (!context.mounted) return null;
  return _parseAndAsk(context, text);
}

/// Read rule JSON from the clipboard, parse and ask append/replace.
Future<DraftRuleImport?> pickRulesFromClipboard(BuildContext context) async {
  final data = await Clipboard.getData(Clipboard.kTextPlain);
  if (!context.mounted) return null;
  final text = data?.text ?? '';
  if (text.trim().isEmpty) {
    _toast(context, '剪贴板为空');
    return null;
  }
  return _parseAndAsk(context, text);
}

/// Download rule JSON from a URL (loopback-friendly, 15 s timeout), parse
/// and ask append/replace. An empty URL asks for one first (upstream
/// `MsgNeedUrl`).
Future<DraftRuleImport?> pickRulesFromUrl(
  BuildContext context,
  String url,
) async {
  if (url.trim().isEmpty) {
    _toast(context, '请先填写规则 URL');
    return null;
  }
  String text;
  try {
    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 15);
    final request = await client.getUrl(Uri.parse(url.trim()));
    final response = await request.close().timeout(const Duration(seconds: 15));
    if (response.statusCode < 200 || response.statusCode >= 300) {
      if (!context.mounted) return null;
      _toast(context, '下载失败：HTTP ${response.statusCode}');
      client.close();
      return null;
    }
    text = await response.transform(utf8.decoder).join();
    client.close();
  } catch (_) {
    if (!context.mounted) return null;
    _toast(context, '下载失败，请检查 URL');
    return null;
  }
  if (!context.mounted) return null;
  return _parseAndAsk(context, text);
}

Future<DraftRuleImport?> _parseAndAsk(BuildContext context, String text) async {
  final List<r.RoutingRuleDto> rules;
  try {
    rules = RoutingController.parseImportedRuleDtos(text);
  } on FormatException catch (e) {
    _toast(context, '导入失败：${e.message}');
    return null;
  }
  return _askReplace(context, rules);
}

Future<DraftRuleImport?> _askReplace(
  BuildContext context,
  List<r.RoutingRuleDto> rules,
) async {
  final replace = await showDialog<bool>(
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
  if (replace == null) return null;
  return (rules: rules, replace: replace);
}

void _toast(BuildContext context, String message) {
  ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
}
