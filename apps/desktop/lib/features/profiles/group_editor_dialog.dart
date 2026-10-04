import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_dedup.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// True for subscription-derived children, mirroring upstream
/// `!ConfigType.IsComplexType() || Outbound` and the Rust generation path
/// (`application::groups::is_eligible_child`): leaves stay, Outbound stays, and
/// PolicyGroup/ProxyChain/Custom are dropped.
bool _isEligibleSubChild(ConfigType type) =>
    !isComplexProfile(type) || type == ConfigType.outbound;

/// `SubChildItems` targets, resolving the `self` sentinel against the owning
/// subscription ([ownerSubId]) and dropping empty entries (upstream
/// `Utils.String2List`, Rust `sub_child_ids`).
List<String> _subChildTargets(String? raw, String ownerSubId) {
  if (raw == null) return const <String>[];
  return <String>[
    for (final part in raw.split(','))
      if (part.trim().toLowerCase() == 'self') ownerSubId else part.trim(),
  ].where((id) => id.isNotEmpty).toList();
}

/// Match `remarks` against the draft filter, mirroring upstream
/// `Utils.IsRegexMatch`: an empty filter matches everything, an empty remarks
/// never matches, and an uncompilable pattern also matches everything (the
/// .NET helper catches `ArgumentException` and returns true).
bool _remarksMatch(String? filter, String remarks) {
  if (filter == null || filter.trim().isEmpty) return true;
  if (remarks.isEmpty) return false;
  try {
    return RegExp(filter).hasMatch(remarks);
  } on FormatException {
    return true;
  }
}

final RegExp _guidPattern = RegExp(
  r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$',
);

/// VLESS flows accepted by `ProfileItem.IsValid()` (`Global.Flows`).
const List<String> _vlessFlows = <String>[
  '',
  'xtls-rprx-vision',
  'xtls-rprx-vision-udp443',
];

/// Dart port of `domain::Profile::is_valid` (upstream `ProfileItem.IsValid()`):
/// complex/Outbound kinds are always valid, ordinary nodes need a usable
/// address/port plus their protocol credentials. Subscription-derived policy
/// group children are filtered by this so a tolerant import's invalid leaves
/// never enter the group (upstream `GroupProfileManager` requires
/// `p.IsValid()`), keeping the preview in sync with Rust `resolve_sub_children`
/// (R3-PROF-07).
bool _isProfileValid(c.ProfileDto p) {
  final type = p.configType;
  if (isComplexProfile(type) || type == ConfigType.outbound) return true;
  if (p.address.trim().isEmpty || p.port < 1 || p.port > 65535) return false;
  switch (type) {
    case ConfigType.vmess:
      if (!_guidPattern.hasMatch(p.password)) return false;
      break;
    case ConfigType.vless:
      if (p.password.isEmpty) return false;
      if (!_guidPattern.hasMatch(p.password) && p.password.length > 30) {
        return false;
      }
      if (!_vlessFlows.contains(p.protoExtra.flow ?? '')) return false;
      break;
    case ConfigType.shadowsocks:
      if (p.password.isEmpty) return false;
      break;
    default:
      break;
  }
  final isReality =
      (type == ConfigType.vless || type == ConfigType.trojan) &&
      (p.security.streamSecurity ?? '').toLowerCase() == 'reality';
  if (isReality && (p.security.publicKey ?? '').isEmpty) return false;
  return true;
}

/// Resolve the current group/chain draft for the editor preview.
///
/// Faithful Dart port of the persisted generation path
/// (`application::groups::resolve_children` / `resolve_sub_children`, itself
/// the port of upstream `GroupProfileManager.GetChildProfileItemsByProtocolExtra`):
/// subscription matches first (eligible leaf/Outbound nodes of [subChildItems]
/// whose remarks pass [filter], ordered by `IndexId`), then the explicit
/// [childIds] in list order, de-duplicated. Pure: it never reads or writes the
/// store, so refreshing the preview has no side effect on the saved node.
List<c.ProfileDto> resolveGroupPreview({
  required List<c.ProfileDto> all,
  required List<String> childIds,
  String? subChildItems,
  String? filter,
  String ownerSubId = '',
}) {
  final result = <c.ProfileDto>[];
  final seen = <String>{};
  final subIds = _subChildTargets(subChildItems, ownerSubId);
  if (subIds.isNotEmpty) {
    final matched = <c.ProfileDto>[
      for (final p in all)
        if (subIds.contains(p.subid) &&
            _isEligibleSubChild(p.configType) &&
            _isProfileValid(p) &&
            _remarksMatch(filter, p.remarks))
          p,
    ]..sort((a, b) => a.indexId.compareTo(b.indexId));
    for (final p in matched) {
      if (seen.add(p.indexId)) result.add(p);
    }
  }
  final byId = <String, c.ProfileDto>{for (final p in all) p.indexId: p};
  for (final id in childIds) {
    if (id.isEmpty || id.toLowerCase() == 'self') continue;
    final profile = byId[id];
    if (profile != null && seen.add(profile.indexId)) result.add(profile);
  }
  return result;
}

/// Modal editor for PolicyGroup (101) / ProxyChain (102) nodes.
///
/// Mirrors `AddGroupServerViewModel`: remarks, core, five `MultipleLoad`
/// modes, ordered child list (multi-select add, remove, T/U/D/B), subscription
/// child source (`SubChildItems`) plus remarks `Filter`, and a preview resolved
/// from the **current draft** (RE-PROF-10). Cancel never persists; save
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

  /// Optional external preview override (test seam). Production leaves this
  /// null, so the editor resolves the current draft itself through
  /// [resolveGroupPreview] against [allProfiles]; the preview then reflects
  /// unsaved `ChildItems`/mode/`SubChildItems`/`Filter` edits instead of the
  /// persisted node.
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
  c.ErrorDto? _serverError;
  bool _submitting = false;

  /// Existing children selected for batch removal (upstream
  /// `AddGroupServerViewModel.ChildRemoveAsync` iterates `SelectedChildren`).
  final Set<String> _selectedChildren = <String>{};

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

  /// Resolve the preview from the in-progress draft (upstream
  /// `UpdatePreviewList` -> `GetUpdatedProtocolExtra`): the current
  /// `ChildItems`, `SubChildItems`, `Filter` and owner `Subid` are read from the
  /// editor, not the persisted node. Never persists, so cancel leaves the store
  /// untouched. A non-null [GroupEditorDialog.previewChildren] overrides it.
  void _refreshPreview() {
    final override = widget.previewChildren;
    if (override != null) {
      _preview = override(_draft.indexId);
      return;
    }
    _preview = resolveGroupPreview(
      all: widget.allProfiles,
      childIds: _childIds,
      subChildItems: _selectedSubId,
      filter: _draft.filter,
      ownerSubId: _draft.subid,
    );
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
                _previewSection(),
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
    final picked = await showNodePicker(
      context,
      candidates: _candidates,
      subItems: widget.subItems,
      multiSelect: true,
      // Upstream `AddGroupServerViewModel.AddChildAsync`:
      // `SetConfigTypeFilter([EConfigType.Custom], exclude: true)`.
      filterConfigTypes: const <ConfigType>[ConfigType.custom],
      filterExclude: true,
    );
    if (picked == null || picked.isEmpty) return;
    setState(() {
      final taken = _childIds.toSet();
      for (final id in picked) {
        if (taken.add(id)) _childIds.add(id);
      }
    });
  }

  Widget _addRow() {
    final selectedCount = _selectedChildren.length;
    return Row(
      children: <Widget>[
        OutlinedButton.icon(
          key: const ValueKey('group-pick-open'),
          onPressed: _candidates.isEmpty ? null : _pickNodes,
          icon: const Icon(Icons.add, size: 16),
          label: const Text('选择节点... (多选)', style: TextStyle(fontSize: 12)),
        ),
        const SizedBox(width: 8),
        OutlinedButton.icon(
          key: const ValueKey('group-remove-selected'),
          onPressed: selectedCount == 0
              ? null
              : () => setState(() {
                  _childIds.removeWhere(_selectedChildren.contains);
                  _selectedChildren.clear();
                }),
          icon: const Icon(Icons.delete_outline, size: 16),
          label: Text(
            '移除选中 ($selectedCount)',
            style: const TextStyle(fontSize: 12),
          ),
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
    final selected = _selectedChildren.contains(id);
    return ListTile(
      key: ValueKey('group-child-$id'),
      dense: true,
      selected: selected,
      leading: Checkbox(
        key: ValueKey('group-child-select-$id'),
        value: selected,
        visualDensity: VisualDensity.compact,
        onChanged: (_) => setState(() {
          if (!_selectedChildren.add(id)) _selectedChildren.remove(id);
        }),
      ),
      onTap: () => setState(() {
        if (!_selectedChildren.add(id)) _selectedChildren.remove(id);
      }),
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
            onPressed: () => setState(() {
              _childIds.remove(id);
              _selectedChildren.remove(id);
            }),
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

  /// Draft child resolution (upstream pre-outbound preview tab): subscription
  /// matches first, then the explicit order. [GroupEditorDialog.previewChildren]
  /// may override it in tests; otherwise [resolveGroupPreview] reads the current
  /// editor state, so an unsaved reorder/filter is reflected on refresh.
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
                  '组合预览 (按当前草稿解析，订阅匹配优先)',
                  style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
                ),
              ),
              OutlinedButton(
                key: const ValueKey('group-preview-refresh'),
                onPressed: () => setState(_refreshPreview),
                child: const Text('刷新预览', style: TextStyle(fontSize: 12)),
              ),
            ],
          ),
        ),
        if (_preview.isEmpty)
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

/// Node picker for group children / external callers, mirroring the frozen
/// `ProfilesSelectWindow` + `ProfilesSelectViewModel` (RE-PROF-11).
///
/// Restores the upstream contract: a current-group switch, a remarks/address
/// search box, the profile columns (type/remarks/address/port/transport/TLS/
/// subscription/delay/speed), click-to-sort headers, an auto-column-width
/// button, and single- or multi-select results. Callers can constrain the
/// candidate set by `ConfigType` include/exclude, exactly like
/// `ProfilesSelectViewModel.SetConfigTypeFilter`
/// (`AddGroupServerViewModel.AddChildAsync` passes `[Custom]` with
/// `exclude: true`). The WPF window has no on-screen type-filter control, so
/// the type constraint is a caller contract, not a new widget.
///
/// Returns the picked `indexId`s in list order, or `null` on cancel.
Future<List<String>?> showNodePicker(
  BuildContext context, {
  required List<c.ProfileDto> candidates,
  List<c.SubItemDto>? subItems,
  bool multiSelect = true,
  List<ConfigType>? filterConfigTypes,
  bool filterExclude = false,
}) {
  return showDialog<List<String>>(
    context: context,
    builder: (context) => _NodePickerDialog(
      candidates: candidates,
      subItems: subItems,
      multiSelect: multiSelect,
      filterConfigTypes: filterConfigTypes,
      filterExclude: filterExclude,
    ),
  );
}

/// One `ProfilesSelectWindow` DataGrid column (`MyDGTextColumn`).
class _PickerColumn {
  const _PickerColumn(this.key, this.title, this.display);

  final String key;
  final String title;
  final String Function(c.ProfileDto p) display;
}

String _pickerType(c.ProfileDto p) => p.configType.name;
String _pickerRemarks(c.ProfileDto p) => p.remarks;
String _pickerAddress(c.ProfileDto p) => p.address;
String _pickerPort(c.ProfileDto p) => '${p.port}';
String _pickerNetwork(c.ProfileDto p) => p.network;
String _pickerTls(c.ProfileDto p) => p.security.streamSecurity ?? '';
String _pickerSub(c.ProfileDto p) => p.subid;

// Delay/speed live on `ProfileExItem`, which the caller-supplied `ProfileDto`
// candidates do not carry; the columns are present but render `-` until a
// speedtest overlay is threaded through (interface gap, see task card).
String _pickerDelay(c.ProfileDto p) => '-';
String _pickerSpeed(c.ProfileDto p) => '-';

const List<_PickerColumn> _pickerColumns = <_PickerColumn>[
  _PickerColumn('configType', '类型', _pickerType),
  _PickerColumn('remarks', '备注', _pickerRemarks),
  _PickerColumn('address', '地址', _pickerAddress),
  _PickerColumn('port', '端口', _pickerPort),
  _PickerColumn('network', '传输', _pickerNetwork),
  _PickerColumn('security', 'TLS', _pickerTls),
  _PickerColumn('subid', '订阅', _pickerSub),
  _PickerColumn('delay', '延迟', _pickerDelay),
  _PickerColumn('speed', '速度', _pickerSpeed),
];

const Map<String, double> _pickerDefaultWidths = <String, double>{
  'configType': 70,
  'remarks': 140,
  'address': 180,
  'port': 60,
  'network': 80,
  'security': 70,
  'subid': 90,
  'delay': 60,
  'speed': 60,
};

class _NodePickerDialog extends StatefulWidget {
  const _NodePickerDialog({
    required this.candidates,
    this.subItems,
    this.multiSelect = true,
    this.filterConfigTypes,
    this.filterExclude = false,
  });

  final List<c.ProfileDto> candidates;
  final List<c.SubItemDto>? subItems;
  final bool multiSelect;
  final List<ConfigType>? filterConfigTypes;
  final bool filterExclude;

  @override
  State<_NodePickerDialog> createState() => _NodePickerDialogState();
}

class _NodePickerDialogState extends State<_NodePickerDialog> {
  final Set<String> _picked = <String>{};
  final TextEditingController _search = TextEditingController();
  String? _groupSubId;
  String? _pickSingle;
  String _filter = '';
  String _sortKey = '';
  bool _sortAscending = true;
  Map<String, double> _widths = <String, double>{};

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  /// Group choices (upstream `RefreshSubscriptions` puts `AllGroupServers`
  /// first). Derived from [subItems] when given, otherwise from the candidate
  /// `subid`s so callers without a subscription list still get a switch.
  List<(String?, String)> get _groups {
    final result = <(String?, String)>[(null, '全部分组')];
    final items = widget.subItems;
    if (items != null) {
      for (final s in items) {
        result.add((s.id, s.remarks.isEmpty ? s.id : s.remarks));
      }
    } else {
      final seen = <String>{};
      for (final p in widget.candidates) {
        if (seen.add(p.subid)) {
          result.add((p.subid, p.subid.isEmpty ? '(无分组)' : p.subid));
        }
      }
    }
    return result;
  }

  /// Current-group + type-filter + search + sort, mirroring
  /// `ProfilesSelectViewModel.GetProfileItemsEx` (`OrderBy(Sort)` then the
  /// include/exclude type filter) plus the header sort.
  List<c.ProfileDto> get _rows {
    var list = widget.candidates;
    final types = widget.filterConfigTypes;
    if (types != null && types.isNotEmpty) {
      list = widget.filterExclude
          ? <c.ProfileDto>[
              for (final p in list)
                if (!types.contains(p.configType)) p,
            ]
          : <c.ProfileDto>[
              for (final p in list)
                if (types.contains(p.configType)) p,
            ];
    }
    if (_groupSubId != null) {
      list = <c.ProfileDto>[
        for (final p in list)
          if (p.subid == _groupSubId) p,
      ];
    }
    final q = _filter.trim().toLowerCase();
    if (q.isNotEmpty) {
      list = <c.ProfileDto>[
        for (final p in list)
          if (_searchHit(p, q)) p,
      ];
    }
    if (_sortKey.isEmpty) return list;
    final column = _pickerColumns.firstWhere((c) => c.key == _sortKey);
    return List<c.ProfileDto>.of(list)..sort((a, b) {
      final cmp = _compare(column, a, b);
      return _sortAscending ? cmp : -cmp;
    });
  }

  bool _searchHit(c.ProfileDto p, String q) =>
      p.remarks.toLowerCase().contains(q) ||
      p.address.toLowerCase().contains(q) ||
      '${p.port}'.contains(q);

  int _compare(_PickerColumn column, c.ProfileDto a, c.ProfileDto b) {
    if (column.key == 'port') return a.port.compareTo(b.port);
    return column
        .display(a)
        .toLowerCase()
        .compareTo(column.display(b).toLowerCase());
  }

  bool _isPicked(String id) =>
      widget.multiSelect ? _picked.contains(id) : _pickSingle == id;

  void _toggle(String id) {
    setState(() {
      if (widget.multiSelect) {
        if (!_picked.add(id)) _picked.remove(id);
      } else {
        _pickSingle = _pickSingle == id ? null : id;
      }
    });
  }

  void _toggleAll() {
    final rows = _rows;
    final allPicked =
        rows.isNotEmpty && rows.every((p) => _picked.contains(p.indexId));
    setState(() {
      if (allPicked) {
        for (final p in rows) {
          _picked.remove(p.indexId);
        }
      } else {
        for (final p in rows) {
          _picked.add(p.indexId);
        }
      }
    });
  }

  void _commitSearch(String value) => setState(() => _filter = value);

  double _widthOf(_PickerColumn column) =>
      _widths[column.key] ?? _pickerDefaultWidths[column.key]!;

  void _autofit() {
    const headerStyle = TextStyle(fontWeight: FontWeight.bold, fontSize: 12);
    const cellStyle = TextStyle(fontSize: 12);
    final rows = _rows;
    final updated = <String, double>{};
    for (final column in _pickerColumns) {
      var width = _measure('${column.title} \u25B2', headerStyle);
      for (final p in rows) {
        final w = _measure(column.display(p), cellStyle);
        if (w > width) width = w;
      }
      updated[column.key] = (width + 16).clamp(48.0, 260.0).toDouble();
    }
    setState(() => _widths = updated);
  }

  double _measure(String text, TextStyle style) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      maxLines: 1,
      textDirection: TextDirection.ltr,
    )..layout();
    return painter.width;
  }

  List<String> _result() {
    if (widget.multiSelect) {
      return <String>[
        for (final p in widget.candidates)
          if (_picked.contains(p.indexId)) p.indexId,
      ];
    }
    return _pickSingle == null ? const <String>[] : <String>[_pickSingle!];
  }

  @override
  Widget build(BuildContext context) {
    final rows = _rows;
    return AlertDialog(
      key: const ValueKey('group-picker'),
      title: Text(
        widget.multiSelect ? '选择子节点 (多选)' : '选择子节点',
        style: const TextStyle(fontSize: 15),
      ),
      content: SizedBox(
        width: 720,
        height: 480,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: <Widget>[
                for (final group in _groups)
                  ChoiceChip(
                    key: ValueKey('group-pick-group-${group.$1 ?? 'all'}'),
                    label: Text(group.$2, style: const TextStyle(fontSize: 12)),
                    selected: _groupSubId == group.$1,
                    onSelected: (_) => setState(() => _groupSubId = group.$1),
                  ),
              ],
            ),
            const SizedBox(height: 6),
            Row(
              children: <Widget>[
                SizedBox(
                  width: 220,
                  child: TextField(
                    key: const ValueKey('group-pick-search'),
                    controller: _search,
                    style: const TextStyle(fontSize: 12),
                    decoration: const InputDecoration(
                      labelText: '搜索 (备注/地址)',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                    onChanged: (v) {
                      if (v.trim().isEmpty) _commitSearch('');
                    },
                    onSubmitted: _commitSearch,
                  ),
                ),
                const SizedBox(width: 8),
                OutlinedButton(
                  key: const ValueKey('group-pick-autofit'),
                  onPressed: _autofit,
                  child: const Text('自动列宽', style: TextStyle(fontSize: 12)),
                ),
                if (widget.multiSelect) ...<Widget>[
                  const SizedBox(width: 8),
                  TextButton(
                    key: const ValueKey('group-pick-select-all'),
                    onPressed: rows.isEmpty ? null : _toggleAll,
                    child: Text(
                      rows.isNotEmpty &&
                              rows.every((p) => _picked.contains(p.indexId))
                          ? '全不选'
                          : '全选',
                    ),
                  ),
                ],
                const SizedBox(width: 8),
                Text(
                  '已选 ${widget.multiSelect ? _picked.length : (_pickSingle == null ? 0 : 1)}'
                  '/${widget.candidates.length}',
                  key: const ValueKey('group-pick-count'),
                  style: const TextStyle(fontSize: 12),
                ),
              ],
            ),
            const Divider(height: 12),
            Expanded(child: _table(rows)),
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
          onPressed: widget.multiSelect || _pickSingle != null
              ? () => Navigator.of(context).pop(_result())
              : null,
          child: const Text('确定'),
        ),
      ],
    );
  }

  Widget _table(List<c.ProfileDto> rows) {
    final totalWidth =
        32 + _pickerColumns.fold<double>(0, (sum, c) => sum + _widthOf(c));
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: SizedBox(
        width: totalWidth,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            _headerRow(),
            const Divider(height: 1),
            Expanded(
              child: rows.isEmpty
                  ? const Center(
                      child: Text(
                        '没有可选节点',
                        key: ValueKey('group-pick-empty'),
                        style: TextStyle(fontSize: 12),
                      ),
                    )
                  : ListView.builder(
                      itemCount: rows.length,
                      itemBuilder: (context, index) => _row(rows[index]),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _headerRow() {
    const style = TextStyle(fontWeight: FontWeight.bold, fontSize: 12);
    return Row(
      children: <Widget>[
        const SizedBox(width: 32),
        for (final column in _pickerColumns)
          InkWell(
            key: ValueKey('group-pick-sort-${column.key}'),
            onTap: () => setState(() {
              if (_sortKey == column.key) {
                _sortAscending = !_sortAscending;
              } else {
                _sortKey = column.key;
                _sortAscending = true;
              }
            }),
            child: SizedBox(
              width: _widthOf(column),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 6),
                child: Text(
                  '${column.title}'
                  '${_sortKey == column.key ? (_sortAscending ? ' ▲' : ' ▼') : ''}',
                  style: style,
                ),
              ),
            ),
          ),
      ],
    );
  }

  Widget _row(c.ProfileDto p) {
    final selected = _isPicked(p.indexId);
    return InkWell(
      key: ValueKey('group-pick-node-${p.indexId}'),
      onTap: () => _toggle(p.indexId),
      child: Container(
        color: selected ? Theme.of(context).colorScheme.primaryContainer : null,
        child: Row(
          children: <Widget>[
            SizedBox(
              width: 32,
              child: widget.multiSelect
                  ? Checkbox(
                      value: selected,
                      visualDensity: VisualDensity.compact,
                      onChanged: (_) => _toggle(p.indexId),
                    )
                  : Icon(
                      selected
                          ? Icons.radio_button_checked
                          : Icons.radio_button_unchecked,
                      size: 16,
                    ),
            ),
            for (final column in _pickerColumns)
              SizedBox(
                width: _widthOf(column),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 4,
                    vertical: 4,
                  ),
                  child: Text(
                    column.display(p),
                    style: const TextStyle(fontSize: 12),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
