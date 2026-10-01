import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/groups.dart' as g;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Full-config template settings window (v2ray + sing-box dual tabs).
///
/// Mirrors `FullConfigTemplateViewModel`: per-core `Enabled`, `Config`,
/// `TunConfig`, `AddProxyOnly` and `ProxyDetour`. Both JSON blobs must be
/// empty or parse as JSON objects. Cancel discards the drafts; save submits
/// both rows through [onSave] and stays open with the first error.
class FullConfigTemplateWindow extends StatefulWidget {
  const FullConfigTemplateWindow({
    super.key,
    required this.initial,
    required this.onSave,
  });

  /// The stored rows (Xray + sing-box builtins are always present).
  final List<g.FullConfigTemplateDto> initial;
  final g.TemplateDtoResult Function(g.FullConfigTemplateDto item) onSave;

  @override
  State<FullConfigTemplateWindow> createState() =>
      _FullConfigTemplateWindowState();
}

class _TemplateTabState {
  _TemplateTabState(this.item);

  g.FullConfigTemplateDto item;
  late TextEditingController config = TextEditingController(
    text: item.config ?? '',
  );
  late TextEditingController tunConfig = TextEditingController(
    text: item.tunConfig ?? '',
  );
  late TextEditingController proxyDetour = TextEditingController(
    text: item.proxyDetour ?? '',
  );
  String? configError;
  String? tunConfigError;

  void dispose() {
    config.dispose();
    tunConfig.dispose();
    proxyDetour.dispose();
  }
}

class _FullConfigTemplateWindowState extends State<FullConfigTemplateWindow>
    with SingleTickerProviderStateMixin {
  late TabController _tabs;
  late _TemplateTabState _xray;
  late _TemplateTabState _singbox;
  String? _serverError;
  bool _submitting = false;

  g.FullConfigTemplateDto _rowFor(CoreType core) {
    for (final t in widget.initial) {
      if (t.coreType == core) return t;
    }
    return g.FullConfigTemplateDto(
      id: '',
      remarks: core == CoreType.singBox ? 'sing-box' : 'V2ray',
      enabled: false,
      coreType: core,
    );
  }

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 2, vsync: this);
    _xray = _TemplateTabState(_rowFor(CoreType.xray));
    _singbox = _TemplateTabState(_rowFor(CoreType.singBox));
  }

  @override
  void dispose() {
    _tabs.dispose();
    _xray.dispose();
    _singbox.dispose();
    super.dispose();
  }

  /// A template blob is valid when empty or a JSON object.
  static String? validateTemplateJson(String text) {
    if (text.trim().isEmpty) return null;
    try {
      final value = jsonDecode(text);
      if (value is Map) return null;
      return '需为 JSON 对象';
    } catch (_) {
      return 'JSON 解析失败';
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      key: const ValueKey('template-window'),
      title: const Text('完整配置模板设置', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 680,
        height: 600,
        child: Column(
          children: <Widget>[
            if (_serverError != null)
              Container(
                key: const ValueKey('template-error'),
                margin: const EdgeInsets.only(bottom: 8),
                padding: const EdgeInsets.all(8),
                width: double.infinity,
                decoration: BoxDecoration(
                  color: theme.colorScheme.errorContainer,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  '保存失败: $_serverError',
                  style: TextStyle(
                    fontSize: 12,
                    color: theme.colorScheme.onErrorContainer,
                  ),
                ),
              ),
            TabBar(
              key: const ValueKey('template-tabs'),
              controller: _tabs,
              labelStyle: const TextStyle(fontSize: 13),
              tabs: const <Widget>[
                Tab(text: 'V2ray (Xray)'),
                Tab(text: 'sing-box'),
              ],
            ),
            Expanded(
              child: TabBarView(
                controller: _tabs,
                children: <Widget>[
                  _tab(_xray, 'xray'),
                  _tab(_singbox, 'singbox'),
                ],
              ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('template-cancel'),
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('template-save'),
          onPressed: _submitting ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _tab(_TemplateTabState state, String prefix) {
    return SingleChildScrollView(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          SwitchListTile(
            key: ValueKey('template-enabled-$prefix'),
            dense: true,
            title: const Text('启用完整配置模板', style: TextStyle(fontSize: 12)),
            value: state.item.enabled,
            onChanged: (v) =>
                setState(() => state.item = _copyWith(state.item, enabled: v)),
          ),
          SwitchListTile(
            key: ValueKey('template-add-proxy-only-$prefix'),
            dense: true,
            title: const Text(
              '仅追加代理 (AddProxyOnly)',
              style: TextStyle(fontSize: 12),
            ),
            value: state.item.addProxyOnly ?? false,
            onChanged: (v) => setState(
              () => state.item = _copyWith(state.item, addProxyOnly: v),
            ),
          ),
          TextFormField(
            key: ValueKey('template-detour-$prefix'),
            controller: state.proxyDetour,
            style: const TextStyle(fontSize: 12),
            decoration: const InputDecoration(
              labelText: '代理前置 (ProxyDetour, 空=不使用)',
              isDense: true,
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          TextFormField(
            key: ValueKey('template-config-$prefix'),
            controller: state.config,
            minLines: 8,
            maxLines: 12,
            style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
            decoration: InputDecoration(
              labelText: '完整配置 (Config, JSON 对象)',
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: state.configError,
            ),
            onChanged: (_) => setState(() => state.configError = null),
          ),
          const SizedBox(height: 8),
          TextFormField(
            key: ValueKey('template-tun-$prefix'),
            controller: state.tunConfig,
            minLines: 4,
            maxLines: 8,
            style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
            decoration: InputDecoration(
              labelText: 'TUN 配置 (TunConfig, JSON 对象)',
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: state.tunConfigError,
            ),
            onChanged: (_) => setState(() => state.tunConfigError = null),
          ),
        ],
      ),
    );
  }

  g.FullConfigTemplateDto _copyWith(
    g.FullConfigTemplateDto item, {
    bool? enabled,
    bool? addProxyOnly,
  }) => g.FullConfigTemplateDto(
    id: item.id,
    remarks: item.remarks,
    enabled: enabled ?? item.enabled,
    coreType: item.coreType,
    config: item.config,
    tunConfig: item.tunConfig,
    addProxyOnly: addProxyOnly ?? item.addProxyOnly,
    proxyDetour: item.proxyDetour,
  );

  void _save() {
    setState(() {
      _serverError = null;
      _submitting = true;
      for (final state in <_TemplateTabState>[_xray, _singbox]) {
        state.configError = validateTemplateJson(state.config.text);
        state.tunConfigError = validateTemplateJson(state.tunConfig.text);
      }
    });
    if (_xray.configError != null ||
        _xray.tunConfigError != null ||
        _singbox.configError != null ||
        _singbox.tunConfigError != null) {
      setState(() => _submitting = false);
      return;
    }
    for (final entry in <(_TemplateTabState, String)>[
      (_xray, 'xray'),
      (_singbox, 'singbox'),
    ]) {
      final state = entry.$1;
      final item = g.FullConfigTemplateDto(
        id: state.item.id,
        remarks: state.item.remarks,
        enabled: state.item.enabled,
        coreType: state.item.coreType,
        config: state.config.text.trim().isEmpty ? null : state.config.text,
        tunConfig: state.tunConfig.text.trim().isEmpty
            ? null
            : state.tunConfig.text,
        addProxyOnly: state.item.addProxyOnly,
        proxyDetour: state.proxyDetour.text.trim().isEmpty
            ? null
            : state.proxyDetour.text.trim(),
      );
      final result = widget.onSave(item);
      if (!result.ok) {
        setState(() {
          _submitting = false;
          final error = result.error;
          _serverError = error == null
              ? '${entry.$2}: 保存失败'
              : '${entry.$2}: ${error.messageKey} (${error.code})';
        });
        return;
      }
    }
    Navigator.of(context).pop(true);
  }
}

/// Show the template window; returns `true` when both rows saved.
Future<bool?> showFullConfigTemplateWindow(
  BuildContext context, {
  required List<g.FullConfigTemplateDto> initial,
  required g.TemplateDtoResult Function(g.FullConfigTemplateDto item) onSave,
}) {
  return showDialog<bool>(
    context: context,
    builder: (context) =>
        FullConfigTemplateWindow(initial: initial, onSave: onSave),
  );
}
