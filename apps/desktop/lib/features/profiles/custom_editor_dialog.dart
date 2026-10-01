import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Storage key of the pass-through text inside `proto_extra.extra`.
const String kCustomConfigKey = 'customConfigText';

/// Modal editor for Custom (2) / Outbound (13) nodes.
///
/// Mirrors `AddServer2ViewModel`: remarks, core adapter (`CoreType`),
/// config file path (`Address`), inline JSON/YAML editor (`customConfigText`),
/// `DisplayLog`, `PreSocksPort` and `IsSingboxEndpoint`. Cancel never
/// persists; save delegates to [onSave] and keeps the form open on rejection.
class CustomEditorDialog extends StatefulWidget {
  const CustomEditorDialog({
    super.key,
    required this.initial,
    required this.onSave,
  });

  final ProfileDraft initial;
  final c.SaveProfileResult Function(c.ProfileDto draft) onSave;

  @override
  State<CustomEditorDialog> createState() => _CustomEditorDialogState();
}

class _CustomEditorDialogState extends State<CustomEditorDialog> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late ProfileDraft _draft;
  late TextEditingController _configText;
  c.ErrorDto? _serverError;
  String? _configError;
  bool _submitting = false;

  @override
  void initState() {
    super.initState();
    _draft = ProfileDraft.fromDto(widget.initial.toDto());
    if (_draft.configType != ConfigType.outbound) {
      _draft.configType = ConfigType.custom;
    }
    _configText = TextEditingController(text: _readCustomText(_draft));
  }

  @override
  void dispose() {
    _configText.dispose();
    super.dispose();
  }

  static String _readCustomText(ProfileDraft draft) {
    try {
      final map = jsonDecode(
        draft.protoExtraJson.isEmpty ? '{}' : draft.protoExtraJson,
      );
      if (map is Map) {
        final value = map[kCustomConfigKey];
        if (value is String) return value;
      }
    } catch (_) {
      // Fall through to empty.
    }
    return '';
  }

  /// Client-side check mirroring the Rust `custom::validate_custom_text`:
  /// a JSON object or a YAML mapping.
  static String? validateConfigText(String text) {
    if (text.trim().isEmpty) return null;
    try {
      final value = jsonDecode(text);
      if (value is Map) return null;
      return '需为 JSON 对象或 YAML 映射';
    } catch (_) {
      // Not JSON; fall through to the YAML heuristic.
    }
    if (_looksYamlMapping(text)) return null;
    return '配置内容无效 (需为 JSON 对象或 YAML 映射)';
  }

  static bool _looksYamlMapping(String text) =>
      !text.trimLeft().startsWith(RegExp(r'[[{]')) && text.contains(':');

  void _writeCustomText() {
    Map<String, dynamic> map = <String, dynamic>{};
    try {
      final decoded = jsonDecode(
        _draft.protoExtraJson.isEmpty ? '{}' : _draft.protoExtraJson,
      );
      if (decoded is Map) {
        map = Map<String, dynamic>.from(decoded);
      }
    } catch (_) {
      map = <String, dynamic>{};
    }
    final text = _configText.text;
    if (text.trim().isEmpty) {
      map.remove(kCustomConfigKey);
    } else {
      map[kCustomConfigKey] = text;
    }
    _draft.protoExtraJson = jsonEncode(map);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      key: const ValueKey('custom-editor'),
      title: Text(
        widget.initial.isNew
            ? '添加 [${_draft.configType.name}]'
            : '编辑 [${_draft.configType.name}]',
        style: const TextStyle(fontSize: 15),
      ),
      content: SizedBox(
        width: 640,
        height: 580,
        child: Form(
          key: _formKey,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                if (_serverError != null)
                  Container(
                    key: const ValueKey('custom-error'),
                    margin: const EdgeInsets.only(bottom: 8),
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: theme.colorScheme.errorContainer,
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text(
                      '保存失败: ${_serverError!.messageKey} (${_serverError!.code})',
                      style: TextStyle(
                        fontSize: 12,
                        color: theme.colorScheme.onErrorContainer,
                      ),
                    ),
                  ),
                if (widget.initial.isNew)
                  DropdownButtonFormField<String>(
                    key: const ValueKey('custom-config-type'),
                    initialValue: _draft.configType.name,
                    isExpanded: true,
                    decoration: const InputDecoration(
                      labelText: '类型',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                    items: const <DropdownMenuItem<String>>[
                      DropdownMenuItem<String>(
                        value: 'custom',
                        child: Text(
                          '自定义配置 (Custom)',
                          style: TextStyle(fontSize: 12),
                        ),
                      ),
                      DropdownMenuItem<String>(
                        value: 'outbound',
                        child: Text(
                          '自定义出站 (Outbound)',
                          style: TextStyle(fontSize: 12),
                        ),
                      ),
                    ],
                    onChanged: (v) => setState(
                      () => _draft.configType = v == 'outbound'
                          ? ConfigType.outbound
                          : ConfigType.custom,
                    ),
                  ),
                DropdownButtonFormField<String>(
                  key: const ValueKey('custom-core-type'),
                  initialValue: _draft.coreType?.name,
                  isExpanded: true,
                  decoration: const InputDecoration(
                    labelText: '适配内核 (AdapterType / CoreType)',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  items: <DropdownMenuItem<String>>[
                    for (final core in CoreType.values)
                      DropdownMenuItem<String>(
                        value: core.name,
                        child: Text(
                          core.name,
                          style: const TextStyle(fontSize: 12),
                        ),
                      ),
                  ],
                  onChanged: (v) {
                    final match = CoreType.values.where((t) => t.name == v);
                    if (match.isNotEmpty) {
                      setState(() => _draft.coreType = match.first);
                    }
                  },
                ),
                TextFormField(
                  key: const ValueKey('custom-remarks'),
                  initialValue: _draft.remarks,
                  style: const TextStyle(fontSize: 12),
                  decoration: const InputDecoration(
                    labelText: '备注',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onChanged: (v) => _draft.remarks = v,
                  validator: (v) => (v ?? '').trim().isEmpty ? '必填' : null,
                ),
                const SizedBox(height: 8),
                TextFormField(
                  key: const ValueKey('custom-address'),
                  initialValue: _draft.address,
                  style: const TextStyle(fontSize: 12),
                  decoration: const InputDecoration(
                    labelText: '配置文件路径 (Address)',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onChanged: (v) => _draft.address = v,
                  validator: (v) => (v ?? '').trim().isEmpty ? '必填' : null,
                ),
                const SizedBox(height: 8),
                TextFormField(
                  key: const ValueKey('custom-config-text'),
                  controller: _configText,
                  minLines: 8,
                  maxLines: 14,
                  style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                  decoration: InputDecoration(
                    labelText: '透传配置 (JSON 对象 / YAML 映射, 空=用文件)',
                    isDense: true,
                    border: const OutlineInputBorder(),
                    errorText: _configError,
                  ),
                  onChanged: (_) => setState(() => _configError = null),
                ),
                const SizedBox(height: 8),
                TextFormField(
                  key: const ValueKey('custom-pre-socks-port'),
                  initialValue: _draft.preSocksPort?.toString() ?? '',
                  keyboardType: TextInputType.number,
                  style: const TextStyle(fontSize: 12),
                  decoration: const InputDecoration(
                    labelText: '前置 SOCKS 端口 (PreSocksPort, 空=不使用)',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onChanged: (v) => _draft.preSocksPort = v.trim().isEmpty
                      ? null
                      : int.tryParse(v.trim()),
                  validator: (v) {
                    final text = (v ?? '').trim();
                    if (text.isEmpty) return null;
                    final parsed = int.tryParse(text);
                    if (parsed == null || parsed < 1 || parsed > 65535) {
                      return '端口需在 1-65535';
                    }
                    return null;
                  },
                ),
                const SizedBox(height: 4),
                SwitchListTile(
                  key: const ValueKey('custom-display-log'),
                  dense: true,
                  title: const Text(
                    '显示日志 (DisplayLog)',
                    style: TextStyle(fontSize: 12),
                  ),
                  value: _draft.displayLog,
                  onChanged: (v) => setState(() => _draft.displayLog = v),
                ),
                CheckboxListTile(
                  key: const ValueKey('custom-singbox-endpoint'),
                  dense: true,
                  tristate: true,
                  title: const Text(
                    'sing-box 端点 (IsSingboxEndpoint, 空=未设置)',
                    style: TextStyle(fontSize: 12),
                  ),
                  value: _draft.isSingboxEndpoint,
                  onChanged: (v) =>
                      setState(() => _draft.isSingboxEndpoint = v),
                ),
              ],
            ),
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('custom-cancel'),
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('custom-save'),
          onPressed: _submitting ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  void _save() {
    setState(() {
      _serverError = null;
      _submitting = true;
      _configError = validateConfigText(_configText.text);
    });
    if (_configError != null) {
      setState(() => _submitting = false);
      return;
    }
    if (!(_formKey.currentState?.validate() ?? false)) {
      setState(() => _submitting = false);
      return;
    }
    _writeCustomText();
    final result = widget.onSave(_draft.toDto());
    if (result.ok && result.profile != null) {
      Navigator.of(context).pop(result.profile);
      return;
    }
    setState(() {
      _submitting = false;
      _serverError = result.error;
    });
  }
}

/// Show the Custom/Outbound editor; returns the saved profile on success.
Future<c.ProfileDto?> showCustomEditor(
  BuildContext context, {
  required ProfileDraft initial,
  required c.SaveProfileResult Function(c.ProfileDto draft) onSave,
}) {
  return showDialog<c.ProfileDto>(
    context: context,
    builder: (context) => CustomEditorDialog(initial: initial, onSave: onSave),
  );
}
