import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_actions.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';

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
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(routingControllerProvider);
    final controller = ref.read(routingControllerProvider.notifier);
    return AlertDialog(
      key: const ValueKey('routing-setting-window'),
      title: const Text('路由设置', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      content: SizedBox(
        width: 760,
        height: 460,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              children: <Widget>[
                const Text('域名策略', style: TextStyle(fontSize: 12)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  key: const ValueKey('routing-domain-strategy'),
                  value: _strategyValue(state),
                  items: [
                    for (final s in domainStrategyOptions)
                      DropdownMenuItem(
                        value: s,
                        child: Text(
                          s.isEmpty ? '(空)' : s,
                          style: const TextStyle(fontSize: 12),
                        ),
                      ),
                  ],
                  onChanged: (v) => _saveStrategy(v ?? ''),
                ),
                const SizedBox(width: 16),
                const Text('域名策略 (sing-box)', style: TextStyle(fontSize: 12)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  key: const ValueKey('routing-domain-strategy-sbox'),
                  value: _strategySboxValue(state),
                  items: [
                    for (final s in domainStrategySboxOptions)
                      DropdownMenuItem(
                        value: s,
                        child: Text(
                          s.isEmpty ? '(空)' : s,
                          style: const TextStyle(fontSize: 12),
                        ),
                      ),
                  ],
                  onChanged: (v) => _saveStrategySbox(v ?? ''),
                ),
              ],
            ),
            const SizedBox(height: 8),
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
                          onSetDefault: () => controller.setDefault(item.id),
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
      actions: <Widget>[
        TextButton(
          key: const ValueKey('routing-add'),
          onPressed: () => _openRuleset(null),
          child: const Text('添加'),
        ),
        TextButton(
          key: const ValueKey('routing-remove'),
          onPressed: state.selectedId == null
              ? null
              : () => _confirmDelete(state.selectedId!),
          child: const Text('删除'),
        ),
        TextButton(
          key: const ValueKey('routing-set-default'),
          onPressed: state.selectedId == null
              ? null
              : () => controller.setDefault(state.selectedId!),
          child: const Text('设为默认'),
        ),
        TextButton(
          key: const ValueKey('routing-close'),
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    );
  }

  String _strategyValue(RoutingState state) {
    final selected = state.selected;
    final value = selected?.domainStrategy ?? '';
    return domainStrategyOptions.contains(value) ? value : '';
  }

  String _strategySboxValue(RoutingState state) {
    final selected = state.selected;
    final value = selected?.domainStrategy4Singbox ?? '';
    return domainStrategySboxOptions.contains(value) ? value : '';
  }

  void _saveStrategy(String value) {
    final controller = ref.read(routingControllerProvider.notifier);
    final selected = ref.read(routingControllerProvider).selected;
    if (selected == null) return;
    controller.save(_withStrategy(selected, value, null));
  }

  void _saveStrategySbox(String value) {
    final controller = ref.read(routingControllerProvider.notifier);
    final selected = ref.read(routingControllerProvider).selected;
    if (selected == null) return;
    controller.save(_withStrategy(selected, null, value));
  }

  r.RoutingProfileDto _withStrategy(
    r.RoutingProfileDto item,
    String? strategy,
    String? strategySbox,
  ) => r.RoutingProfileDto(
    id: item.id,
    remarks: item.remarks,
    url: item.url,
    ruleSet: item.ruleSet,
    ruleNum: item.ruleNum,
    enabled: item.enabled,
    locked: item.locked,
    customIcon: item.customIcon,
    customRulesetPath4Singbox: item.customRulesetPath4Singbox,
    domainStrategy: strategy ?? item.domainStrategy,
    domainStrategy4Singbox: strategySbox ?? item.domainStrategy4Singbox,
    sort: item.sort,
    isActive: item.isActive,
  );

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
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      child: Row(
        children: <Widget>[
          Expanded(flex: 3, child: Text('备注', style: style)),
          Expanded(child: Text('规则数', style: style)),
          Expanded(child: Text('排序', style: style)),
          Expanded(flex: 3, child: Text('URL', style: style)),
          Expanded(child: Text('状态', style: style)),
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
    required this.onSetDefault,
  });

  final r.RoutingProfileDto item;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onEdit;
  final VoidCallback onSetDefault;

  @override
  Widget build(BuildContext context) {
    final flags = <String>[
      if (!item.enabled) '禁用',
      if (item.locked) '锁定',
      if (item.isActive) '默认',
    ].join(' ');
    return InkWell(
      key: ValueKey('routing-row-${item.id}'),
      onTap: onTap,
      onDoubleTap: onEdit,
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
            Expanded(child: Text(flags, style: const TextStyle(fontSize: 12))),
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
                  _selectedRuleIds.isEmpty ? null : _removeSelected,
                ),
                _tool(
                  '导出选中',
                  const ValueKey('rule-export'),
                  _selectedRuleIds.isEmpty ? null : _exportSelected,
                ),
                _tool('从剪贴板导入', const ValueKey('rule-import-clipboard'), () {
                  final id = widget.item?.id;
                  if (id == null) return;
                  importRulesFromClipboard(
                    context,
                    ref,
                    id,
                  ).then((_) => _loadRules(id));
                }),
                _tool('从文件导入', const ValueKey('rule-import-file'), () {
                  final id = widget.item?.id;
                  if (id == null) return;
                  importRulesFromFile(
                    context,
                    ref,
                    id,
                  ).then((_) => _loadRules(id));
                }),
                _tool('从URL导入', const ValueKey('rule-import-url'), () {
                  final id = widget.item?.id;
                  if (id == null) return;
                  _importFromUrl(id);
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

  void _removeSelected() {
    setState(() {
      _rules = _rules.where((e) => !_selectedRuleIds.contains(e.id)).toList();
      _selectedRuleIds.clear();
    });
  }

  Future<void> _exportSelected() async {
    final id = widget.item?.id;
    if (id == null) {
      // Unsaved scheme: export from the in-memory draft list.
      await Clipboard.setData(
        ClipboardData(
          text: _rules
              .where((e) => _selectedRuleIds.contains(e.id))
              .map((e) => e.outboundTag ?? '')
              .join('\n'),
        ),
      );
      return;
    }
    await exportSelectedRules(context, ref, id, _selectedRuleIds.toList());
  }

  void _move(int index, int direction) {
    final id = widget.item?.id;
    if (id == null) {
      // Unsaved scheme: reorder the in-memory draft list.
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
      return;
    }
    ref.read(routingControllerProvider.notifier).moveRule(id, index, direction);
    _loadRules(id);
  }

  Future<void> _importFromUrl(String id) async {
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
    await importRulesFromUrl(context, ref, id, url);
    if (mounted) _loadRules(id);
  }

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
      ruleSet: existing?.ruleSet ?? '[]',
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
    final routingId = saved.item?.id;
    if (routingId != null && _rules.isNotEmpty) {
      // Persist the edited rule list (ids kept stable across edits).
      final withIds = _rules.map((e) {
        if (e.id.isNotEmpty) return e;
        return r.RoutingRuleDto(
          id: 'draft-${e.hashCode}',
          ruleKind: e.ruleKind,
          port: e.port,
          network: e.network,
          inboundTag: e.inboundTag,
          hasInboundTag: e.hasInboundTag,
          outboundTag: e.outboundTag,
          ip: e.ip,
          hasIp: e.hasIp,
          domain: e.domain,
          hasDomain: e.hasDomain,
          protocol: e.protocol,
          hasProtocol: e.hasProtocol,
          process: e.process,
          hasProcess: e.hasProcess,
          enabled: e.enabled,
          remarks: e.remarks,
          ruleType: e.ruleType,
        );
      }).toList();
      controller.saveRules(routingId, withIds);
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
    return InkWell(
      key: ValueKey('rule-row-$index'),
      onTap: onTap,
      onDoubleTap: onEdit,
      child: Container(
        color: selected ? Colors.blue.withValues(alpha: 0.08) : null,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
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
  const RoutingRuleDetailsDialog({super.key, required this.rule});

  final r.RoutingRuleDto rule;

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
  Future<void> _selectProfile() async {
    final bridge = ref.read(bridgePortProvider);
    final profiles = bridge.queryAllProfiles();
    final remarks = profiles.map((p) => p.remarks).toList()..sort();
    final options = <String>['proxy', 'direct', 'block', ...remarks];
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
    if (_outbound.text.trim().isEmpty) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('请填写出站标签或选择节点')));
      return;
    }
    Navigator.pop(
      context,
      r.RoutingRuleDto(
        id: widget.rule.id,
        port: _port.text.trim().isEmpty ? null : _port.text.trim(),
        network: _network.text.trim().isEmpty ? null : _network.text.trim(),
        inboundTag: _inbounds.toList(),
        hasInboundTag: _inbounds.isNotEmpty,
        outboundTag: _outbound.text.trim(),
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
