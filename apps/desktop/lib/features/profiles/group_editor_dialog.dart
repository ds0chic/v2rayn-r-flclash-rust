import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Modal editor for PolicyGroup (101) / ProxyChain (102) nodes.
///
/// Mirrors `AddGroupServerViewModel`: remarks, core, five `MultipleLoad`
/// modes, ordered child list (multi-select add, remove, T/U/D/B), subscription
/// child source (`SubChildItems`) plus remarks `Filter`, and a persisted
/// resolution preview (`group_children`). Cancel never persists; save
/// delegates to [onSave] and keeps the form open on server rejection.
class GroupEditorDialog extends StatefulWidget {
  const GroupEditorDialog({
    super.key,
    required this.initial,
    required this.allProfiles,
    required this.subItems,
    required this.onSave,
    this.previewChildren,
  });

  final ProfileDraft initial;
  final List<c.ProfileDto> allProfiles;
  final List<c.SubItemDto> subItems;
  final c.SaveProfileResult Function(c.ProfileDto draft) onSave;

  /// Persisted child resolution for [indexId] (subscription matches first,
  /// then explicit order). Injected so widget tests can stub it; production
  /// passes `ProfilesController.groupChildPreview`.
  final List<c.ProfileDto> Function(String indexId)? previewChildren;

  @override
  State<GroupEditorDialog> createState() => _GroupEditorDialogState();
}

class _GroupEditorDialogState extends State<GroupEditorDialog> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late ProfileDraft _draft;
  late List<String> _childIds;
  String? _selectedSubId;
  List<c.ProfileDto> _preview = const <c.ProfileDto>[];
  bool _previewLoaded = false;
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
    _refreshPreview();
  }

  /// Reload the persisted child resolution. It reflects the stored node, not
  /// the in-progress draft: recompute after saving to confirm the new order.
  void _refreshPreview() {
    final preview = widget.previewChildren;
    if (preview == null || _draft.indexId.isEmpty) {
      _preview = const <c.ProfileDto>[];
      _previewLoaded = preview != null;
      return;
    }
    _preview = preview(_draft.indexId);
    _previewLoaded = true;
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

  /// Manual child selection excludes only Custom, exactly like upstream
  /// `AddGroupServerViewModel.AddChildAsync`
  /// (`SetConfigTypeFilter([EConfigType.Custom], exclude: true)`): nested
  /// PolicyGroup/ProxyChain and Outbound nodes stay selectable, and a cycle
  /// through nested groups is rejected at save time by the Rust
  /// `validate_group` ring check. (The subscription-derived `SubChildItems`
  /// resolution is the stricter rule that drops complex nodes; it does not
  /// apply to hand-picked `ChildItems`.)
  static bool _isEligible(ConfigType type) => type != ConfigType.custom;

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
                if (widget.previewChildren != null) _previewSection(),
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

  /// Open the multi-select node picker (upstream `ProfilesSelectWindow`:
  /// multi-select with select-all, Custom excluded) and append the chosen ids
  /// in list order, skipping duplicates.
  Future<void> _pickNodes() async {
    final picked = await showNodePicker(context, candidates: _candidates);
    if (picked == null || picked.isEmpty) return;
    setState(() {
      final taken = _childIds.toSet();
      for (final id in picked) {
        if (taken.add(id)) _childIds.add(id);
      }
    });
  }

  Widget _addRow() {
    return Row(
      children: <Widget>[
        OutlinedButton.icon(
          key: const ValueKey('group-pick-open'),
          onPressed: _candidates.isEmpty ? null : _pickNodes,
          icon: const Icon(Icons.add, size: 16),
          label: const Text('选择节点... (多选)', style: TextStyle(fontSize: 12)),
        ),
        const SizedBox(width: 8),
        Text(
          '已选 ${_childIds.length}',
          key: const ValueKey('group-child-count'),
          style: const TextStyle(fontSize: 12),
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

  /// Persisted child resolution (upstream pre-outbound list tab): subscription
  /// matches first, then the explicit order. It reflects the stored node, so
  /// save first and then refresh to confirm a reordered draft.
  Widget _previewSection() {
    String labelOf(String id) {
      for (final p in widget.allProfiles) {
        if (p.indexId == id) return _labelOf(p);
      }
      for (final p in _preview) {
        if (p.indexId == id) return _labelOf(p);
      }
      return id;
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: 12, bottom: 4),
          child: Row(
            children: <Widget>[
              const Expanded(
                child: Text(
                  '组合预览 (已落库解析，订阅匹配优先)',
                  style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
                ),
              ),
              OutlinedButton(
                key: const ValueKey('group-preview-refresh'),
                onPressed: () => setState(_refreshPreview),
                child: const Text('刷新', style: TextStyle(fontSize: 12)),
              ),
            ],
          ),
        ),
        if (!_previewLoaded)
          const Text(
            '新建节点保存后可预览',
            key: ValueKey('group-preview-empty'),
            style: TextStyle(fontSize: 12),
          )
        else if (_preview.isEmpty)
          const Text(
            '暂无解析子节点',
            key: ValueKey('group-preview-empty'),
            style: TextStyle(fontSize: 12),
          )
        else
          for (var i = 0; i < _preview.length; i++)
            Text(
              '${i + 1}. ${labelOf(_preview[i].indexId)}',
              key: ValueKey('group-preview-${_preview[i].indexId}'),
              style: const TextStyle(fontSize: 12),
            ),
      ],
    );
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
  List<c.ProfileDto> Function(String indexId)? previewChildren,
}) {
  return showDialog<c.ProfileDto>(
    context: context,
    builder: (context) => GroupEditorDialog(
      initial: initial,
      allProfiles: allProfiles,
      subItems: subItems,
      onSave: onSave,
      previewChildren: previewChildren,
    ),
  );
}

/// `group_children` result ids in order (test helper).
List<String> groupChildIds(c.ProfilePageDto page) => <String>[
  for (final p in page.items) p.indexId,
];

/// Multi-select node picker for group children.
///
/// Mirrors upstream `ProfilesSelectWindow` as used by
/// `AddGroupServerViewModel.AddChildAsync`: multi-select with select-all over
/// the eligible candidates (Custom already filtered out by the caller).
/// Returns the picked `indexId`s in list order, or `null` on cancel.
Future<List<String>?> showNodePicker(
  BuildContext context, {
  required List<c.ProfileDto> candidates,
}) {
  return showDialog<List<String>>(
    context: context,
    builder: (context) => _NodePickerDialog(candidates: candidates),
  );
}

class _NodePickerDialog extends StatefulWidget {
  const _NodePickerDialog({required this.candidates});

  final List<c.ProfileDto> candidates;

  @override
  State<_NodePickerDialog> createState() => _NodePickerDialogState();
}

class _NodePickerDialogState extends State<_NodePickerDialog> {
  final Set<String> _picked = <String>{};

  String _labelOf(c.ProfileDto p) =>
      p.remarks.isEmpty ? p.indexId : '${p.remarks} [${p.configType.name}]';

  @override
  Widget build(BuildContext context) {
    final allPicked =
        widget.candidates.isNotEmpty &&
        _picked.length == widget.candidates.length;
    return AlertDialog(
      key: const ValueKey('group-picker'),
      title: const Text('选择子节点 (多选)', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 480,
        height: 420,
        child: Column(
          children: <Widget>[
            Row(
              children: <Widget>[
                TextButton(
                  key: const ValueKey('group-pick-select-all'),
                  onPressed: widget.candidates.isEmpty
                      ? null
                      : () => setState(() {
                          if (allPicked) {
                            _picked.clear();
                          } else {
                            _picked
                              ..clear()
                              ..addAll(widget.candidates.map((p) => p.indexId));
                          }
                        }),
                  child: Text(allPicked ? '全不选' : '全选'),
                ),
                const SizedBox(width: 8),
                Text(
                  '已选 ${_picked.length}/${widget.candidates.length}',
                  key: const ValueKey('group-pick-count'),
                  style: const TextStyle(fontSize: 12),
                ),
              ],
            ),
            const Divider(height: 8),
            Expanded(
              child: widget.candidates.isEmpty
                  ? const Center(
                      child: Text(
                        '没有可选节点',
                        key: ValueKey('group-pick-empty'),
                        style: TextStyle(fontSize: 12),
                      ),
                    )
                  : ListView(
                      children: <Widget>[
                        for (final p in widget.candidates)
                          CheckboxListTile(
                            key: ValueKey('group-pick-node-${p.indexId}'),
                            dense: true,
                            title: Text(
                              _labelOf(p),
                              style: const TextStyle(fontSize: 12),
                            ),
                            value: _picked.contains(p.indexId),
                            onChanged: (v) => setState(() {
                              if (v == true) {
                                _picked.add(p.indexId);
                              } else {
                                _picked.remove(p.indexId);
                              }
                            }),
                          ),
                      ],
                    ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('group-pick-cancel'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('group-pick-ok'),
          onPressed: () => Navigator.of(context).pop(<String>[
            for (final p in widget.candidates)
              if (_picked.contains(p.indexId)) p.indexId,
          ]),
          child: const Text('确定'),
        ),
      ],
    );
  }
}
