import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Storage key of the pass-through text inside `proto_extra.extra`.
const String kCustomConfigKey = 'customConfigText';

/// Browse seam: copy a picked file into the config dir, return its stored name.
typedef CustomImportFile = c.CustomFileResult Function(String sourcePath);

/// Resolve the current data directory (for the external-edit full path).
typedef DataDirLookup = String Function();

/// Pick a source config file; null on cancel.
typedef CustomFilePicker = Future<String?> Function();

/// Open a full path with the OS handler; false when missing/unopenable.
typedef CustomFileOpener = Future<bool> Function(String fullPath);

/// Pick a JSON/YAML custom config file (mirrors the upstream file picker).
Future<String?> pickCustomConfigFile() async {
  const group = XTypeGroup(
    label: '自定义配置',
    extensions: <String>['json', 'yaml', 'yml', 'txt'],
  );
  final file = await openFile(acceptedTypeGroups: <XTypeGroup>[group]);
  return file?.path;
}

/// Open [fullPath] with the OS default handler (upstream
/// `ProcUtils.ProcessStart`). Returns false when the file does not exist.
Future<bool> openConfigFileExternally(String fullPath) async {
  if (fullPath.trim().isEmpty || !File(fullPath).existsSync()) return false;
  if (Platform.isWindows) {
    await Process.run('cmd', <String>['/c', 'start', '', fullPath]);
    return true;
  }
  return false;
}

/// Absolute path used as-is; a bare file name joins
/// `<dataDir>/config/<name>` (upstream `Utils.GetConfigPath`).
String resolveCustomConfigPath({
  required String address,
  required String dataDir,
}) {
  final trimmed = address.trim();
  if (trimmed.isEmpty) return '';
  final absolute =
      RegExp(r'^[A-Za-z]:[\\/]').hasMatch(trimmed) ||
      trimmed.startsWith(r'\\') ||
      trimmed.startsWith('/');
  if (absolute) return trimmed;
  final sep = Platform.pathSeparator;
  final base = dataDir.endsWith('/') || dataDir.endsWith(r'\')
      ? dataDir.substring(0, dataDir.length - 1)
      : dataDir;
  return '$base${sep}config$sep$trimmed';
}

/// Default remarks upstream fills on browse when the field is empty
/// (`import custom@yyyy/MM/dd HH:mm:ss`, outbound adds ` outbound`).
String defaultCustomRemarks(ConfigType type, DateTime now) {
  final prefix = type == ConfigType.outbound
      ? 'import custom outbound'
      : 'import custom';
  String two(int v) => v.toString().padLeft(2, '0');
  return '$prefix@${now.year}/${two(now.month)}/${two(now.day)} '
      '${two(now.hour)}:${two(now.minute)}:${two(now.second)}';
}

c.CustomFileResult _unwiredCustomImport(String _) =>
    throw StateError('customImportFile is not wired');

String _unwiredDataDir() => '';

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
    required this.customImportFile,
    required this.dataDir,
    required this.pickFile,
    required this.openFile,
  });

  final ProfileDraft initial;
  final c.SaveProfileResult Function(c.ProfileDto draft) onSave;
  final CustomImportFile customImportFile;
  final DataDirLookup dataDir;
  final CustomFilePicker pickFile;
  final CustomFileOpener openFile;

  @override
  State<CustomEditorDialog> createState() => _CustomEditorDialogState();
}

class _CustomEditorDialogState extends State<CustomEditorDialog> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late ProfileDraft _draft;
  late TextEditingController _configText;
  late TextEditingController _remarks;
  late TextEditingController _address;
  c.ErrorDto? _serverError;
  String? _configError;
  String? _fileError;
  bool _submitting = false;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _draft = ProfileDraft.fromDto(widget.initial.toDto());
    if (_draft.configType != ConfigType.outbound) {
      _draft.configType = ConfigType.custom;
    }
    _configText = TextEditingController(text: _readCustomText(_draft));
    _remarks = TextEditingController(text: _draft.remarks);
    _address = TextEditingController(text: _draft.address);
  }

  @override
  void dispose() {
    _configText.dispose();
    _remarks.dispose();
    _address.dispose();
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

  /// Browse a source file, copy it into the config dir, then rewrite Address
  /// and (when empty) Remarks, mirroring `AddServer2ViewModel.BrowseServer`.
  /// A cancelled picker touches nothing.
  Future<void> _browse() async {
    String? source;
    try {
      source = await widget.pickFile();
    } catch (_) {
      source = null;
    }
    if (!mounted || source == null || source.trim().isEmpty) return;
    setState(() {
      _fileError = null;
      _busy = true;
    });
    final result = widget.customImportFile(source);
    if (!mounted) return;
    setState(() {
      _busy = false;
      if (!result.ok || result.fileName == null) {
        _fileError =
            '导入失败: ${result.error?.messageKey ?? 'error.import_custom_failed'}';
        return;
      }
      _draft.address = result.fileName!;
      _address.text = result.fileName!;
      if (_draft.remarks.trim().isEmpty) {
        _draft.remarks = defaultCustomRemarks(
          _draft.configType,
          DateTime.now(),
        );
        _remarks.text = _draft.remarks;
      }
    });
  }

  /// Open the stored config file with the OS handler, mirroring
  /// `AddServer2ViewModel.EditServer`.
  Future<void> _edit() async {
    final address = _address.text.trim();
    if (address.isEmpty) {
      setState(() => _fileError = '请先填写配置文件路径');
      return;
    }
    setState(() {
      _fileError = null;
      _busy = true;
    });
    final fullPath = resolveCustomConfigPath(
      address: address,
      dataDir: widget.dataDir(),
    );
    bool opened;
    try {
      opened = await widget.openFile(fullPath);
    } catch (_) {
      opened = false;
    }
    if (!mounted) return;
    setState(() {
      _busy = false;
      if (!opened) _fileError = '文件不存在或无法打开: $fullPath';
    });
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
                  controller: _remarks,
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
                  controller: _address,
                  style: const TextStyle(fontSize: 12),
                  decoration: const InputDecoration(
                    labelText: '配置文件路径 (Address)',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onChanged: (v) => _draft.address = v,
                  validator: (v) => (v ?? '').trim().isEmpty ? '必填' : null,
                ),
                const SizedBox(height: 4),
                Row(
                  children: <Widget>[
                    OutlinedButton.icon(
                      key: const ValueKey('custom-browse'),
                      onPressed: _busy ? null : _browse,
                      icon: const Icon(Icons.folder_open, size: 16),
                      label: const Text('浏览', style: TextStyle(fontSize: 12)),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton.icon(
                      key: const ValueKey('custom-edit'),
                      onPressed: _busy ? null : _edit,
                      icon: const Icon(Icons.edit, size: 16),
                      label: const Text('编辑', style: TextStyle(fontSize: 12)),
                    ),
                  ],
                ),
                if (_fileError != null)
                  Padding(
                    key: const ValueKey('custom-file-error'),
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      _fileError!,
                      style: TextStyle(
                        fontSize: 12,
                        color: theme.colorScheme.error,
                      ),
                    ),
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
  CustomImportFile? customImportFile,
  DataDirLookup? dataDir,
  CustomFilePicker? pickFile,
  CustomFileOpener? openFile,
}) {
  return showDialog<c.ProfileDto>(
    context: context,
    builder: (context) => CustomEditorDialog(
      initial: initial,
      onSave: onSave,
      customImportFile: customImportFile ?? _unwiredCustomImport,
      dataDir: dataDir ?? _unwiredDataDir,
      pickFile: pickFile ?? pickCustomConfigFile,
      openFile: openFile ?? openConfigFileExternally,
    ),
  );
}
