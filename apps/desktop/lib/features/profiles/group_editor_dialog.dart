import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Modal editor for PolicyGroup (101) / ProxyChain (102) nodes.
///
/// Mirrors `AddGroupServerViewModel`: remarks, core, five `MultipleLoad`
/// modes, ordered child list (add/remove/T/U/D/B), subscription child source
/// (`SubChildItems`) plus remarks `Filter`. Cancel never persists; save
/// delegates to [onSave] and keeps the form open on server rejection.
class GroupEditorDialog extends StatefulWidget {
  const GroupEditorDialog({
    super.key,
    required this.initial,
    required this.allProfiles,
    required this.subItems,
    required this.onSave,
  });

  final ProfileDraft initial;
  final List<c.ProfileDto> allProfiles;
  final List<c.SubItemDto> subItems;
  final c.SaveProfileResult Function(c.ProfileDto draft) onSave;

  @override
  State<GroupEditorDialog> createState() => _GroupEditorDialogState();
}

class _GroupEditorDialogState extends State<GroupEditorDialog> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late ProfileDraft _draft;
  late List<String> _childIds;
  String? _selectedCandidate;
  String? _selectedSubId;
  c.ErrorDto? _serverError;
  bool _submitting = false;

  static const List<ConfigType> _groupTypes = <ConfigType>[
    ConfigType.policyGroup,
    ConfigType.proxyChain,
  ];

  /// `EMultipleLoad` numeric value -> label.
  static const Map<int, String> _loadLabels = <int, String>{
    0: 'LeastPing (最低延迟)',
    1: 'Fallback (故障转移)',
    2: 'Random (随机)',
    3: 'RoundRobin (轮询)',
    4: 'LeastLoad (最小负载)',
  };

  @override
  void initState() {
    super.initState();
    _draft = ProfileDraft.fromDto(widget.initial.toDto());
    _draft.configType = _draft.configType == ConfigType.proxyChain
        ? ConfigType.proxyChain
        : ConfigType.policyGroup;
    _draft.coreType = _draft.coreType == CoreType.singBox
        ? CoreType.singBox
        : CoreType.xray;
    _draft.multipleLoad ??= 0;
    _draft.groupType ??= _draft.configType == ConfigType.proxyChain
        ? 'ProxyChain'
        : 'PolicyGroup';
    _childIds = (_draft.childItems ?? '')
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty && s.toLowerCase() != 'self')
        .toList();
    final sub = (_draft.subChildItems ?? '').trim();
    _selectedSubId = sub.isEmpty ? null : sub;
  }

  List<c.ProfileDto> get _candidates {
    final taken = _childIds.toSet();
    return <c.ProfileDto>[
      for (final p in widget.allProfiles)
        if (p.indexId != _draft.indexId &&
            !taken.contains(p.indexId) &&
            _isEligible(p.configType))
          p,
    ];
  }

  static bool _isEligible(ConfigType type) =>
      type != ConfigType.policyGroup &&
      type != ConfigType.proxyChain &&
      type != ConfigType.custom;

  String _labelOf(c.ProfileDto p) =>
      p.remarks.isEmpty ? p.indexId : '${p.remarks} [${p.configType.name}]';

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      key: const ValueKey('group-editor'),
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
                    key: const ValueKey('group-error'),
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
                  _dropdown<String>(
                    key: 'group-config-type',
                    label: '组类型',
                    value: _draft.configType.name,
                    options: <String>[for (final t in _groupTypes) t.name],
                    onChanged: (v) {
                      final match = _groupTypes.where((t) => t.name == v);
                      if (match.isNotEmpty) {
                        setState(() {
                          _draft.configType = match.first;
                          _draft.groupType =
                              match.first == ConfigType.proxyChain
                              ? 'ProxyChain'
                              : 'PolicyGroup';
                        });
                      }
                    },
                  ),
                TextFormField(
                  key: const ValueKey('group-remarks'),
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
                _dropdown<String>(
                  key: 'group-core-type',
                  label: '内核 (CoreType)',
                  value: _draft.coreType?.name ?? CoreType.xray.name,
                  options: <String>[CoreType.xray.name, CoreType.singBox.name],
                  onChanged: (v) {
                    final match = CoreType.values.where((t) => t.name == v);
                    if (match.isNotEmpty) {
                      setState(() => _draft.coreType = match.first);
                    }
                  },
                ),
                _dropdown<int>(
                  key: 'group-multiple-load',
                  label: '策略模式 (MultipleLoad)',
                  value: _draft.multipleLoad ?? 0,
                  options: _loadLabels.keys.toList(),
                  labels: _loadLabels,
                  onChanged: (v) {
                    if (v != null) setState(() => _draft.multipleLoad = v);
                  },
                ),
                const Padding(
                  padding: EdgeInsets.only(top: 12, bottom: 4),
                  child: Text(
                    '子节点 (ChildItems, 保序)',
                    style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
                  ),
                ),
                _addRow(),
                const SizedBox(height: 4),
                ..._childIds.map(_childTile),
                const Padding(
                  padding: EdgeInsets.only(top: 12, bottom: 4),
                  child: Text(
                    '订阅子项 (SubChildItems + Filter)',
                    style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
                  ),
                ),
                _dropdown<String?>(
                  key: 'group-sub-child',
                  label: '订阅分组 (空=不使用)',
                  value: _selectedSubId,
                  options: <String?>[
                    null,
                    for (final s in widget.subItems) s.id,
                  ],
                  labels: <String?, String>{
                    null: '(不使用)',
                    for (final s in widget.subItems)
                      s.id: s.remarks.isEmpty ? s.id : s.remarks,
                  },
                  onChanged: (v) => setState(() => _selectedSubId = v),
                ),
                TextFormField(
                  key: const ValueKey('group-filter'),
                  initialValue: _draft.filter ?? '',
                  style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                  decoration: const InputDecoration(
                    labelText: 'Filter (备注正则, 空=全部)',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onChanged: (v) => _draft.filter = v,
                ),
              ],
            ),
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('group-cancel'),
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('group-save'),
          onPressed: _submitting ? null : _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _addRow() {
    final candidates = _candidates;
    return Row(
      children: <Widget>[
        Expanded(
          child: DropdownButtonFormField<String>(
            key: const ValueKey('group-add-select'),
            initialValue: _selectedCandidate,
            isExpanded: true,
            decoration: const InputDecoration(
              labelText: '选择节点加入',
              isDense: true,
              border: OutlineInputBorder(),
            ),
            items: <DropdownMenuItem<String>>[
              for (final p in candidates)
                DropdownMenuItem<String>(
                  value: p.indexId,
                  child: Text(
                    _labelOf(p),
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
            ],
            onChanged: (v) => setState(() => _selectedCandidate = v),
          ),
        ),
        const SizedBox(width: 8),
        OutlinedButton(
          key: const ValueKey('group-add'),
          onPressed: () {
            final id = _selectedCandidate;
            if (id == null || id.isEmpty) return;
            setState(() {
              _childIds.add(id);
              _selectedCandidate = null;
            });
          },
          child: const Text('加入'),
        ),
      ],
    );
  }

  Widget _childTile(String id) {
    c.ProfileDto? profile;
    for (final p in widget.allProfiles) {
      if (p.indexId == id) {
        profile = p;
        break;
      }
    }
    final index = _childIds.indexOf(id);
    return ListTile(
      key: ValueKey('group-child-$id'),
      dense: true,
      title: Text(
        profile == null ? id : _labelOf(profile),
        style: const TextStyle(fontSize: 12),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          for (final entry in <(String, String, void Function())>[
            ('T', '置顶', () => _move(index, 0)),
            ('U', '上移', () => _move(index, index - 1)),
            ('D', '下移', () => _move(index, index + 1)),
            ('B', '置底', () => _move(index, _childIds.length - 1)),
          ])
            IconButton(
              key: ValueKey('group-${entry.$1.toLowerCase()}-$id'),
              tooltip: entry.$2,
              iconSize: 16,
              visualDensity: VisualDensity.compact,
              onPressed: entry.$3,
              icon: Text(entry.$1, style: const TextStyle(fontSize: 11)),
            ),
          IconButton(
            key: ValueKey('group-remove-$id'),
            tooltip: '移除',
            iconSize: 16,
            visualDensity: VisualDensity.compact,
            onPressed: () => setState(() => _childIds.remove(id)),
            icon: const Icon(Icons.close, size: 14),
          ),
        ],
      ),
    );
  }

  void _move(int from, int to) {
    if (from < 0 || from >= _childIds.length) return;
    final clamped = to.clamp(0, _childIds.length - 1);
    if (clamped == from) return;
    setState(() {
      final id = _childIds.removeAt(from);
      _childIds.insert(clamped, id);
    });
  }

  Widget _dropdown<T>({
    required String key,
    required String label,
    required T? value,
    required List<T> options,
    Map<T, String>? labels,
    required ValueChanged<T?> onChanged,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: DropdownButtonFormField<T>(
        key: ValueKey(key),
        initialValue: value,
        isExpanded: true,
        decoration: InputDecoration(
          labelText: label,
          isDense: true,
          border: const OutlineInputBorder(),
        ),
        items: <DropdownMenuItem<T>>[
          for (final option in options)
            DropdownMenuItem<T>(
              value: option,
              child: Text(
                labels?[option] ?? '$option',
                style: const TextStyle(fontSize: 12),
              ),
            ),
        ],
        onChanged: onChanged,
      ),
    );
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
    if (_childIds.isEmpty &&
        (_selectedSubId == null || _selectedSubId!.isEmpty)) {
      setState(() {
        _submitting = false;
        _serverError = const c.ErrorDto(
          code: 'E_FIELD_REQUIRED',
          messageKey: 'error.group_children_required',
          fieldPath: 'proto_extra.childItems',
          retryable: false,
        );
      });
      return;
    }
    _draft.childItems = _childIds.isEmpty ? null : _childIds.join(',');
    _draft.subChildItems = _selectedSubId;
    if ((_draft.filter ?? '').trim().isEmpty) _draft.filter = null;
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

/// Show the group editor; returns the saved profile, or `null` on cancel.
Future<c.ProfileDto?> showGroupEditor(
  BuildContext context, {
  required ProfileDraft initial,
  required List<c.ProfileDto> allProfiles,
  required List<c.SubItemDto> subItems,
  required c.SaveProfileResult Function(c.ProfileDto draft) onSave,
}) {
  return showDialog<c.ProfileDto>(
    context: context,
    builder: (context) => GroupEditorDialog(
      initial: initial,
      allProfiles: allProfiles,
      subItems: subItems,
      onSave: onSave,
    ),
  );
}

/// `group_children` result ids in order (test helper).
List<String> groupChildIds(c.ProfilePageDto page) => <String>[
  for (final p in page.items) p.indexId,
];
