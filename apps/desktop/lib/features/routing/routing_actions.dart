import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// ACT-MAIN-025: open the routing settings window as an independent top-level
/// window. The window runs in its own Flutter engine with a snapshot of the
/// schemes+rules; 确定 relays the draft back here, where the existing routing
/// save/reload path persists and applies it. 取消/Esc/title-bar close write
/// nothing. When the native host is absent (widget tests) or window creation
/// fails, the embedded dialog is kept as a fallback with visible feedback.
Future<void> openRoutingSettings(BuildContext context, WidgetRef ref) async {
  ref.read(routingControllerProvider.notifier).reload();
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  // Keep a context that survives the await below: the caller may be a popup
  // menu entry that unmounts when the menu closes.
  final navigator = Navigator.of(context, rootNavigator: true);
  final RoutingEditorSnapshot snapshot;
  try {
    snapshot = _buildRoutingSnapshot(ref);
  } catch (_) {
    if (context.mounted) await showRoutingSettingWindow(context, ref);
    return;
  }
  final opened = await RoutingWindowHost.instance.open(
    snapshot: snapshot,
    onSave: (draftJson) => _applyRoutingDraft(ref, draftJson),
  );
  if (opened) return;
  final fallback = navigator.context;
  if (!RoutingWindowHost.instance.unavailable && fallback.mounted) {
    ScaffoldMessenger.maybeOf(fallback)
        ?.showSnackBar(const SnackBar(content: Text('打开路由设置窗口失败')));
  }
  if (fallback.mounted) {
    await showRoutingSettingWindow(fallback, ref);
  }
}

RoutingEditorSnapshot _buildRoutingSnapshot(WidgetRef ref) {
  final bridge = ref.read(bridgePortProvider);
  final items = ref.read(routingControllerProvider).items;
  final schemes = <RoutingSchemeSnapshot>[];
  for (final item in items) {
    final page = bridge.listRoutingRules(item.id);
    schemes.add(
      RoutingSchemeSnapshot(
        profile: item,
        rules: page.ok ? page.rules : const <r.RoutingRuleDto>[],
      ),
    );
  }
  final basic = ref.read(settingsControllerProvider).group('RoutingBasicItem');
  final outbound = <String>['proxy', 'direct', 'block'];
  try {
    final remarks =
        bridge
            .queryAllProfiles()
            .where((p) => p.configType != ConfigType.custom)
            .map((p) => p.remarks)
            .toList()
          ..sort();
    outbound.addAll(remarks);
  } catch (_) {}
  return RoutingEditorSnapshot(
    schemes: schemes,
    domainStrategy: basic['DomainStrategy']?.toString() ?? '',
    domainStrategySbox: basic['DomainStrategy4Singbox']?.toString() ?? '',
    outboundTags: outbound,
  );
}

/// Persist a draft relayed from the routing window through the existing
/// routing save/delete/set-default + settings-group paths, then reload the
/// runtime so the change takes effect only after a successful save.
Future<RoutingEditorOutcome> _applyRoutingDraft(
  WidgetRef ref,
  String draftJson,
) async {
  // R4-12/D31: the independent window relays each original upstream commit
  // action as its own transactional use case. A payload carrying `kind` is an
  // incremental action, not the whole-window draft.
  Object? raw;
  try {
    raw = jsonDecode(draftJson);
  } catch (_) {
    raw = null;
  }
  if (raw is Map && raw['kind'] is String) {
    return _applyRoutingAction(ref, raw.cast<String, dynamic>());
  }
  final decoded = decodeRoutingDraft(draftJson);
  if (decoded == null) {
    return const RoutingEditorOutcome(ok: false, message: '保存路由设置失败');
  }
  final controller = ref.read(routingControllerProvider.notifier);
  // Snapshot the pre-edit list so deletion detection is not affected by the
  // reload each save performs.
  final current = ref.read(routingControllerProvider).items;
  final draftIds = <String>{};
  var activeId = '';
  for (final scheme in decoded.schemes) {
    final dto = scheme.profile;
    draftIds.add(dto.id);
    if (dto.isActive && dto.id.isNotEmpty) activeId = dto.id;
    final result = controller.save(dto);
    if (!result.ok) {
      return RoutingEditorOutcome(
        ok: false,
        message: _routingErrorMessage(result.error?.messageKey),
      );
    }
  }
  for (final item in current) {
    if (!draftIds.contains(item.id)) {
      controller.delete(item.id);
    }
  }
  final settings = ref.read(settingsControllerProvider.notifier);
  settings.saveGroup('RoutingBasicItem', <String, dynamic>{
    ...ref.read(settingsControllerProvider).group('RoutingBasicItem'),
    'DomainStrategy': decoded.domainStrategy,
    'DomainStrategy4Singbox': decoded.domainStrategySbox,
  });
  String? previousActive;
  for (final item in current) {
    if (item.isActive) {
      previousActive = item.id;
      break;
    }
  }
  final activeChanged = activeId.isNotEmpty && activeId != previousActive;
  if (activeChanged && current.any((e) => e.id == activeId)) {
    await controller.setDefaultAndReload(activeId);
  } else {
    await ref.read(runtimeControllerProvider.notifier).reload();
  }
  return const RoutingEditorOutcome(ok: true);
}

/// One transactional original-upstream routing action relayed from the
/// independent window (R4-12/D31). Each case persists on its own; a failure
/// returns an error and leaves the window open without rolling back earlier
/// completed actions.
Future<RoutingEditorOutcome> _applyRoutingAction(
  WidgetRef ref,
  Map<String, dynamic> action,
) async {
  final controller = ref.read(routingControllerProvider.notifier);
  switch (action['kind']) {
    case 'saveScheme':
      final rawScheme = action['scheme'];
      if (rawScheme is! Map) {
        return const RoutingEditorOutcome(ok: false, message: '保存路由设置失败');
      }
      final scheme = routingSchemeFromJson(rawScheme.cast<String, dynamic>());
      final result = controller.save(scheme.profile);
      if (!result.ok) {
        return RoutingEditorOutcome(
          ok: false,
          message: _routingErrorMessage(result.error?.messageKey),
        );
      }
      await ref.read(runtimeControllerProvider.notifier).reload();
      return const RoutingEditorOutcome(ok: true);
    case 'deleteScheme':
      controller.delete(action['id'] as String? ?? '');
      await ref.read(runtimeControllerProvider.notifier).reload();
      return const RoutingEditorOutcome(ok: true);
    case 'setDefault':
      await controller.setDefaultAndReload(action['id'] as String? ?? '');
      return const RoutingEditorOutcome(ok: true);
    case 'strategy':
      final settings = ref.read(settingsControllerProvider.notifier);
      final group = Map<String, dynamic>.of(
        ref.read(settingsControllerProvider).group('RoutingBasicItem'),
      );
      group['DomainStrategy'] = action['domainStrategy'] as String? ?? '';
      group['DomainStrategy4Singbox'] =
          action['domainStrategySbox'] as String? ?? '';
      settings.saveGroup('RoutingBasicItem', group);
      await ref.read(runtimeControllerProvider.notifier).reload();
      return const RoutingEditorOutcome(ok: true);
    default:
      return const RoutingEditorOutcome(ok: false, message: '未知的路由操作');
  }
}

String _routingErrorMessage(String? key) {
  switch (key) {
    case 'error.routing_delete_failed':
      return '删除路由方案失败';
    case 'error.routing_save_failed':
    default:
      return '保存路由设置失败';
  }
}

/// ACT-MAIN-026: open the DNS settings window.
Future<void> openDnsSettings(BuildContext context, WidgetRef ref) async {
  await showDnsSettingWindow(context, ref);
}

/// A parsed draft import: rules plus whether they replace the draft list.
/// Returning null means cancelled/failed; the caller's draft stays untouched.
typedef DraftRuleImport = ({List<r.RoutingRuleDto> rules, bool replace});

/// Pick a rule JSON file with the native picker, parse and ask append/replace.
/// Nothing is persisted; the caller merges the result into its draft. A
/// cancelled picker or a failed parse leaves the draft untouched.
Future<DraftRuleImport?> pickRulesFromFile(BuildContext context) async {
  const group = XTypeGroup(
    label: '规则 JSON',
    extensions: <String>['json', 'txt'],
  );
  XFile? file;
  try {
    file = await openFile(acceptedTypeGroups: <XTypeGroup>[group]);
  } catch (_) {
    if (context.mounted) _toast(context, '无法打开文件选择器');
    return null;
  }
  if (file == null) return null;
  String text;
  try {
    text = await file.readAsString();
  } catch (_) {
    if (!context.mounted) return null;
    _toast(context, '读取文件失败：${file.path}');
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
