import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_actions.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Upstream `DomainStrategy` candidates (Xray).
const domainStrategyOptions = <String>[
  'AsIs',
  'UseIP',
  'UseIPv4v6',
  'UseIPv6v4',
  'UseIPv4',
  'UseIPv6',
  '',
];

/// Upstream `DomainStrategies4Sbox` candidates (sing-box).
const domainStrategySboxOptions = <String>[
  '',
  'prefer_ipv4',
  'prefer_ipv6',
  'ipv4_only',
  'ipv6_only',
];

/// The routing settings window (upstream `RoutingSettingWindow`,
/// LAY-ROUTINGSET-001, F-ROUTING-002). Opened from 设置/路由设置
/// (ACT-MAIN-025).
Future<void> showRoutingSettingWindow(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const RoutingSettingWindow(),
  );
}

class RoutingSettingWindow extends ConsumerStatefulWidget {
  const RoutingSettingWindow({super.key});

  @override
  ConsumerState<RoutingSettingWindow> createState() =>
      _RoutingSettingWindowState();
}

class _RoutingSettingWindowState extends ConsumerState<RoutingSettingWindow> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(routingControllerProvider.notifier).reload();
      // The top strategy row edits the global object; load it for display.
      try {
        ref.read(settingsControllerProvider.notifier).load();
      } catch (_) {}
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(routingControllerProvider);
    final controller = ref.read(routingControllerProvider.notifier);
    final primary = Theme.of(context).colorScheme.primary;
    return AlertDialog(
      key: const ValueKey('routing-setting-window'),
      title: const Text('路由设置', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      content: SizedBox(
        width: 820,
        height: 500,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            // Upstream ToolBarTray (RoutingSettingWindow.xaml:27):
            // 添加规则集 / 一键导入规则集 sit above the strategy rows.
            Row(
              children: <Widget>[
                TextButton.icon(
                  key: const ValueKey('routing-add'),
                  onPressed: () => _openRuleset(null),
                  icon: const Icon(Icons.add, size: 16),
                  label: const Text('添加规则集'),
                ),
                const SizedBox(width: 4),
                TextButton.icon(
                  key: const ValueKey('routing-import-builtin'),
                  onPressed: _importBuiltinRules,
                  icon: const Icon(Icons.download, size: 16),
                  label: const Text('一键导入规则集'),
                ),
              ],
            ),
            const Divider(height: 1),
            const SizedBox(height: 8),
            _strategyRow(
              context,
              label: '域名解析策略',
              key: const ValueKey('routing-domain-strategy'),
              value: _strategyValue(),
              options: domainStrategyOptions,
              onChanged: _saveStrategy,
            ),
            const SizedBox(height: 4),
            _strategyRow(
              context,
              label: 'sing-box 域名解析策略',
              key: const ValueKey('routing-domain-strategy-sbox'),
              value: _strategySboxValue(),
              options: domainStrategySboxOptions,
              onChanged: _saveStrategySbox,
            ),
            const SizedBox(height: 8),
            // Upstream TabItem header (RoutingSettingWindow.xaml:105).
            Align(
              alignment: Alignment.centerLeft,
              child: Text(
                '预定义规则集列表',
                key: const ValueKey('routing-block-title'),
                style: TextStyle(
                  fontSize: 13,
                  color: primary,
                  decoration: TextDecoration.underline,
                ),
              ),
            ),
            const SizedBox(height: 4),
            const _SchemeTableHeader(),
            const Divider(height: 1),
            Expanded(
              child: state.items.isEmpty
                  ? const Center(
                      key: ValueKey('routing-empty'),
                      child: Text('暂无路由方案'),
                    )
                  : ListView.builder(
                      key: const ValueKey('routing-list'),
                      itemCount: state.items.length,
                      itemBuilder: (context, index) {
                        final item = state.items[index];
                        return _SchemeRow(
                          item: item,
                          selected: item.id == state.selectedId,
                          onTap: () => controller.select(item.id),
                          onEdit: () => _openRuleset(item),
                          onContextMenu: (position) =>
                              _showRowMenu(context, position, item),
                        );
                      },
                    ),
            ),
            if (state.status != null)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  state.status!,
                  key: const ValueKey('routing-status'),
                  style: const TextStyle(fontSize: 11, color: Colors.grey),
                ),
              ),
          ],
        ),
      ),
      // Upstream RoutingSettingWindow has no bottom action bar: 添加规则集
      // lives in the toolbar and 移除所选规则 / 设为活动规则 / 全选 sit in the
      // row context menu (MenuItems, RoutingSettingWindow.xaml:118-144). A
      // single 关闭 remains until the window-form card replaces the dialog.
      actions: <Widget>[
        TextButton(
          key: const ValueKey('routing-close'),
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    );
  }

  /// One strategy row: linked label (upstream `TbdomainStrategy` /
  /// `TbdomainStrategy4Singbox`) plus a 300px combo box.
  Widget _strategyRow(
    BuildContext context, {
    required String label,
    required ValueKey<String> key,
    required String value,
    required List<String> options,
    required ValueChanged<String> onChanged,
  }) {
    final primary = Theme.of(context).colorScheme.primary;
    return Row(
      children: <Widget>[
        Text(label, style: TextStyle(fontSize: 12, color: primary)),
        const SizedBox(width: 2),
        Icon(Icons.link, size: 14, color: primary),
        const SizedBox(width: 8),
        SizedBox(
          width: 300,
          child: DropdownButton<String>(
            key: key,
            value: value,
            isExpanded: true,
            items: [
              for (final s in options)
                DropdownMenuItem(
                  value: s,
                  child: Text(s, style: const TextStyle(fontSize: 12)),
                ),
            ],
            onChanged: (v) => onChanged(v ?? ''),
          ),
        ),
      ],
    );
  }

  /// Upstream `RoutingAdvancedImportRules` -> `ConfigHandler.InitRouting`.
  /// The Rust side exposes no re-import use case yet (registered gap), so the
  /// button re-reads the persisted builtin schemes for now.
  Future<void> _importBuiltinRules() async {
    ref.read(routingControllerProvider.notifier).reload();
    if (!mounted) return;
    ScaffoldMessenger.maybeOf(context)
        ?.showSnackBar(const SnackBar(content: Text('内置规则集已刷新（导入后端用例待接入）')));
  }

  /// Upstream row `ContextMenu` (RoutingSettingWindow.xaml:118): 添加规则集 /
  /// 移除所选规则 / 设为活动规则 / 一键导入规则集. `全选` is omitted because
  /// the RC list is single-select (registered gap).
  Future<void> _showRowMenu(
    BuildContext context,
    Offset globalPosition,
    r.RoutingProfileDto item,
  ) async {
    ref.read(routingControllerProvider.notifier).select(item.id);
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final action = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        globalPosition & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<String>>[
        const PopupMenuItem<String>(value: 'add', child: Text('添加规则集')),
        PopupMenuItem<String>(
          value: 'remove',
          enabled: item.remarks.isNotEmpty,
          child: const Text('移除所选规则'),
        ),
        PopupMenuItem<String>(
          value: 'default',
          enabled: item.remarks.isNotEmpty,
          child: const Text('设为活动规则'),
        ),
        const PopupMenuDivider(),
        const PopupMenuItem<String>(value: 'import', child: Text('一键导入规则集')),
      ],
    );
    if (!mounted || action == null) return;
    switch (action) {
      case 'add':
        await _openRuleset(null);
      case 'remove':
        await _confirmDelete(item.id);
      case 'default':
        ref.read(routingControllerProvider.notifier).setDefault(item.id);
      case 'import':
        await _importBuiltinRules();
    }
  }

  /// Upstream `RoutingSettingViewModel.SaveSettingsAsync`: the top strategy
  /// row edits the global `RoutingBasicItem`, never the selected scheme.
  String _strategyValue() {
    final group = ref
        .watch(settingsControllerProvider)
        .group('RoutingBasicItem');
    final value = group['DomainStrategy']?.toString() ?? '';
    return domainStrategyOptions.contains(value) ? value : '';
  }

  String _strategySboxValue() {
    final group = ref
        .watch(settingsControllerProvider)
        .group('RoutingBasicItem');
    final value = group['DomainStrategy4Singbox']?.toString() ?? '';
    return domainStrategySboxOptions.contains(value) ? value : '';
  }

  void _saveStrategy(String value) {
    final settings = ref.read(settingsControllerProvider.notifier);
    final group = Map<String, dynamic>.of(
      ref.read(settingsControllerProvider).group('RoutingBasicItem'),
    );
    group['DomainStrategy'] = value;
    settings.saveGroup('RoutingBasicItem', group);
  }

  void _saveStrategySbox(String value) {
    final settings = ref.read(settingsControllerProvider.notifier);
    final group = Map<String, dynamic>.of(
      ref.read(settingsControllerProvider).group('RoutingBasicItem'),
    );
    group['DomainStrategy4Singbox'] = value;
    settings.saveGroup('RoutingBasicItem', group);
  }

  Future<void> _openRuleset(r.RoutingProfileDto? item) async {
    await showRoutingRulesetWindow(context, ref, item);
    ref.read(routingControllerProvider.notifier).reload();
  }

  Future<void> _confirmDelete(String id) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('删除路由方案', style: TextStyle(fontSize: 14)),
        content: const Text('确定删除选中的路由方案吗？'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('routing-delete-confirm'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed == true && mounted) {
      ref.read(routingControllerProvider.notifier).delete(id);
    }
  }
}

class _SchemeTableHeader extends StatelessWidget {
  const _SchemeTableHeader();

  @override
  Widget build(BuildContext context) {
    const style = TextStyle(fontSize: 11, color: Colors.grey);
    // Upstream DataGrid columns (RoutingSettingWindow.xaml:157-177):
    // Remarks(*) / Count(60) / Sort(60) / Url(*) / CustomIcon(300).
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      child: Row(
        children: <Widget>[
          Expanded(flex: 3, child: Text('别名', style: style)),
          Expanded(child: Text('数量', style: style)),
          Expanded(child: Text('排序', style: style)),
          Expanded(flex: 3, child: Text('可选地址 (Url)', style: style)),
          Expanded(flex: 2, child: Text('自定义图标', style: style)),
        ],
      ),
    );
  }
}

class _SchemeRow extends StatelessWidget {
  const _SchemeRow({
    required this.item,
    required this.selected,
    required this.onTap,
    required this.onEdit,
    required this.onContextMenu,
  });

  final r.RoutingProfileDto item;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onEdit;
  final void Function(Offset globalPosition) onContextMenu;

  @override
  Widget build(BuildContext context) {
    // Upstream highlights the active scheme (DataGrid DataTrigger on IsActive);
    // the 状态 column is not part of the frozen XAML.
    return InkWell(
      key: ValueKey('routing-row-${item.id}'),
      onTap: onTap,
      onDoubleTap: onEdit,
      onSecondaryTapDown: (details) => onContextMenu(details.globalPosition),
      child: Container(
        color: selected ? Colors.blue.withValues(alpha: 0.08) : null,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Row(
          children: <Widget>[
            Expanded(flex: 3, child: Text(item.remarks)),
            Expanded(child: Text('${item.ruleNum}')),
            Expanded(child: Text('${item.sort}')),
            Expanded(
              flex: 3,
              child: Text(
                item.url.isEmpty ? '—' : item.url,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: Colors.grey),
              ),
            ),
            Expanded(
              flex: 2,
              child: Text(
                item.customIcon,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: Colors.grey),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The rule-set editor (upstream `RoutingRuleSettingWindow`,
/// LAY-ROUTINGRULESET-001, F-ROUTING-003). Edits a scheme's profile fields
/// and its rule list (T/U/D/B move, import/export, export-selected).
/// Cancel discards the draft; Save persists via the bridge.
Future<void> showRoutingRulesetWindow(
  BuildContext context,
  WidgetRef ref,
  r.RoutingProfileDto? item,
) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => RoutingRulesetWindow(item: item),
  );
}

class RoutingRulesetWindow extends ConsumerStatefulWidget {
  const RoutingRulesetWindow({super.key, this.item});

  final r.RoutingProfileDto? item;

  @override
  ConsumerState<RoutingRulesetWindow> createState() =>
      _RoutingRulesetWindowState();
}

class _RoutingRulesetWindowState extends ConsumerState<RoutingRulesetWindow> {
  late TextEditingController _remarks;
  late TextEditingController _url;
  late TextEditingController _customIcon;
  late TextEditingController _rulesetPath;
  late TextEditingController _sort;
  bool _enabled = true;
  final Set<String> _selectedRuleIds = <String>{};
  List<r.RoutingRuleDto> _rules = const [];
  List<r.RoutingWarningDto> _warnings = const [];

  bool get isNew => widget.item == null;

  @override
  void initState() {
    super.initState();
    final item = widget.item;
    _remarks = TextEditingController(text: item?.remarks ?? '');
    _url = TextEditingController(text: item?.url ?? '');
    _customIcon = TextEditingController(text: item?.customIcon ?? '');
    _rulesetPath = TextEditingController(
      text: item?.customRulesetPath4Singbox ?? '',
    );
    _sort = TextEditingController(text: '${item?.sort ?? 0}');
    _enabled = item?.enabled ?? true;
    if (item != null) _loadRules(item.id);
  }

  void _loadRules(String routingId) {
    final page = ref.read(bridgePortProvider).listRoutingRules(routingId);
    if (page.ok) {
      setState(() {
        _rules = page.rules;
        _warnings = page.warnings;
      });
    }
  }

  @override
  void dispose() {
    _remarks.dispose();
    _url.dispose();
    _customIcon.dispose();
    _rulesetPath.dispose();
    _sort.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      key: const ValueKey('routing-ruleset-window'),
      title: Text(
        isNew ? '新增路由方案' : '编辑路由方案',
        style: const TextStyle(fontSize: 15),
      ),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      content: SizedBox(
        width: 780,
        height: 520,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            _field('备注', _remarks, const ValueKey('ruleset-remarks')),
            _field('URL', _url, const ValueKey('ruleset-url')),
            Row(
              children: <Widget>[
                Expanded(
                  child: _field(
                    '自定义图标',
                    _customIcon,
                    const ValueKey('ruleset-icon'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: _field(
                    'sing-box 自定义规则集路径',
                    _rulesetPath,
                    const ValueKey('ruleset-path'),
                  ),
                ),
                const SizedBox(width: 8),
                SizedBox(
                  width: 90,
                  child: _field(
                    '排序',
                    _sort,
                    const ValueKey('ruleset-sort'),
                    numeric: true,
                  ),
                ),
                const SizedBox(width: 8),
                Row(
                  children: <Widget>[
                    const Text('启用', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('ruleset-enabled'),
                      value: _enabled,
                      onChanged: (v) => setState(() => _enabled = v),
                    ),
                  ],
                ),
              ],
            ),
            Row(
              children: <Widget>[
                _tool('新增规则', const ValueKey('rule-add'), _addRule),
                _tool(
                  '删除',
                  const ValueKey('rule-remove'),
                  _selectedRuleIds.isEmpty ? null : _confirmRemove,
                ),
                _tool(
                  '导出选中',
                  const ValueKey('rule-export'),
                  _selectedRuleIds.isEmpty ? null : _exportSelected,
                ),
                _tool('从剪贴板导入', const ValueKey('rule-import-clipboard'), () {
                  pickRulesFromClipboard(context).then(_mergeImported);
                }),
                _tool('从文件导入', const ValueKey('rule-import-file'), () {
                  pickRulesFromFile(context).then(_mergeImported);
                }),
                _tool('从URL导入', const ValueKey('rule-import-url'), () {
                  _importFromUrl();
                }),
              ],
            ),
            const _RuleTableHeader(),
            const Divider(height: 1),
            Expanded(
              child: _rules.isEmpty
                  ? const Center(child: Text('暂无规则，请新增或导入'))
                  : ListView.builder(
                      key: const ValueKey('rule-list'),
                      itemCount: _rules.length,
                      itemBuilder: (context, index) {
                        final rule = _rules[index];
                        return _RuleRow(
                          rule: rule,
                          index: index,
                          selected: _selectedRuleIds.contains(rule.id),
                          onTap: () => setState(() {
                            if (_selectedRuleIds.contains(rule.id)) {
                              _selectedRuleIds.remove(rule.id);
                            } else {
                              _selectedRuleIds.add(rule.id);
                            }
                          }),
                          onEdit: () => _editRule(rule),
                          onTop: () => _move(index, 0),
                          onUp: () => _move(index, 1),
                          onDown: () => _move(index, 2),
                          onBottom: () => _move(index, 3),
                        );
                      },
                    ),
            ),
            if (_warnings.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  _warnings.first.message,
                  key: const ValueKey('rule-warning'),
                  style: const TextStyle(fontSize: 11, color: Colors.orange),
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('ruleset-cancel'),
          onPressed: () => Navigator.pop(context),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('ruleset-save'),
          onPressed: _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _field(
    String label,
    TextEditingController controller,
    ValueKey<String> key, {
    bool numeric = false,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: TextField(
        key: key,
        controller: controller,
        keyboardType: numeric ? TextInputType.number : null,
        inputFormatters: numeric
            ? <TextInputFormatter>[FilteringTextInputFormatter.digitsOnly]
            : null,
        decoration: InputDecoration(
          labelText: label,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        style: const TextStyle(fontSize: 13),
      ),
    );
  }

  Widget _tool(String label, ValueKey<String> key, VoidCallback? onPressed) {
    return Padding(
      padding: const EdgeInsets.only(right: 4),
      child: TextButton(key: key, onPressed: onPressed, child: Text(label)),
    );
  }

  Future<void> _addRule() async {
    final draft = ref.read(routingControllerProvider.notifier).newRuleDraft();
    final saved = await showRoutingRuleDetailsDialog(context, ref, draft);
    if (saved != null && mounted) {
      setState(() => _rules = [..._rules, saved]);
    }
  }

  Future<void> _editRule(r.RoutingRuleDto rule) async {
    final saved = await showRoutingRuleDetailsDialog(context, ref, rule);
    if (saved != null && mounted) {
      setState(() {
        _rules = _rules.map((e) => e.id == rule.id ? saved : e).toList();
      });
    }
  }

  /// Confirm before removing the selected rules (upstream `RuleRemoveAsync`
  /// asks `RemoveServer`); cancelling keeps the draft untouched.
  Future<void> _confirmRemove() async {
    if (_selectedRuleIds.isEmpty) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        key: const ValueKey('rule-delete-confirm-dialog'),
        title: const Text('删除规则', style: TextStyle(fontSize: 14)),
        content: Text('确定删除选中的 ${_selectedRuleIds.length} 条规则吗？'),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('rule-delete-cancel'),
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('rule-delete-confirm'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed == true && mounted) {
      setState(() {
        _rules = _rules.where((e) => !_selectedRuleIds.contains(e.id)).toList();
        _selectedRuleIds.clear();
      });
    }
  }

  /// Merge parsed import results into this draft only; storage is untouched
  /// until Save (upstream `AddBatchRoutingRulesAsync` mutates `_rules`).
  /// A failed parse or a cancelled dialog leaves the draft unchanged.
  void _mergeImported(({List<r.RoutingRuleDto> rules, bool replace})? picked) {
    if (picked == null || !mounted) return;
    setState(() {
      _rules = picked.replace
          ? List<r.RoutingRuleDto>.of(picked.rules)
          : <r.RoutingRuleDto>[..._rules, ...picked.rules];
    });
  }

  Future<void> _exportSelected() async {
    // Export what the user is looking at (the draft), not the stored row.
    if (_selectedRuleIds.isEmpty) return;
    final text = RoutingController.exportDraftRulesJson(
      _rules,
      _selectedRuleIds.toList(),
    );
    await Clipboard.setData(ClipboardData(text: text));
  }

  /// Reorder the draft list in place (upstream `ConfigHandler.MoveRoutingRule`
  /// on `_rules`). Nothing is persisted until Save.
  void _move(int index, int direction) {
    setState(() {
      final list = List.of(_rules);
      var target = index;
      switch (direction) {
        case 0:
          target = 0;
        case 1:
          target = index - 1;
        case 2:
          target = index + 1;
        case 3:
          target = list.length - 1;
      }
      if (target >= 0 && target < list.length && target != index) {
        final item = list.removeAt(index);
        list.insert(target, item);
        _rules = list;
      }
    });
  }

  Future<void> _importFromUrl() async {
    final urlController = TextEditingController(text: _url.text);
    final url = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        key: const ValueKey('routing-import-url-dialog'),
        title: const Text('从 URL 导入规则', style: TextStyle(fontSize: 15)),
        content: SizedBox(
          width: 420,
          child: TextField(
            key: const ValueKey('routing-import-url-field'),
            controller: urlController,
            decoration: const InputDecoration(
              hintText: 'http(s)://…/rules.json',
              border: OutlineInputBorder(),
            ),
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('routing-import-url-ok'),
            onPressed: () => Navigator.pop(context, urlController.text),
            child: const Text('下载导入'),
          ),
        ],
      ),
    );
    if (url == null || !mounted) return;
    pickRulesFromUrl(context, url).then(_mergeImported);
  }

  /// Persist the scheme and its draft rules with one `save_routing` call, so
  /// an empty rule list also saves and a rule failure never closes the window
  /// with a partial commit. Cancel/close leaves storage untouched.
  void _save() {
    if (_remarks.text.trim().isEmpty) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('请填写备注')));
      return;
    }
    final controller = ref.read(routingControllerProvider.notifier);
    final existing = widget.item;
    final draft = r.RoutingProfileDto(
      id: existing?.id ?? '',
      remarks: _remarks.text.trim(),
      url: _url.text.trim(),
      ruleSet: RoutingController.rulesToRuleSetJson(_rules),
      ruleNum: _rules.length,
      enabled: _enabled,
      locked: existing?.locked ?? false,
      customIcon: _customIcon.text.trim(),
      customRulesetPath4Singbox: _rulesetPath.text.trim(),
      domainStrategy: existing?.domainStrategy ?? '',
      domainStrategy4Singbox: existing?.domainStrategy4Singbox ?? '',
      sort: int.tryParse(_sort.text.trim()) ?? 0,
      isActive: existing?.isActive ?? false,
    );
    final saved = controller.save(draft);
    if (!saved.ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('保存失败：${saved.error?.messageKey}')),
      );
      return;
    }
    if (mounted) Navigator.pop(context);
  }
}

class _RuleTableHeader extends StatelessWidget {
  const _RuleTableHeader();

  @override
  Widget build(BuildContext context) {
    const style = TextStyle(fontSize: 11, color: Colors.grey);
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      child: Row(
        children: <Widget>[
          Expanded(flex: 2, child: Text('备注', style: style)),
          Expanded(child: Text('类型', style: style)),
          Expanded(child: Text('出站', style: style)),
          Expanded(flex: 3, child: Text('匹配', style: style)),
          Expanded(child: Text('启用', style: style)),
          SizedBox(width: 120, child: Text('移动', style: style)),
        ],
      ),
    );
  }
}

class _RuleRow extends StatelessWidget {
  const _RuleRow({
    required this.rule,
    required this.index,
    required this.selected,
    required this.onTap,
    required this.onEdit,
    required this.onTop,
    required this.onUp,
    required this.onDown,
    required this.onBottom,
  });

  final r.RoutingRuleDto rule;
  final int index;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onEdit;
  final VoidCallback onTop;
  final VoidCallback onUp;
  final VoidCallback onDown;
  final VoidCallback onBottom;

  @override
  Widget build(BuildContext context) {
    final match = <String>[
      if ((rule.port ?? '').isNotEmpty) 'port:${rule.port}',
      if ((rule.network ?? '').isNotEmpty) 'net:${rule.network}',
      if (rule.hasDomain && rule.domain.isNotEmpty)
        'domain:${rule.domain.length}',
      if (rule.hasIp && rule.ip.isNotEmpty) 'ip:${rule.ip.length}',
      if (rule.hasProtocol && rule.protocol.isNotEmpty)
        'proto:${rule.protocol.join(',')}',
      if (rule.hasProcess && rule.process.isNotEmpty)
        'proc:${rule.process.length}',
      if (rule.hasInboundTag && rule.inboundTag.isNotEmpty)
        'in:${rule.inboundTag.join(',')}',
    ].join(' ');
    final typeName = switch (rule.ruleType) {
      0 => 'ALL',
      2 => 'DNS',
      _ => 'Routing',
    };
    // The move buttons sit OUTSIDE the row InkWell on purpose: an ancestor
    // with onDoubleTap starves inner button taps in the gesture arena (the
    // single tap never resolves), so nesting them silently kills T/U/D/B.
    return Container(
      key: ValueKey('rule-row-$index'),
      color: selected ? Colors.blue.withValues(alpha: 0.08) : null,
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      child: Row(
        children: <Widget>[
          Expanded(
            flex: 5,
            child: InkWell(
              onTap: onTap,
              onDoubleTap: onEdit,
              child: Row(
                children: <Widget>[
                  Expanded(flex: 2, child: Text(rule.remarks ?? '—')),
                  Expanded(child: Text(typeName)),
                  Expanded(child: Text(rule.outboundTag ?? '—')),
                  Expanded(
                    flex: 3,
                    child: Text(
                      match.isEmpty ? '—' : match,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 12, color: Colors.grey),
                    ),
                  ),
                  Expanded(child: Text(rule.enabled ? '是' : '否')),
                ],
              ),
            ),
          ),
          SizedBox(
            width: 120,
            child: Row(
              children: <Widget>[
                _mv('T', const ValueKey('rule-top'), onTop, index),
                _mv('U', const ValueKey('rule-up'), onUp, index),
                _mv('D', const ValueKey('rule-down'), onDown, index),
                _mv('B', const ValueKey('rule-bottom'), onBottom, index),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _mv(
    String label,
    ValueKey<String> key,
    VoidCallback onPressed,
    int index,
  ) {
    return SizedBox(
      width: 30,
      child: TextButton(
        key: ValueKey('${key.value}-$index'),
        onPressed: onPressed,
        style: TextButton.styleFrom(padding: EdgeInsets.zero),
        child: Text(label, style: const TextStyle(fontSize: 12)),
      ),
    );
  }
}

/// The rule details window (upstream `RoutingRuleDetailsWindow`,
/// LAY-ROUTINGRULEDETAIL-001, F-ROUTING-004). Full-field form with validation;
/// Cancel discards the draft, Save returns it to the caller.
Future<r.RoutingRuleDto?> showRoutingRuleDetailsDialog(
  BuildContext context,
  WidgetRef ref,
  r.RoutingRuleDto rule,
) {
  return showDialog<r.RoutingRuleDto>(
    context: context,
    barrierDismissible: false,
    builder: (_) => RoutingRuleDetailsDialog(rule: rule),
  );
}

class RoutingRuleDetailsDialog extends ConsumerStatefulWidget {
  const RoutingRuleDetailsDialog({
    super.key,
    required this.rule,
    this.outboundTags,
  });

  final r.RoutingRuleDto rule;

  /// Outbound candidates supplied by the routing window (second engine) so the
  /// picker never needs the Rust bridge. Null keeps the embedded-dialog path.
  final List<String>? outboundTags;

  @override
  ConsumerState<RoutingRuleDetailsDialog> createState() =>
      _RoutingRuleDetailsDialogState();
}

class _RoutingRuleDetailsDialogState
    extends ConsumerState<RoutingRuleDetailsDialog> {
  late TextEditingController _remarks;
  late TextEditingController _port;
  late TextEditingController _network;
  late TextEditingController _domain;
  late TextEditingController _ip;
  late TextEditingController _process;
  late TextEditingController _outbound;
  bool _enabled = true;
  int _ruleType = 1;
  final Set<String> _protocols = <String>{};
  final Set<String> _inbounds = <String>{};
  bool _autoSort = false;

  static const protocolOptions = <String>['http', 'tls', 'quic', 'bittorrent'];
  static const inboundOptions = <String>['tun', 'socks', 'socks2', 'socks3'];
  static const networkOptions = <String>['', 'tcp', 'udp', 'tcp,udp'];

  @override
  void initState() {
    super.initState();
    final rule = widget.rule;
    _remarks = TextEditingController(text: rule.remarks ?? '');
    _port = TextEditingController(text: rule.port ?? '');
    _network = TextEditingController(text: rule.network ?? '');
    _domain = TextEditingController(text: rule.domain.join('\n'));
    _ip = TextEditingController(text: rule.ip.join('\n'));
    _process = TextEditingController(text: rule.process.join('\n'));
    _outbound = TextEditingController(text: rule.outboundTag ?? 'proxy');
    _enabled = rule.enabled;
    _ruleType = rule.ruleType ?? 1;
    _protocols.addAll(rule.protocol);
    _inbounds.addAll(rule.inboundTag);
  }

  @override
  void dispose() {
    _remarks.dispose();
    _port.dispose();
    _network.dispose();
    _domain.dispose();
    _ip.dispose();
    _process.dispose();
    _outbound.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      key: const ValueKey('routing-rule-details-window'),
      title: const Text('规则详情', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      content: SizedBox(
        width: 720,
        height: 520,
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                children: <Widget>[
                  Expanded(
                    child: _field(
                      '备注',
                      _remarks,
                      const ValueKey('rule-remarks'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Row(
                    children: <Widget>[
                      const Text('启用', style: TextStyle(fontSize: 12)),
                      Switch(
                        key: const ValueKey('rule-enabled'),
                        value: _enabled,
                        onChanged: (v) => setState(() => _enabled = v),
                      ),
                    ],
                  ),
                  const SizedBox(width: 8),
                  DropdownButton<int>(
                    key: const ValueKey('rule-type'),
                    value: _ruleType,
                    items: const [
                      DropdownMenuItem(value: 0, child: Text('ALL')),
                      DropdownMenuItem(value: 1, child: Text('Routing')),
                      DropdownMenuItem(value: 2, child: Text('DNS')),
                    ],
                    onChanged: (v) => setState(() => _ruleType = v ?? 1),
                  ),
                ],
              ),
              Row(
                children: <Widget>[
                  Expanded(
                    child: _field(
                      '出站 (OutboundTag)',
                      _outbound,
                      const ValueKey('rule-outbound'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  TextButton(
                    key: const ValueKey('rule-select-profile'),
                    onPressed: _selectProfile,
                    child: const Text('选择节点…'),
                  ),
                ],
              ),
              Row(
                children: <Widget>[
                  Expanded(
                    child: _field(
                      '端口 (如 80,443 / 0-65535)',
                      _port,
                      const ValueKey('rule-port'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Row(
                      children: <Widget>[
                        const Text('网络', style: TextStyle(fontSize: 12)),
                        const SizedBox(width: 8),
                        DropdownButton<String>(
                          key: const ValueKey('rule-network'),
                          value: networkOptions.contains(_network.text)
                              ? _network.text
                              : '',
                          items: [
                            for (final n in networkOptions)
                              DropdownMenuItem(
                                value: n,
                                child: Text(
                                  n.isEmpty ? '(空)' : n,
                                  style: const TextStyle(fontSize: 12),
                                ),
                              ),
                          ],
                          onChanged: (v) =>
                              setState(() => _network.text = v ?? ''),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              _checkGroup(
                '协议',
                protocolOptions,
                _protocols,
                const ValueKey('rule-protocol'),
              ),
              _checkGroup(
                '入站标签',
                inboundOptions,
                _inbounds,
                const ValueKey('rule-inbound'),
              ),
              Row(
                children: <Widget>[
                  const Text('自动排序列表', style: TextStyle(fontSize: 12)),
                  Switch(
                    key: const ValueKey('rule-autosort'),
                    value: _autoSort,
                    onChanged: (v) => setState(() => _autoSort = v),
                  ),
                ],
              ),
              _field(
                '域名（每行一个）',
                _domain,
                const ValueKey('rule-domain'),
                lines: 3,
              ),
              _field('IP（每行一个）', _ip, const ValueKey('rule-ip'), lines: 3),
              _field(
                '进程（每行一个）',
                _process,
                const ValueKey('rule-process'),
                lines: 2,
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('rule-details-cancel'),
          onPressed: () => Navigator.pop(context),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('rule-details-save'),
          onPressed: _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _field(
    String label,
    TextEditingController controller,
    ValueKey<String> key, {
    int lines = 1,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: TextField(
        key: key,
        controller: controller,
        maxLines: lines,
        minLines: lines,
        decoration: InputDecoration(
          labelText: label,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        style: const TextStyle(fontSize: 13),
      ),
    );
  }

  Widget _checkGroup(
    String label,
    List<String> options,
    Set<String> selected,
    ValueKey<String> key,
  ) {
    return Padding(
      key: key,
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          SizedBox(
            width: 64,
            child: Text(label, style: const TextStyle(fontSize: 12)),
          ),
          Expanded(
            child: Wrap(
              spacing: 4,
              children: [
                for (final option in options)
                  FilterChip(
                    key: ValueKey('${key.value}-$option'),
                    label: Text(option, style: const TextStyle(fontSize: 12)),
                    selected: selected.contains(option),
                    onSelected: (v) => setState(() {
                      if (v) {
                        selected.add(option);
                      } else {
                        selected.remove(option);
                      }
                    }),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// OutboundTag selector: built-in tags + live node remarks (F-ROUTING-004).
  /// Custom-config profiles are excluded (upstream `SelectProfileAsync`:
  /// `SetConfigTypeFilter([EConfigType.Custom], exclude: true)`).
  Future<void> _selectProfile() async {
    final supplied = widget.outboundTags;
    final options = supplied != null
        ? List<String>.of(supplied)
        : () {
            final bridge = ref.read(bridgePortProvider);
            final remarks =
                bridge
                    .queryAllProfiles()
                    .where((p) => p.configType != ConfigType.custom)
                    .map((p) => p.remarks)
                    .toList()
                  ..sort();
            return <String>['proxy', 'direct', 'block', ...remarks];
          }();
    final picked = await showDialog<String>(
      context: context,
      builder: (context) => SimpleDialog(
        key: const ValueKey('rule-outbound-picker'),
        title: const Text('选择目标出站', style: TextStyle(fontSize: 14)),
        children: [
          for (final option in options)
            SimpleDialogOption(
              key: ValueKey('rule-outbound-$option'),
              onPressed: () => Navigator.pop(context, option),
              child: Text(option),
            ),
        ],
      ),
    );
    if (picked != null && mounted) {
      setState(() => _outbound.text = picked);
    }
  }

  List<String> _lines(TextEditingController controller, bool autoSort) {
    final list = controller.text
        .split(RegExp(r'[\r\n,，]+'))
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();
    if (autoSort) list.sort();
    return list;
  }

  void _save() {
    final domains = _lines(_domain, _autoSort);
    final ips = _lines(_ip, _autoSort);
    final processes = _lines(_process, _autoSort);
    final hasRule =
        domains.isNotEmpty ||
        ips.isNotEmpty ||
        processes.isNotEmpty ||
        _protocols.isNotEmpty ||
        _port.text.trim().isNotEmpty ||
        _network.text.trim().isNotEmpty ||
        _inbounds.isNotEmpty;
    if (!hasRule) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('请至少填写 Network/Port/Protocol/Domain/IP/Process 其中之一'),
        ),
      );
      return;
    }
    // Empty outbound falls back to `proxy` with a warning at generation
    // (upstream `CoreConfigContextBuilder`); it must not block the save.
    final outbound = _outbound.text.trim();
    Navigator.pop(
      context,
      r.RoutingRuleDto(
        id: widget.rule.id.isEmpty
            ? RoutingController.newRuleId()
            : widget.rule.id,
        ruleKind: widget.rule.ruleKind,
        port: _port.text.trim().isEmpty ? null : _port.text.trim(),
        network: _network.text.trim().isEmpty ? null : _network.text.trim(),
        inboundTag: _inbounds.toList(),
        hasInboundTag: _inbounds.isNotEmpty,
        outboundTag: outbound.isEmpty ? null : outbound,
        ip: ips,
        hasIp: ips.isNotEmpty,
        domain: domains,
        hasDomain: domains.isNotEmpty,
        protocol: _protocols.toList(),
        hasProtocol: _protocols.isNotEmpty,
        process: processes,
        hasProcess: processes.isNotEmpty,
        enabled: _enabled,
        remarks: _remarks.text.trim().isEmpty ? null : _remarks.text.trim(),
        ruleType: _ruleType,
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// R3-WPF-ROUTING-WINDOW: 路由设置 as an independent top-level window.
//
// The window runs in its own Flutter engine with a snapshot of the routing
// schemes (with their rules) and the global strategy. It has no Rust handle:
// every edit mutates a local draft, and 确定 relays the draft to the main
// engine, which persists it through the existing routing save path. 取消/Esc/
// title-bar close write nothing. Widget tests use an in-memory fake host; the
// embedded dialog is kept as a fallback for environments without the native
// host (e.g. `flutter test`).
// ---------------------------------------------------------------------------

/// One scheme plus its editable rules, as exchanged with the main engine.
class RoutingSchemeSnapshot {
  const RoutingSchemeSnapshot({required this.profile, required this.rules});

  final r.RoutingProfileDto profile;
  final List<r.RoutingRuleDto> rules;
}

/// The routing window's draft at 确定 time.
class RoutingDraft {
  const RoutingDraft({
    required this.schemes,
    required this.domainStrategy,
    required this.domainStrategySbox,
  });

  final List<RoutingSchemeSnapshot> schemes;
  final String domainStrategy;
  final String domainStrategySbox;
}

/// The routing window's starting snapshot.
class RoutingEditorSnapshot {
  const RoutingEditorSnapshot({
    required this.schemes,
    required this.domainStrategy,
    required this.domainStrategySbox,
    required this.outboundTags,
  });

  final List<RoutingSchemeSnapshot> schemes;
  final String domainStrategy;
  final String domainStrategySbox;
  final List<String> outboundTags;
}

/// Outcome of persisting a routing draft through the main engine.
class RoutingEditorOutcome {
  const RoutingEditorOutcome({required this.ok, this.message});

  final bool ok;

  /// User-facing error text; only set when [ok] is false.
  final String? message;
}

/// Thrown when the routing window cannot read its starting snapshot from the
/// main engine. R4-12/D34: a failed read must surface as an error, never as an
/// empty snapshot whose 确定 would delete every persisted routing scheme.
class RoutingEditorLoadException implements Exception {
  const RoutingEditorLoadException([this.message = '读取路由设置失败']);

  final String message;

  @override
  String toString() => 'RoutingEditorLoadException: $message';
}

/// Persistence/close seam for the routing window. The desktop implementation
/// talks to the main window through the native host; tests use a fake.
abstract class RoutingEditorHost {
  Future<RoutingEditorSnapshot> loadSnapshot();
  Future<RoutingEditorOutcome> save(RoutingDraft draft);
  Future<void> close();
}

/// R4-12/D31: optional capability for hosts that relay one original upstream
/// commit action (strategy change, scheme/sub-editor 确定, delete, set-default)
/// so it persists immediately, instead of being deferred to the whole-window
/// 确定/取消 draft model. Hosts without it keep the legacy draft path.
abstract class RoutingCommitHost {
  Future<RoutingEditorOutcome> commit(String actionJson);
}

Map<String, dynamic> routingRuleToJson(r.RoutingRuleDto rule) =>
    <String, dynamic>{
      'id': rule.id,
      'ruleKind': rule.ruleKind,
      'port': rule.port,
      'network': rule.network,
      'inboundTag': rule.inboundTag,
      'hasInboundTag': rule.hasInboundTag,
      'outboundTag': rule.outboundTag,
      'ip': rule.ip,
      'hasIp': rule.hasIp,
      'domain': rule.domain,
      'hasDomain': rule.hasDomain,
      'protocol': rule.protocol,
      'hasProtocol': rule.hasProtocol,
      'process': rule.process,
      'hasProcess': rule.hasProcess,
      'enabled': rule.enabled,
      'remarks': rule.remarks,
      'ruleType': rule.ruleType,
    };

List<String> _routingStrList(Object? value) {
  if (value is List) {
    return value.map((e) => e.toString()).toList();
  }
  return const <String>[];
}

r.RoutingRuleDto routingRuleFromJson(Map<String, dynamic> json) =>
    r.RoutingRuleDto(
      id: json['id'] as String? ?? RoutingController.newRuleId(),
      ruleKind: json['ruleKind'] as String?,
      port: json['port'] as String?,
      network: json['network'] as String?,
      inboundTag: _routingStrList(json['inboundTag']),
      hasInboundTag: json['hasInboundTag'] == true,
      outboundTag: json['outboundTag'] as String?,
      ip: _routingStrList(json['ip']),
      hasIp: json['hasIp'] == true,
      domain: _routingStrList(json['domain']),
      hasDomain: json['hasDomain'] == true,
      protocol: _routingStrList(json['protocol']),
      hasProtocol: json['hasProtocol'] == true,
      process: _routingStrList(json['process']),
      hasProcess: json['hasProcess'] == true,
      enabled: json['enabled'] != false,
      remarks: json['remarks'] as String?,
      ruleType: (json['ruleType'] as num?)?.toInt(),
    );

Map<String, dynamic> routingSchemeToJson(RoutingSchemeSnapshot scheme) {
  final d = scheme.profile;
  return <String, dynamic>{
    'id': d.id,
    'remarks': d.remarks,
    'url': d.url,
    'enabled': d.enabled,
    'locked': d.locked,
    'customIcon': d.customIcon,
    'customRulesetPath4Singbox': d.customRulesetPath4Singbox,
    'domainStrategy': d.domainStrategy,
    'domainStrategy4Singbox': d.domainStrategy4Singbox,
    'sort': d.sort,
    'isActive': d.isActive,
    'rules': scheme.rules.map(routingRuleToJson).toList(),
  };
}

RoutingSchemeSnapshot routingSchemeFromJson(Map<String, dynamic> json) {
  final rules = <r.RoutingRuleDto>[
    for (final entry in (json['rules'] as List? ?? const <Object>[]))
      if (entry is Map) routingRuleFromJson(entry.cast<String, dynamic>()),
  ];
  final profile = r.RoutingProfileDto(
    id: json['id'] as String? ?? '',
    remarks: json['remarks'] as String? ?? '',
    url: json['url'] as String? ?? '',
    ruleSet: RoutingController.rulesToRuleSetJson(rules),
    ruleNum: rules.length,
    enabled: json['enabled'] != false,
    locked: json['locked'] == true,
    customIcon: json['customIcon'] as String? ?? '',
    customRulesetPath4Singbox:
        json['customRulesetPath4Singbox'] as String? ?? '',
    domainStrategy: json['domainStrategy'] as String? ?? '',
    domainStrategy4Singbox: json['domainStrategy4Singbox'] as String? ?? '',
    sort: (json['sort'] as num?)?.toInt() ?? 0,
    isActive: json['isActive'] == true,
  );
  return RoutingSchemeSnapshot(profile: profile, rules: rules);
}

String encodeRoutingSnapshot(RoutingEditorSnapshot snapshot) => jsonEncode({
  'schemes': snapshot.schemes.map(routingSchemeToJson).toList(),
  'domainStrategy': snapshot.domainStrategy,
  'domainStrategy4Singbox': snapshot.domainStrategySbox,
  'outboundTags': snapshot.outboundTags,
});

RoutingEditorSnapshot decodeRoutingSnapshot(String text) {
  Object? decoded;
  try {
    decoded = jsonDecode(text);
  } catch (_) {
    decoded = null;
  }
  final map = decoded is Map
      ? decoded.cast<String, dynamic>()
      : <String, dynamic>{};
  return RoutingEditorSnapshot(
    schemes: _decodeRoutingSchemes(map),
    domainStrategy: map['domainStrategy'] as String? ?? '',
    domainStrategySbox: map['domainStrategy4Singbox'] as String? ?? '',
    outboundTags: _routingStrList(map['outboundTags']),
  );
}

String encodeRoutingDraft(RoutingDraft draft) => jsonEncode({
  'schemes': draft.schemes.map(routingSchemeToJson).toList(),
  'domainStrategy': draft.domainStrategy,
  'domainStrategy4Singbox': draft.domainStrategySbox,
});

List<RoutingSchemeSnapshot> _decodeRoutingSchemes(Map<String, dynamic> map) =>
    <RoutingSchemeSnapshot>[
      for (final entry in (map['schemes'] as List? ?? const <Object>[]))
        if (entry is Map) routingSchemeFromJson(entry.cast<String, dynamic>()),
    ];

/// Decoded routing draft (`{schemes, domainStrategy, domainStrategy4Singbox}`).
class RoutingDraftDecoded {
  const RoutingDraftDecoded({
    required this.schemes,
    required this.domainStrategy,
    required this.domainStrategySbox,
  });

  final List<RoutingSchemeSnapshot> schemes;
  final String domainStrategy;
  final String domainStrategySbox;
}

RoutingDraftDecoded? decodeRoutingDraft(String text) {
  Object? decoded;
  try {
    decoded = jsonDecode(text);
  } catch (_) {
    return null;
  }
  if (decoded is! Map) return null;
  final map = decoded.cast<String, dynamic>();
  return RoutingDraftDecoded(
    schemes: _decodeRoutingSchemes(map),
    domainStrategy: map['domainStrategy'] as String? ?? '',
    domainStrategySbox: map['domainStrategy4Singbox'] as String? ?? '',
  );
}

/// Main-window side of the native routing host. Registers the `applyDraft`
/// callback and asks the native side to open the independent window. Mirrors
/// [OptionWindowHost] on a distinct channel.
class RoutingWindowHost {
  RoutingWindowHost._();

  static final RoutingWindowHost instance = RoutingWindowHost._();

  static const MethodChannel _channel = MethodChannel('v2rayn/routing_window');

  Future<RoutingEditorOutcome> Function(String draftJson)? _save;
  bool _unavailable = false;

  /// True when the native host is absent (widget-test environment); callers
  /// should fall back to the embedded dialog instead of reporting a failure.
  bool get unavailable => _unavailable;

  Future<bool> open({
    required RoutingEditorSnapshot snapshot,
    required Future<RoutingEditorOutcome> Function(String draftJson) onSave,
  }) async {
    _save = onSave;
    _unavailable = false;
    _channel.setMethodCallHandler(_handle);
    try {
      final opened = await _channel
          .invokeMethod<bool>('open', encodeRoutingSnapshot(snapshot))
          .timeout(const Duration(seconds: 5));
      return opened ?? false;
    } on MissingPluginException {
      _unavailable = true;
      return false;
    } on TimeoutException {
      _unavailable = true;
      return false;
    } on PlatformException {
      return false;
    }
  }

  Future<dynamic> _handle(MethodCall call) async {
    if (call.method != 'applyDraft') return null;
    final args = (call.arguments as Map).cast<String, dynamic>();
    final id = args['id'];
    final draftJson = args['draft'] as String? ?? '{}';
    final save = _save;
    final outcome = save == null
        ? const RoutingEditorOutcome(ok: false, message: '保存路由设置失败')
        : await save(draftJson);
    await _channel.invokeMethod<void>('reportOutcome', <String, dynamic>{
      'id': id,
      'ok': outcome.ok,
      'message': outcome.message,
    });
    return null;
  }
}

/// Routing-window side of the native host. Runs in the second Flutter engine,
/// which has no Rust bridge handle; every mutation is relayed to the main
/// engine, which performs the actual save.
class NativeRoutingEditorHost implements RoutingEditorHost, RoutingCommitHost {
  NativeRoutingEditorHost() {
    _ready = _init();
  }

  static const MethodChannel _channel = MethodChannel('v2rayn/routing_window');

  final Map<int, Completer<RoutingEditorOutcome>> _pending =
      <int, Completer<RoutingEditorOutcome>>{};
  late final Future<void> _ready;
  int _nextId = 1;
  String _snapshotJson = '{}';
  bool _loadFailed = false;

  Future<void> _init() async {
    _channel.setMethodCallHandler(_handle);
    for (var attempt = 0; attempt < 50; attempt++) {
      try {
        final value = await _channel.invokeMethod<String>('ready');
        if (value != null && value.isNotEmpty && value != '{}') {
          _snapshotJson = value;
          _loadFailed = false;
          return;
        }
      } on MissingPluginException {
        // handler not installed yet
      } on PlatformException {
        // transient; retry
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    _loadFailed = true;
  }

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async {
    await _ready;
    final raw = _snapshotJson.trim();
    if (_loadFailed || raw.isEmpty || raw == '{}') {
      throw const RoutingEditorLoadException('读取路由设置失败');
    }
    return decodeRoutingSnapshot(raw);
  }

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    final id = _nextId++;
    final completer = Completer<RoutingEditorOutcome>();
    _pending[id] = completer;
    try {
      await _channel.invokeMethod<void>('saveDraft', <String, dynamic>{
        'id': id,
        'draft': encodeRoutingDraft(draft),
      });
    } catch (_) {
      _pending.remove(id);
      return const RoutingEditorOutcome(ok: false, message: '保存路由设置失败');
    }
    return completer.future;
  }

  @override
  Future<RoutingEditorOutcome> commit(String actionJson) async {
    // Reuses the existing saveDraft/saveOutcome channel round-trip; the main
    // engine dispatches on the embedded `kind` field (see
    // routing_actions._applyRoutingAction). No native protocol change needed.
    final id = _nextId++;
    final completer = Completer<RoutingEditorOutcome>();
    _pending[id] = completer;
    try {
      await _channel.invokeMethod<void>('saveDraft', <String, dynamic>{
        'id': id,
        'draft': actionJson,
      });
    } catch (_) {
      _pending.remove(id);
      return const RoutingEditorOutcome(ok: false, message: '提交路由更改失败');
    }
    return completer.future;
  }

  @override
  Future<void> close() async {
    try {
      await _channel.invokeMethod<void>('close');
    } catch (_) {}
  }

  Future<dynamic> _handle(MethodCall call) async {
    if (call.method != 'saveOutcome') return null;
    final args = (call.arguments as Map).cast<String, dynamic>();
    final id = args['id'] as int?;
    final completer = id == null ? null : _pending.remove(id);
    if (completer != null && !completer.isCompleted) {
      completer.complete(
        RoutingEditorOutcome(
          ok: args['ok'] == true,
          message: args['message'] as String?,
        ),
      );
    }
    return null;
  }
}

r.RoutingRuleDto _newRoutingRuleDraft() => r.RoutingRuleDto(
  id: RoutingController.newRuleId(),
  inboundTag: const <String>[],
  hasInboundTag: false,
  outboundTag: 'proxy',
  ip: const <String>[],
  hasIp: false,
  domain: const <String>[],
  hasDomain: false,
  protocol: const <String>[],
  hasProtocol: false,
  process: const <String>[],
  hasProcess: false,
  enabled: true,
  ruleType: 1,
);

/// Root widget of the independent routing window (second Flutter engine).
class RoutingWindowApp extends StatelessWidget {
  const RoutingWindowApp({super.key, required this.host});

  final RoutingEditorHost host;

  @override
  Widget build(BuildContext context) {
    // ProviderScope is required by the reused rule-details dialog, but this
    // engine never reads the Rust-backed routing controllers.
    return ProviderScope(
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildAppTheme(Brightness.light),
        home: RoutingEditorWindow(host: host),
      ),
    );
  }
}

/// Boots the routing window engine. Called by `routingWindowMain`
/// (`lib/main.dart`), the actual Flutter entrypoint symbol.
void runRoutingWindow() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(RoutingWindowApp(host: NativeRoutingEditorHost()));
}

/// The routing settings UI for the independent window. Mirrors the Wave K
/// structure (toolbar, strategy rows, five-column scheme gread) and adds the
/// upstream window's 确定/取消 semantics: 确定 persists+applies via the host,
/// 取消/Esc/title-bar close discard the draft.
class RoutingEditorWindow extends StatefulWidget {
  const RoutingEditorWindow({super.key, required this.host});

  final RoutingEditorHost host;

  @override
  State<RoutingEditorWindow> createState() => _RoutingEditorWindowState();
}

class _RoutingEditorWindowState extends State<RoutingEditorWindow> {
  bool _loading = true;
  bool _loadFailed = false;
  String? _error;
  String? _status;
  bool _busy = false;
  List<RoutingSchemeSnapshot> _schemes = const <RoutingSchemeSnapshot>[];
  String? _selectedId;
  String _domainStrategy = '';
  String _domainStrategySbox = '';
  List<String> _outboundTags = const <String>[];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final snapshot = await widget.host.loadSnapshot();
      if (!mounted) return;
      setState(() {
        _schemes = snapshot.schemes;
        _domainStrategy = snapshot.domainStrategy;
        _domainStrategySbox = snapshot.domainStrategySbox;
        _outboundTags = snapshot.outboundTags;
        _selectedId = _schemes.isEmpty ? null : _schemes.first.profile.id;
        _loading = false;
        _loadFailed = false;
        _error = null;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _loadFailed = true;
        _error = '读取路由设置失败';
      });
    }
  }

  void _retryLoad() {
    setState(() {
      _loading = true;
      _loadFailed = false;
      _error = null;
    });
    _load();
  }

  /// R4-12/D31: relay one upstream commit action through the host. On a host
  /// that supports it the edit is persisted immediately and is NOT undone by a
  /// later whole-window 取消; on a legacy/fake host the local draft is kept and
  /// the whole-window 确定 still persists it.
  Future<void> _commitAction(Map<String, dynamic> action) async {
    final Object host = widget.host;
    if (host is! RoutingCommitHost) {
      // Legacy/fake host: keep the change in the local draft; the whole-window
      // 确定 still persists it.
      if (mounted) setState(() => _status = '有未保存的更改');
      return;
    }
    final outcome = await host.commit(jsonEncode(action));
    if (!mounted) return;
    setState(() {
      if (outcome.ok) {
        _error = null;
        _status = '已保存';
      } else {
        _status = '有未保存的更改';
      }
    });
  }

  Future<void> _openSchemeEditor(RoutingSchemeSnapshot? existing) async {
    final edited = await showRoutingSchemeEditor(
      context,
      existing,
      _outboundTags,
    );
    if (edited == null || !mounted) return;
    setState(() {
      if (existing == null) {
        final id = edited.profile.id.isEmpty
            ? 'new-${DateTime.now().microsecondsSinceEpoch}'
            : edited.profile.id;
        final profile = r.RoutingProfileDto(
          id: id,
          remarks: edited.profile.remarks,
          url: edited.profile.url,
          ruleSet: edited.profile.ruleSet,
          ruleNum: edited.profile.ruleNum,
          enabled: edited.profile.enabled,
          locked: edited.profile.locked,
          customIcon: edited.profile.customIcon,
          customRulesetPath4Singbox: edited.profile.customRulesetPath4Singbox,
          domainStrategy: edited.profile.domainStrategy,
          domainStrategy4Singbox: edited.profile.domainStrategy4Singbox,
          sort: edited.profile.sort,
          isActive: _schemes.isEmpty,
        );
        _schemes = <RoutingSchemeSnapshot>[
          ..._schemes,
          RoutingSchemeSnapshot(profile: profile, rules: edited.rules),
        ];
        _selectedId = id;
      } else {
        _schemes = _schemes
            .map(
              (s) => s.profile.id == existing.profile.id
                  ? RoutingSchemeSnapshot(
                      profile: edited.profile,
                      rules: edited.rules,
                    )
                  : s,
            )
            .toList();
      }
    });
    // Sub-editor 确定 commits immediately (upstream `SaveSettingsAsync`), so a
    // later whole-window 取消 cannot discard it.
    await _commitAction(<String, dynamic>{
      'kind': 'saveScheme',
      'scheme': routingSchemeToJson(edited),
    });
  }

  Future<void> _deleteScheme(String id) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('删除路由方案', style: TextStyle(fontSize: 14)),
        content: const Text('确定删除选中的路由方案吗？'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('routing-delete-confirm'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    final wasActive = _schemes.any(
      (s) => s.profile.id == id && s.profile.isActive,
    );
    setState(() {
      _schemes = _schemes.where((s) => s.profile.id != id).toList();
      if (_schemes.isNotEmpty && wasActive) {
        _schemes = _schemes
            .map(
              (s) => s.profile.id == _schemes.first.profile.id
                  ? RoutingSchemeSnapshot(
                      profile: _copyProfile(s.profile, isActive: true),
                      rules: s.rules,
                    )
                  : s,
            )
            .toList();
      }
      if (_selectedId == id) {
        _selectedId = _schemes.isEmpty ? null : _schemes.first.profile.id;
      }
    });
    await _commitAction(<String, dynamic>{'kind': 'deleteScheme', 'id': id});
  }

  Future<void> _setDefault(String id) async {
    setState(() {
      _schemes = _schemes
          .map(
            (s) => RoutingSchemeSnapshot(
              profile: _copyProfile(s.profile, isActive: s.profile.id == id),
              rules: s.rules,
            ),
          )
          .toList();
    });
    await _commitAction(<String, dynamic>{'kind': 'setDefault', 'id': id});
  }

  /// Strategy changes persist immediately (upstream `RoutingSettingViewModel.
  /// SaveSettingsAsync`), not on whole-window 确定.
  Future<void> _onStrategyChanged(String value, {required bool sbox}) async {
    setState(() {
      if (sbox) {
        _domainStrategySbox = value;
      } else {
        _domainStrategy = value;
      }
    });
    await _commitAction(<String, dynamic>{
      'kind': 'strategy',
      'domainStrategy': _domainStrategy,
      'domainStrategySbox': _domainStrategySbox,
    });
  }

  Future<void> _importBuiltin() async {
    // Upstream `ConfigHandler.InitRouting(config, true)`; the Rust import use
    // case is a registered gap, so this is surfaced honestly.
    if (!mounted) return;
    ScaffoldMessenger.maybeOf(context)
        ?.showSnackBar(const SnackBar(content: Text('内置规则集已刷新（导入后端用例待接入）')));
  }

  Future<void> _ok() async {
    // R4-12/D34: never persist a whole draft built from a failed read.
    if (_busy || _loadFailed) return;
    setState(() => _busy = true);
    final outcome = await widget.host.save(
      RoutingDraft(
        schemes: _schemes,
        domainStrategy: _domainStrategy,
        domainStrategySbox: _domainStrategySbox,
      ),
    );
    if (!mounted) return;
    if (outcome.ok) {
      await widget.host.close();
      return;
    }
    setState(() {
      _busy = false;
      _error = outcome.message ?? '保存路由设置失败';
    });
  }

  void _cancel() {
    widget.host.close();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (_loadFailed) {
      // A failed read must not offer 确定: the local list is empty and saving it
      // would delete every persisted routing scheme (R4-12/D34).
      return Scaffold(
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                _error ?? '读取路由设置失败',
                key: const ValueKey('routing-load-error'),
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
              const SizedBox(height: 8),
              FilledButton(
                key: const ValueKey('routing-load-retry'),
                onPressed: _retryLoad,
                child: const Text('重试'),
              ),
            ],
          ),
        ),
      );
    }
    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.escape): _cancel,
      },
      child: Focus(
        autofocus: true,
        child: Scaffold(
          body: Padding(
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    TextButton.icon(
                      key: const ValueKey('routing-add'),
                      onPressed: () => _openSchemeEditor(null),
                      icon: const Icon(Icons.add, size: 16),
                      label: const Text('添加规则集'),
                    ),
                    const SizedBox(width: 4),
                    TextButton.icon(
                      key: const ValueKey('routing-import-builtin'),
                      onPressed: _importBuiltin,
                      icon: const Icon(Icons.download, size: 16),
                      label: const Text('一键导入规则集'),
                    ),
                  ],
                ),
                const Divider(height: 1),
                const SizedBox(height: 8),
                _strategyRow(
                  label: '域名解析策略',
                  key: const ValueKey('routing-domain-strategy'),
                  value: _domainStrategy,
                  options: domainStrategyOptions,
                  onChanged: (v) => _onStrategyChanged(v, sbox: false),
                ),
                const SizedBox(height: 4),
                _strategyRow(
                  label: 'sing-box 域名解析策略',
                  key: const ValueKey('routing-domain-strategy-sbox'),
                  value: _domainStrategySbox,
                  options: domainStrategySboxOptions,
                  onChanged: (v) => _onStrategyChanged(v, sbox: true),
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    '预定义规则集列表',
                    key: const ValueKey('routing-block-title'),
                    style: TextStyle(
                      fontSize: 13,
                      color: Theme.of(context).colorScheme.primary,
                      decoration: TextDecoration.underline,
                    ),
                  ),
                ),
                const SizedBox(height: 4),
                const _SchemeTableHeader(),
                const Divider(height: 1),
                Expanded(
                  child: _schemes.isEmpty
                      ? const Center(
                          key: ValueKey('routing-empty'),
                          child: Text('暂无路由方案'),
                        )
                      : ListView.builder(
                          key: const ValueKey('routing-list'),
                          itemCount: _schemes.length,
                          itemBuilder: (context, index) {
                            final scheme = _schemes[index];
                            return _SchemeRow(
                              item: scheme.profile,
                              selected: scheme.profile.id == _selectedId,
                              onTap: () => setState(
                                () => _selectedId = scheme.profile.id,
                              ),
                              onEdit: () => _openSchemeEditor(scheme),
                              onContextMenu: (position) => _showRowMenu(
                                context,
                                position,
                                scheme.profile,
                              ),
                            );
                          },
                        ),
                ),
                if (_error != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      _error!,
                      key: const ValueKey('routing-error'),
                      style: const TextStyle(fontSize: 11, color: Colors.red),
                    ),
                  ),
                if (_status != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      _status!,
                      key: const ValueKey('routing-status'),
                      style: const TextStyle(fontSize: 11, color: Colors.grey),
                    ),
                  ),
                const Divider(height: 1),
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: <Widget>[
                      TextButton(
                        key: const ValueKey('routing-cancel'),
                        onPressed: _cancel,
                        child: const Text('取消'),
                      ),
                      const SizedBox(width: 8),
                      FilledButton(
                        key: const ValueKey('routing-ok'),
                        onPressed: _busy ? null : _ok,
                        child: const Text('确定'),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _strategyRow({
    required String label,
    required ValueKey<String> key,
    required String value,
    required List<String> options,
    required ValueChanged<String> onChanged,
  }) {
    final primary = Theme.of(context).colorScheme.primary;
    return Row(
      children: <Widget>[
        Text(label, style: TextStyle(fontSize: 12, color: primary)),
        const SizedBox(width: 2),
        Icon(Icons.link, size: 14, color: primary),
        const SizedBox(width: 8),
        SizedBox(
          width: 300,
          child: DropdownButton<String>(
            key: key,
            value: options.contains(value) ? value : '',
            isExpanded: true,
            items: [
              for (final s in options)
                DropdownMenuItem(
                  value: s,
                  child: Text(s, style: const TextStyle(fontSize: 12)),
                ),
            ],
            onChanged: (v) => onChanged(v ?? ''),
          ),
        ),
      ],
    );
  }

  Future<void> _showRowMenu(
    BuildContext context,
    Offset globalPosition,
    r.RoutingProfileDto item,
  ) async {
    setState(() => _selectedId = item.id);
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    final action = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        globalPosition & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: <PopupMenuEntry<String>>[
        const PopupMenuItem<String>(value: 'add', child: Text('添加规则集')),
        PopupMenuItem<String>(
          value: 'remove',
          enabled: item.remarks.isNotEmpty,
          child: const Text('移除所选规则'),
        ),
        PopupMenuItem<String>(
          value: 'default',
          enabled: item.remarks.isNotEmpty,
          child: const Text('设为活动规则'),
        ),
        const PopupMenuDivider(),
        const PopupMenuItem<String>(value: 'import', child: Text('一键导入规则集')),
      ],
    );
    if (!mounted || action == null) return;
    switch (action) {
      case 'add':
        await _openSchemeEditor(null);
      case 'remove':
        await _deleteScheme(item.id);
      case 'default':
        await _setDefault(item.id);
      case 'import':
        await _importBuiltin();
    }
  }
}

r.RoutingProfileDto _copyProfile(
  r.RoutingProfileDto d, {
  bool? isActive,
  bool? enabled,
}) => r.RoutingProfileDto(
  id: d.id,
  remarks: d.remarks,
  url: d.url,
  ruleSet: d.ruleSet,
  ruleNum: d.ruleNum,
  enabled: enabled ?? d.enabled,
  locked: d.locked,
  customIcon: d.customIcon,
  customRulesetPath4Singbox: d.customRulesetPath4Singbox,
  domainStrategy: d.domainStrategy,
  domainStrategy4Singbox: d.domainStrategy4Singbox,
  sort: d.sort,
  isActive: isActive ?? d.isActive,
);

/// Rule-set editor for the independent window. Edits a local draft (scheme
/// metadata + rules) and returns it to the caller; nothing is persisted here.
Future<RoutingSchemeSnapshot?> showRoutingSchemeEditor(
  BuildContext context,
  RoutingSchemeSnapshot? scheme,
  List<String> outboundTags,
) {
  return showDialog<RoutingSchemeSnapshot>(
    context: context,
    barrierDismissible: false,
    builder: (_) =>
        _RoutingSchemeEditor(scheme: scheme, outboundTags: outboundTags),
  );
}

class _RoutingSchemeEditor extends StatefulWidget {
  const _RoutingSchemeEditor({this.scheme, required this.outboundTags});

  final RoutingSchemeSnapshot? scheme;
  final List<String> outboundTags;

  @override
  State<_RoutingSchemeEditor> createState() => _RoutingSchemeEditorState();
}

class _RoutingSchemeEditorState extends State<_RoutingSchemeEditor> {
  late TextEditingController _remarks;
  late TextEditingController _url;
  late TextEditingController _customIcon;
  late TextEditingController _rulesetPath;
  late TextEditingController _sort;
  bool _enabled = true;
  final Set<String> _selectedRuleIds = <String>{};
  List<r.RoutingRuleDto> _rules = const <r.RoutingRuleDto>[];

  bool get isNew => widget.scheme == null;

  @override
  void initState() {
    super.initState();
    final profile = widget.scheme?.profile;
    _remarks = TextEditingController(text: profile?.remarks ?? '');
    _url = TextEditingController(text: profile?.url ?? '');
    _customIcon = TextEditingController(text: profile?.customIcon ?? '');
    _rulesetPath = TextEditingController(
      text: profile?.customRulesetPath4Singbox ?? '',
    );
    _sort = TextEditingController(text: '${profile?.sort ?? 0}');
    _enabled = profile?.enabled ?? true;
    _rules = List<r.RoutingRuleDto>.of(
      widget.scheme?.rules ?? const <r.RoutingRuleDto>[],
    );
  }

  @override
  void dispose() {
    _remarks.dispose();
    _url.dispose();
    _customIcon.dispose();
    _rulesetPath.dispose();
    _sort.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      key: const ValueKey('routing-ruleset-window'),
      title: Text(
        isNew ? '新增路由方案' : '编辑路由方案',
        style: const TextStyle(fontSize: 15),
      ),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      content: SizedBox(
        width: 780,
        height: 520,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            _field('备注', _remarks, const ValueKey('ruleset-remarks')),
            _field('URL', _url, const ValueKey('ruleset-url')),
            Row(
              children: <Widget>[
                Expanded(
                  child: _field(
                    '自定义图标',
                    _customIcon,
                    const ValueKey('ruleset-icon'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: _field(
                    'sing-box 自定义规则集路径',
                    _rulesetPath,
                    const ValueKey('ruleset-path'),
                  ),
                ),
                const SizedBox(width: 8),
                SizedBox(
                  width: 90,
                  child: _field(
                    '排序',
                    _sort,
                    const ValueKey('ruleset-sort'),
                    numeric: true,
                  ),
                ),
                const SizedBox(width: 8),
                Row(
                  children: <Widget>[
                    const Text('启用', style: TextStyle(fontSize: 12)),
                    Switch(
                      key: const ValueKey('ruleset-enabled'),
                      value: _enabled,
                      onChanged: (v) => setState(() => _enabled = v),
                    ),
                  ],
                ),
              ],
            ),
            Row(
              children: <Widget>[
                _tool('新增规则', const ValueKey('rule-add'), _addRule),
                _tool(
                  '删除',
                  const ValueKey('rule-remove'),
                  _selectedRuleIds.isEmpty ? null : _confirmRemove,
                ),
                _tool(
                  '导出选中',
                  const ValueKey('rule-export'),
                  _selectedRuleIds.isEmpty ? null : _exportSelected,
                ),
                _tool('从剪贴板导入', const ValueKey('rule-import-clipboard'), () {
                  pickRulesFromClipboard(context).then(_mergeImported);
                }),
                _tool('从文件导入', const ValueKey('rule-import-file'), () {
                  pickRulesFromFile(context).then(_mergeImported);
                }),
                _tool('从URL导入', const ValueKey('rule-import-url'), () {
                  _importFromUrl();
                }),
              ],
            ),
            const _RuleTableHeader(),
            const Divider(height: 1),
            Expanded(
              child: _rules.isEmpty
                  ? const Center(child: Text('暂无规则，请新增或导入'))
                  : ListView.builder(
                      key: const ValueKey('rule-list'),
                      itemCount: _rules.length,
                      itemBuilder: (context, index) {
                        final rule = _rules[index];
                        return _RuleRow(
                          rule: rule,
                          index: index,
                          selected: _selectedRuleIds.contains(rule.id),
                          onTap: () => setState(() {
                            if (_selectedRuleIds.contains(rule.id)) {
                              _selectedRuleIds.remove(rule.id);
                            } else {
                              _selectedRuleIds.add(rule.id);
                            }
                          }),
                          onEdit: () => _editRule(rule),
                          onTop: () => _move(index, 0),
                          onUp: () => _move(index, 1),
                          onDown: () => _move(index, 2),
                          onBottom: () => _move(index, 3),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('ruleset-cancel'),
          onPressed: () => Navigator.pop(context),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('ruleset-save'),
          onPressed: _save,
          child: const Text('保存'),
        ),
      ],
    );
  }

  Widget _field(
    String label,
    TextEditingController controller,
    ValueKey<String> key, {
    bool numeric = false,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: TextField(
        key: key,
        controller: controller,
        keyboardType: numeric ? TextInputType.number : null,
        inputFormatters: numeric
            ? <TextInputFormatter>[FilteringTextInputFormatter.digitsOnly]
            : null,
        decoration: InputDecoration(
          labelText: label,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        style: const TextStyle(fontSize: 13),
      ),
    );
  }

  Widget _tool(String label, ValueKey<String> key, VoidCallback? onPressed) {
    return Padding(
      padding: const EdgeInsets.only(right: 4),
      child: TextButton(key: key, onPressed: onPressed, child: Text(label)),
    );
  }

  Future<void> _addRule() async {
    final saved = await showDialog<r.RoutingRuleDto>(
      context: context,
      builder: (_) => RoutingRuleDetailsDialog(
        rule: _newRoutingRuleDraft(),
        outboundTags: widget.outboundTags,
      ),
    );
    if (saved != null && mounted) {
      setState(() => _rules = <r.RoutingRuleDto>[..._rules, saved]);
    }
  }

  Future<void> _editRule(r.RoutingRuleDto rule) async {
    final saved = await showDialog<r.RoutingRuleDto>(
      context: context,
      builder: (_) => RoutingRuleDetailsDialog(
        rule: rule,
        outboundTags: widget.outboundTags,
      ),
    );
    if (saved != null && mounted) {
      setState(() {
        _rules = _rules.map((e) => e.id == rule.id ? saved : e).toList();
      });
    }
  }

  Future<void> _confirmRemove() async {
    if (_selectedRuleIds.isEmpty) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        key: const ValueKey('rule-delete-confirm-dialog'),
        title: const Text('删除规则', style: TextStyle(fontSize: 14)),
        content: Text('确定删除选中的 ${_selectedRuleIds.length} 条规则吗？'),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('rule-delete-cancel'),
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('rule-delete-confirm'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed == true && mounted) {
      setState(() {
        _rules = _rules.where((e) => !_selectedRuleIds.contains(e.id)).toList();
        _selectedRuleIds.clear();
      });
    }
  }

  void _mergeImported(({List<r.RoutingRuleDto> rules, bool replace})? picked) {
    if (picked == null || !mounted) return;
    setState(() {
      _rules = picked.replace
          ? List<r.RoutingRuleDto>.of(picked.rules)
          : <r.RoutingRuleDto>[..._rules, ...picked.rules];
    });
  }

  Future<void> _exportSelected() async {
    if (_selectedRuleIds.isEmpty) return;
    final text = RoutingController.exportDraftRulesJson(
      _rules,
      _selectedRuleIds.toList(),
    );
    await Clipboard.setData(ClipboardData(text: text));
  }

  void _move(int index, int direction) {
    setState(() {
      final list = List.of(_rules);
      var target = index;
      switch (direction) {
        case 0:
          target = 0;
        case 1:
          target = index - 1;
        case 2:
          target = index + 1;
        case 3:
          target = list.length - 1;
      }
      if (target >= 0 && target < list.length && target != index) {
        final item = list.removeAt(index);
        list.insert(target, item);
        _rules = list;
      }
    });
  }

  Future<void> _importFromUrl() async {
    final urlController = TextEditingController(text: _url.text);
    final url = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        key: const ValueKey('routing-import-url-dialog'),
        title: const Text('从 URL 导入规则', style: TextStyle(fontSize: 15)),
        content: SizedBox(
          width: 420,
          child: TextField(
            key: const ValueKey('routing-import-url-field'),
            controller: urlController,
            decoration: const InputDecoration(
              hintText: 'http(s)://…/rules.json',
              border: OutlineInputBorder(),
            ),
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('routing-import-url-ok'),
            onPressed: () => Navigator.pop(context, urlController.text),
            child: const Text('下载导入'),
          ),
        ],
      ),
    );
    if (url == null || !mounted) return;
    pickRulesFromUrl(context, url).then(_mergeImported);
  }

  void _save() {
    if (_remarks.text.trim().isEmpty) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('请填写备注')));
      return;
    }
    final existing = widget.scheme?.profile;
    final profile = r.RoutingProfileDto(
      id: existing?.id ?? '',
      remarks: _remarks.text.trim(),
      url: _url.text.trim(),
      ruleSet: RoutingController.rulesToRuleSetJson(_rules),
      ruleNum: _rules.length,
      enabled: _enabled,
      locked: existing?.locked ?? false,
      customIcon: _customIcon.text.trim(),
      customRulesetPath4Singbox: _rulesetPath.text.trim(),
      domainStrategy: existing?.domainStrategy ?? '',
      domainStrategy4Singbox: existing?.domainStrategy4Singbox ?? '',
      sort: int.tryParse(_sort.text.trim()) ?? 0,
      isActive: existing?.isActive ?? false,
    );
    Navigator.pop(
      context,
      RoutingSchemeSnapshot(profile: profile, rules: _rules),
    );
  }
}
