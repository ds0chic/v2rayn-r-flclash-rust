import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';

/// Modal editor for one node of any of the 11 basic protocol kinds.
///
/// Cancel never persists (the draft is local); save delegates to [onSave] and
/// keeps the form open with the server error when a field is rejected.
class ProfileEditorDialog extends StatefulWidget {
  const ProfileEditorDialog({
    super.key,
    required this.initial,
    required this.onSave,
    this.allowConfigTypeChange = true,
    this.showActiveToggle = false,
  });

  final ProfileDraft initial;
  final c.SaveProfileResult Function(c.ProfileDto draft) onSave;
  final bool allowConfigTypeChange;
  final bool showActiveToggle;

  @override
  State<ProfileEditorDialog> createState() => _ProfileEditorDialogState();
}

class _ProfileEditorDialogState extends State<ProfileEditorDialog> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late ProfileDraft _draft;
  c.ErrorDto? _serverError;
  final Map<String, String> _fieldErrors = <String, String>{};
  bool _submitting = false;

  static const List<ConfigType> _basicTypes = <ConfigType>[
    ConfigType.vmess,
    ConfigType.vless,
    ConfigType.shadowsocks,
    ConfigType.socks,
    ConfigType.http,
    ConfigType.trojan,
    ConfigType.hysteria2,
    ConfigType.tuic,
    ConfigType.wireGuard,
    ConfigType.anytls,
    ConfigType.naive,
  ];

  @override
  void initState() {
    super.initState();
    _draft = ProfileDraft.fromDto(widget.initial.toDto());
    // Only the two structured-generation cores are selectable for basic
    // protocols; a persisted value outside the allowed set (e.g. a synthetic
    // fixture's v2fly) is clamped so the dropdown stays consistent.
    final allowed = ProfileCapabilities.allowedCores(_draft.configType);
    if (_draft.coreType == null || !allowed.contains(_draft.coreType)) {
      _draft.coreType = ProfileCapabilities.defaultCore(_draft.configType);
    }
    if (_draft.port <= 0) _draft.port = 443;
    // Clamp a persisted network (e.g. synthetic "tcp") to the protocol's set.
    final networks = ProfileCapabilities.allowedNetworks(_draft.configType);
    if (!networks.contains(_draft.network)) _draft.network = 'raw';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      key: const ValueKey('profile-editor'),
      title: Text(
        widget.initial.isNew
            ? '添加 [${_draft.configType.name}]'
            : '编辑 [${_draft.configType.name}]',
        style: const TextStyle(fontSize: 15),
      ),
      content: SizedBox(
        width: 620,
        height: 560,
        child: Form(
          key: _formKey,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                if (_serverError != null) _errorBanner(theme, _serverError!),
                _section('基础', _topFields()),
                _section('协议', protocolFields(_draft.configType)),
                _section('传输', <FieldSpec>[
                  _networkField(),
                  ...transportFields(_draft.network),
                ]),
                _section(
                  'TLS / Reality',
                  securityFields(_draft.streamSecurity),
                ),
              ],
            ),
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('editor-cancel'),
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('editor-save'),
          onPressed: _submitting ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _errorBanner(ThemeData theme, c.ErrorDto error) {
    return Container(
      key: const ValueKey('editor-error'),
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: theme.colorScheme.errorContainer,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        '保存失败: ${error.messageKey} (${error.code})',
        style: TextStyle(
          fontSize: 12,
          color: theme.colorScheme.onErrorContainer,
        ),
      ),
    );
  }

  Widget _section(String title, List<FieldSpec> fields) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 4),
          child: Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
          ),
        ),
        for (final field in fields) _buildField(field),
      ],
    );
  }

  List<FieldSpec> _topFields() {
    return <FieldSpec>[
      if (widget.allowConfigTypeChange && widget.initial.isNew)
        FieldSpec(
          key: 'configType',
          label: '协议类型',
          kind: FieldKind.dropdown,
          get: (d) => d.configType.name,
          set: (d, v) {
            final match = _basicTypes.where((t) => t.name == v);
            if (match.isNotEmpty) {
              setState(() {
                _draft.configType = match.first;
                _draft.coreType = ProfileCapabilities.defaultCore(match.first);
                if (!ProfileCapabilities.allowedNetworks(match.first)
                    .contains(_draft.network)) {
                  _draft.network = 'raw';
                }
              });
            }
          },
          options: <FieldOption>[
            for (final t in _basicTypes) FieldOption(t.name, t.name),
          ],
        ),
      FieldSpec(
        key: 'coreType',
        label: '内核 (CoreType)',
        kind: FieldKind.dropdown,
        get: (d) => d.coreType?.name,
        set: (d, v) {
          final match = CoreType.values.where((t) => t.name == v);
          if (match.isNotEmpty) setState(() => _draft.coreType = match.first);
        },
        options: <FieldOption>[
          for (final core in ProfileCapabilities.allowedCores(
            _draft.configType,
          ))
            FieldOption(core.name, core.name),
        ],
      ),
      FieldSpec(
        key: 'remarks',
        label: '备注',
        kind: FieldKind.text,
        get: (d) => d.remarks,
        set: (d, v) => d.remarks = v ?? '',
        required: true,
      ),
      FieldSpec(
        key: 'address',
        label: '地址',
        kind: FieldKind.text,
        get: (d) => d.address,
        set: (d, v) => d.address = v ?? '',
        required: true,
      ),
      FieldSpec(
        key: 'port',
        label: '端口',
        kind: FieldKind.intField,
        get: (d) => d.port.toString(),
        set: (d, v) => d.port = int.tryParse(v ?? '') ?? 0,
        required: true,
      ),
    ];
  }

  FieldSpec _networkField() => FieldSpec(
    key: 'network',
    label: '传输方式 (Network)',
    kind: FieldKind.dropdown,
    get: (d) => d.network,
    set: (d, v) => setState(() => _draft.network = v ?? 'raw'),
    options: <FieldOption>[
      for (final n in ProfileCapabilities.allowedNetworks(_draft.configType))
        FieldOption(n, n),
    ],
  );

  Widget _buildField(FieldSpec spec) {
    final value = spec.get(_draft);
    final fieldError = _fieldErrors[spec.key];
    switch (spec.kind) {
      case FieldKind.dropdown:
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: DropdownButtonFormField<String?>(
            key: ValueKey('field-${spec.key}'),
            initialValue: value,
            isExpanded: true,
            decoration: InputDecoration(
              labelText: spec.label,
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: fieldError,
            ),
            items: <DropdownMenuItem<String?>>[
              for (final option in spec.options ?? const <FieldOption>[])
                DropdownMenuItem<String?>(
                  value: option.value,
                  child: Text(
                    option.label,
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
            ],
            onChanged: (v) {
              _fieldErrors.remove(spec.key);
              spec.set(_draft, v);
            },
            validator: (_) => _validate(spec, spec.get(_draft)),
          ),
        );
      case FieldKind.boolField:
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: DropdownButtonFormField<String?>(
            key: ValueKey('field-${spec.key}'),
            initialValue: value,
            isExpanded: true,
            decoration: InputDecoration(
              labelText: spec.label,
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: fieldError,
            ),
            items: const <DropdownMenuItem<String?>>[
              DropdownMenuItem<String?>(value: null, child: Text('(未设置)')),
              DropdownMenuItem<String?>(value: 'true', child: Text('启用')),
              DropdownMenuItem<String?>(value: 'false', child: Text('禁用')),
            ],
            onChanged: (v) {
              _fieldErrors.remove(spec.key);
              spec.set(_draft, v);
            },
          ),
        );
      case FieldKind.multiline:
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: TextFormField(
            key: ValueKey('field-${spec.key}'),
            initialValue: value,
            minLines: 2,
            maxLines: 4,
            style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
            decoration: InputDecoration(
              labelText: spec.label,
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: fieldError,
            ),
            onChanged: (v) => spec.set(_draft, v),
            validator: (_) => _validate(spec, spec.get(_draft)),
          ),
        );
      case FieldKind.text:
      case FieldKind.password:
      case FieldKind.intField:
        return Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: TextFormField(
            key: ValueKey('field-${spec.key}'),
            initialValue: value,
            obscureText: spec.kind == FieldKind.password,
            keyboardType: spec.kind == FieldKind.intField
                ? TextInputType.number
                : TextInputType.text,
            style: const TextStyle(fontSize: 12),
            decoration: InputDecoration(
              labelText: spec.label,
              hintText: spec.hint,
              isDense: true,
              border: const OutlineInputBorder(),
              errorText: fieldError,
            ),
            onChanged: (v) {
              _fieldErrors.remove(spec.key);
              spec.set(_draft, v);
            },
            validator: (_) => _validate(spec, spec.get(_draft)),
          ),
        );
    }
  }

  String? _validate(FieldSpec spec, String? value) {
    final text = value?.trim() ?? '';
    if (spec.required && text.isEmpty) return '必填';
    if (spec.kind == FieldKind.intField && text.isNotEmpty) {
      final parsed = int.tryParse(text);
      if (parsed == null) return '需为整数';
      if (spec.key == 'port' && (parsed < 1 || parsed > 65535)) {
        return '端口需在 1-65535';
      }
    }
    if (_draft.streamSecurity == 'reality' &&
        spec.key == 'publicKey' &&
        text.isEmpty) {
      return 'Reality 必填';
    }
    return null;
  }

  void _save() {
    setState(() {
      _serverError = null;
      _submitting = true;
    });
    if (!(_formKey.currentState?.validate() ?? false)) {
      setState(() => _submitting = false);
      return;
    }
    final result = widget.onSave(_draft.toDto());
    if (result.ok && result.profile != null) {
      Navigator.of(context).pop(result.profile);
      return;
    }
    setState(() {
      _submitting = false;
      _serverError = result.error;
      _fieldErrors.clear();
      final path = result.error?.fieldPath;
      if (path != null) {
        _fieldErrors[path] = result.error?.messageKey ?? '字段无效';
      }
    });
  }
}

/// Show the editor and return the saved profile, or `null` on cancel.
Future<c.ProfileDto?> showProfileEditor(
  BuildContext context, {
  required ProfileDraft initial,
  required c.SaveProfileResult Function(c.ProfileDto draft) onSave,
  bool allowConfigTypeChange = true,
}) {
  return showDialog<c.ProfileDto>(
    context: context,
    builder: (context) => ProfileEditorDialog(
      initial: initial,
      onSave: onSave,
      allowConfigTypeChange: allowConfigTypeChange,
    ),
  );
}
