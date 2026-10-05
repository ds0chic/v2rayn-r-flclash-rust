import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_fields.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

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
    // Upstream `AddHysteria2Server`/`AddTuicServer`/`AddAnytlsServer`/
    // `AddNaiveServer` force `Global.StreamSecurity` ("tls") when the imported
    // value is empty; never touch a non-empty (imported) value so a remarks-only
    // save stays lossless.
    if (const <ConfigType>{
          ConfigType.hysteria2,
          ConfigType.tuic,
          ConfigType.anytls,
          ConfigType.naive,
        }.contains(_draft.configType) &&
        (_draft.streamSecurity == null || _draft.streamSecurity!.isEmpty)) {
      _draft.streamSecurity = 'tls';
    }
    // Upstream `AddTuicServer` defaults ALPN to "h3".
    if (_draft.configType == ConfigType.tuic &&
        (_draft.alpn?.isEmpty ?? true)) {
      _draft.alpn = 'h3';
    }
    // Upstream `AddWireguardServer` defaults MTU to `Global.TunMtus.First()`.
    if (_draft.configType == ConfigType.wireGuard &&
        (_draft.wgMtu == null || _draft.wgMtu! <= 0)) {
      _draft.wgMtu = 1280;
    }
    // Upstream `AddServerViewModel` fills protocol defaults on every open:
    // VMess empty security -> Global.DefaultSecurity ("auto"); VLESS empty
    // encryption -> Global.None ("none"). Imported/edited nodes get the same
    // treatment, so a save never persists a blank security/encryption.
    if (_draft.configType == ConfigType.vmess &&
        (_draft.vmessSecurity?.isEmpty ?? true)) {
      _draft.vmessSecurity = 'auto';
    }
    if (_draft.configType == ConfigType.vless &&
        (_draft.vlessEncryption?.isEmpty ?? true)) {
      _draft.vlessEncryption = 'none';
    }
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
                _section(theme, '基础', _topFields()),
                _section(
                  theme,
                  '协议',
                  protocolFields(_draft.configType, coreType: _draft.coreType),
                ),
                // Upstream collapses `gridTransport` for Hysteria2/TUIC/
                // WireGuard/Anytls/Naive.
                if (ProfileCapabilities.supportsTransport(_draft.configType))
                  _section(theme, '传输', <FieldSpec>[
                    _networkField(),
                    ...transportFields(_draft.network),
                  ]),
                // Upstream collapses `gridTls` only for WireGuard.
                if (ProfileCapabilities.supportsTls(_draft.configType) ||
                    ProfileCapabilities.supportsReality(_draft.configType))
                  _section(
                    theme,
                    'TLS / Reality',
                    securityFields(
                      _draft.streamSecurity,
                      finalmask: ProfileCapabilities.supportsFinalmask(
                        _draft.configType,
                      ),
                      realityAllowed: ProfileCapabilities.supportsReality(
                        _draft.configType,
                      ),
                    ),
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
      margin: const EdgeInsets.only(bottom: AppForm.groupGap),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: theme.colorScheme.errorContainer,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        '保存失败: ${error.messageKey} (${error.code})',
        style: AppForm.contentStyle(theme)
            .copyWith(color: theme.colorScheme.onErrorContainer),
      ),
    );
  }

  Widget _section(ThemeData theme, String title, List<FieldSpec> fields) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(
            top: AppForm.groupGap,
            bottom: AppForm.sectionTitleGap,
          ),
          child: Text(
            title,
            key: ValueKey('section-$title'),
            style: AppForm.sectionTitleStyle(theme),
          ),
        ),
        for (var i = 0; i < fields.length; i++) ...<Widget>[
          if (i > 0) const SizedBox(height: AppForm.fieldGap),
          _buildField(theme, fields[i]),
        ],
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

  /// One label/control row. The label lives in a fixed column so long labels
  /// wrap; the row is top-aligned so a wrapping label never pushes the control
  /// down relative to the field above.
  Widget _buildField(ThemeData theme, FieldSpec spec) {
    return Row(
      key: ValueKey('row-${spec.key}'),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SizedBox(
          width: AppForm.labelColumnWidth,
          child: Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              spec.label,
              key: ValueKey('label-${spec.key}'),
              style: AppForm.labelStyle(theme),
            ),
          ),
        ),
        const SizedBox(width: AppForm.labelControlGap),
        Expanded(child: _buildControl(theme, spec)),
      ],
    );
  }

  Widget _buildControl(ThemeData theme, FieldSpec spec) {
    final value = spec.get(_draft);
    final fieldError = _fieldErrors[spec.key];
    final contentStyle = AppForm.contentStyle(theme);
    InputDecoration decoration({String? hint, Widget? suffix}) =>
        InputDecoration(
          hintText: hint,
          hintStyle: AppForm.helperStyle(theme),
          isDense: true,
          contentPadding: AppForm.controlPadding,
          border: const OutlineInputBorder(),
          errorText: fieldError,
          errorStyle: AppForm.errorStyle(theme),
          suffixIcon: suffix,
        );
    switch (spec.kind) {
      case FieldKind.dropdown:
        return DropdownButtonFormField<String?>(
          key: ValueKey('field-${spec.key}'),
          initialValue: _dropdownValue(spec, value),
          isExpanded: true,
          style: contentStyle,
          decoration: decoration(),
          items: _dropdownItems(spec, value),
          onChanged: (v) {
            _fieldErrors.remove(spec.key);
            setState(() {
              spec.set(_draft, v);
              if (spec.key == 'cert') _syncCertSha();
            });
          },
          validator: (_) => _validate(spec, spec.get(_draft)),
        );
      case FieldKind.combo:
        return _ComboField(
          fieldKey: ValueKey('field-${spec.key}'),
          menuKey: ValueKey('combo-${spec.key}'),
          initialValue: value,
          options: spec.options ?? const <FieldOption>[],
          style: contentStyle,
          decoration: decoration(hint: spec.hint),
          onChanged: (v) {
            _fieldErrors.remove(spec.key);
            spec.set(_draft, v);
          },
          validator: (_) => _validate(spec, spec.get(_draft)),
        );
      case FieldKind.boolField:
        return DropdownButtonFormField<String?>(
          key: ValueKey('field-${spec.key}'),
          initialValue: value,
          isExpanded: true,
          style: contentStyle,
          decoration: decoration(),
          items: const <DropdownMenuItem<String?>>[
            DropdownMenuItem<String?>(value: null, child: Text('(未设置)')),
            DropdownMenuItem<String?>(value: 'true', child: Text('启用')),
            DropdownMenuItem<String?>(value: 'false', child: Text('禁用')),
          ],
          onChanged: (v) {
            _fieldErrors.remove(spec.key);
            setState(() => spec.set(_draft, v));
          },
        );
      case FieldKind.multiline:
        return TextFormField(
          key: ValueKey('field-${spec.key}'),
          initialValue: value,
          minLines: 2,
          maxLines: 4,
          style: contentStyle.copyWith(fontFamily: AppTokens.monoFontFamily),
          decoration: decoration(hint: spec.hint),
          onChanged: (v) {
            spec.set(_draft, v);
            if (spec.key == 'cert') _syncCertSha();
          },
          validator: (_) => _validate(spec, spec.get(_draft)),
        );
      case FieldKind.text:
      case FieldKind.password:
      case FieldKind.intField:
        return TextFormField(
          key: ValueKey('field-${spec.key}'),
          initialValue: value,
          obscureText: spec.kind == FieldKind.password,
          keyboardType: spec.kind == FieldKind.intField
              ? TextInputType.number
              : TextInputType.text,
          style: contentStyle,
          decoration: decoration(
            hint: spec.hint,
            suffix: _isUuidField(spec)
                ? _uuidButton(spec)
                : (spec.key == 'cert' ? _certButton() : null),
          ),
          onChanged: (v) {
            _fieldErrors.remove(spec.key);
            spec.set(_draft, v);
            if (spec.key == 'cert') _syncCertSha();
          },
          validator: (_) => _validate(spec, spec.get(_draft)),
        );
    }
  }

  bool _isUuidField(FieldSpec spec) =>
      spec.kind == FieldKind.text &&
      spec.key == uuidFieldKey(_draft.configType);

  /// The upstream "生成" (TbGUID) button regenerates the UUID field in place.
  Widget _uuidButton(FieldSpec spec) {
    return Padding(
      padding: const EdgeInsets.only(right: 6),
      child: TextButton(
        key: ValueKey('gen-uuid-${spec.key}'),
        onPressed: () => setState(() => spec.set(_draft, generateUuidV4())),
        child: const Text('生成'),
      ),
    );
  }

  /// Upstream `btnFetchCert` / `btnFetchCertChain` next to the certificate box.
  Widget _certButton() {
    return PopupMenuButton<String>(
      key: const ValueKey('fetch-cert-menu'),
      tooltip: '获取证书',
      onSelected: (value) => _fetchCert(chain: value == 'chain'),
      itemBuilder: (context) => const <PopupMenuEntry<String>>[
        PopupMenuItem<String>(value: 'leaf', child: Text('获取证书')),
        PopupMenuItem<String>(value: 'chain', child: Text('获取证书链')),
      ],
    );
  }

  Future<void> _fetchCert({required bool chain}) async {
    final host = _draft.address.trim();
    final port = _draft.port;
    if (host.isEmpty || port < 1 || port > 65535) {
      _showSnack('请先填写有效的地址与端口');
      return;
    }
    // Upstream `FetchCert`/`FetchCertChain`: SNI -> transport host -> address.
    final serverName = selectCertFetchServerName(
      sni: _draft.sni,
      transportHost: _draft.host,
      address: host,
    );
    try {
      if (chain) {
        final result = await fetchPeerCertChainPem(
          host: host,
          port: port,
          serverName: serverName,
        );
        if (!mounted) return;
        if (result == null) {
          _showSnack('未能获取证书');
          return;
        }
        setState(() {
          _draft.cert = result.pem;
          _syncCertSha();
        });
        _showSnack(result.leafOnly ? '已获取证书（仅叶子；该平台无法获取完整链）' : '已获取证书链');
        return;
      }
      final pem = await fetchPeerCertPem(
        host: host,
        port: port,
        serverName: serverName,
      );
      if (!mounted) return;
      if (pem == null) {
        _showSnack('未能获取证书');
        return;
      }
      setState(() {
        _draft.cert = pem;
        _syncCertSha();
      });
      _showSnack('已获取证书');
    } on Object catch (e) {
      if (!mounted) return;
      _showSnack('获取证书失败: $e');
    }
  }

  void _showSnack(String message) {
    ScaffoldMessenger.maybeOf(context)
        ?.showSnackBar(SnackBar(content: Text(message)));
  }

  /// Valid `initialValue` for a dropdown. A stored value not in the candidate
  /// list is preserved (see [_dropdownItems]), so it stays visible/editable and
  /// Flutter does not assert.
  String? _dropdownValue(FieldSpec spec, String? value) => value;

  List<DropdownMenuItem<String?>> _dropdownItems(
    FieldSpec spec,
    String? value,
  ) {
    final options = spec.options ?? const <FieldOption>[];
    final items = <DropdownMenuItem<String?>>[
      for (final option in options)
        DropdownMenuItem<String?>(
          value: option.value,
          child: Text(option.label),
        ),
    ];
    final known = options.any((o) => o.value == value);
    if (value != null && !known) {
      items.insert(
        0,
        DropdownMenuItem<String?>(value: value, child: Text('$value (未知候选)')),
      );
    }
    return items;
  }

  /// Recompute CertSha whenever the Cert text changes (upstream `UpdateCertSha`).
  void _syncCertSha() {
    final cert = _draft.cert;
    if (cert == null || cert.isEmpty) return;
    final sha = certShaFromChain(cert);
    if (sha != null) _draft.certSha = sha;
  }

  static final RegExp _uuidPattern = RegExp(
    r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-'
    r'[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$',
  );

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
    if (spec.key == uuidFieldKey(_draft.configType) &&
        text.isNotEmpty &&
        !_uuidPattern.hasMatch(text)) {
      return 'UUID 格式无效';
    }
    if (_draft.streamSecurity == 'reality' &&
        spec.key == 'publicKey' &&
        text.isEmpty) {
      return 'Reality 必填';
    }
    if (spec.key == 'ssMethod' &&
        _draft.configType == ConfigType.shadowsocks &&
        text.isNotEmpty &&
        !shadowsocksMethods(_draft.coreType).contains(text)) {
      return '当前内核不支持该加密方式';
    }
    // Upstream `AddServerViewModel.SaveServerAsync`: non-empty HTTP headers
    // must parse as JSON, otherwise the save is rejected (the codegen would
    // otherwise silently drop an unparsable header string).
    if (spec.key == 'httpHeaders' &&
        _draft.configType == ConfigType.http &&
        text.isNotEmpty) {
      try {
        jsonDecode(text);
      } on FormatException {
        return 'JSON 格式无效';
      }
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

/// Editable dropdown used by fields upstream marks `IsEditable="True"`.
///
/// A free-text [TextFormField] carries the value; a suffix menu offers the
/// candidate list. Any stored value (including one not in the list) is kept.
class _ComboField extends StatelessWidget {
  const _ComboField({
    required this.fieldKey,
    required this.menuKey,
    required this.initialValue,
    required this.options,
    required this.style,
    required this.decoration,
    required this.onChanged,
    required this.validator,
  });

  final Key fieldKey;
  final Key menuKey;
  final String? initialValue;
  final List<FieldOption> options;
  final TextStyle style;
  final InputDecoration decoration;
  final ValueChanged<String?> onChanged;
  final FormFieldValidator<String> validator;

  @override
  Widget build(BuildContext context) {
    final controller = TextEditingController(text: initialValue ?? '');
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Expanded(
          child: TextFormField(
            key: fieldKey,
            controller: controller,
            style: style,
            decoration: decoration,
            onChanged: onChanged,
            validator: validator,
          ),
        ),
        PopupMenuButton<String?>(
          key: menuKey,
          tooltip: '候选值',
          onSelected: (v) {
            controller.text = v ?? '';
            onChanged(v);
          },
          itemBuilder: (context) => <PopupMenuEntry<String?>>[
            for (final option in options)
              PopupMenuItem<String?>(
                value: option.value,
                child: Text(option.label),
              ),
          ],
        ),
      ],
    );
  }
}
