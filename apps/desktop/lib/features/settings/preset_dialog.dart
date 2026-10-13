import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import '../routing/routing_controller.dart';
import 'settings_controller.dart';
import 'settings_defaults.dart';

/// UB2-10: one-click recommended scenarios (UX3 §4.2, scenarios A/B/C/D).
///
/// Hard rules (approval condition E + UX3 §6):
/// - `SysProxyType` is never written: every scenario lists it as a manual
///   step the user confirms on the proxy page.
/// - TUN is never turned on: scenarios only ever write `EnableTun: false`,
///   and only when it currently differs (diff-driven, shown in the preview).
/// - Ports are frozen: no scenario touches `Inbound.LocalPort`.
/// - No credentials: no scenario touches `User`/`Pass`.
/// - The diff preview is forced: apply only writes fields the preview listed.
/// - Undo is disabled: there is no settings-snapshot API yet (UX3-12); the
///   dialog says so and points at the backup page first.

/// One recommended scenario: an id, copy, the routing scheme it suggests,
/// the exact settings writes, and the human-side steps.
class PresetScenario {
  const PresetScenario({
    required this.id,
    required this.title,
    required this.description,
    required this.schemeKeyword,
    required this.writes,
    required this.manualSteps,
    required this.keepNotes,
  });

  final String id;
  final String title;
  final String description;

  /// Substring matched against routing scheme remarks to preselect the target.
  final String schemeKeyword;

  /// group -> {key: desired value}. `Inbound` patches the first list row.
  /// Never contains SysProxyType / LocalPort / User / Pass (tested).
  final Map<String, Map<String, Object?>> writes;
  final List<String> manualSteps;
  final List<String> keepNotes;
}

/// One field the preview will write: `Group.Key: oldValue -> newValue`.
class PresetDiffRow {
  const PresetDiffRow(this.group, this.key, this.oldValue, this.newValue);

  final String group;
  final String key;
  final Object? oldValue;
  final Object? newValue;

  String get locator => 'settings-preset-diff-$group.$key';
}

const _cloudflareDoh = 'https://cloudflare-dns.com/dns-query';
const _directDns = '119.29.29.29';

const _sysproxyManual = '系统代理：方案永不自动改，需要时请到"代理与 TUN"页手动确认后再改。';
const _portsFrozen = '端口冻结：主端口（LocalPort）保持你的现状，方案不碰。';

/// UX3 §4.2 scenario tables as data. Order is fixed: A/B/C/D.
///
/// A, B and D share the §4.2 DNS + inbound targets; C deliberately has no
/// DNS writes and a narrower inbound patch (low latency = fewer moving
/// parts); D additionally pins AllowLANConn off.
List<PresetScenario> presetScenarios() => const <PresetScenario>[
  PresetScenario(
    id: 'A',
    title: '日常浏览 · 绕过大陆（默认推荐）',
    description: '国内直连、国外走代理，改动最小；分流靠域名＋嗅探，DNS 双通道防污染。',
    schemeKeyword: '绕过大陆',
    writes: <String, Map<String, Object?>>{
      'RoutingBasicItem': <String, Object?>{'DomainStrategy': 'AsIs'},
      'SimpleDNSItem': <String, Object?>{
        'RemoteDNS': _cloudflareDoh,
        'DirectDNS': _directDns,
        'BootstrapDNS': _directDns,
        'FakeIP': false,
      },
      'Inbound': <String, Object?>{
        'SniffingEnabled': true,
        'DestOverride': ['http', 'tls'],
        'UdpEnabled': true,
      },
      'TunModeItem': <String, Object?>{'EnableTun': false},
    },
    manualSteps: <String>[
      '系统代理：方案建议"设置"，但按硬约束永不自动写，请到"代理与 TUN"页手动确认后再改。',
      'BlockAAAA：纯 IPv4 宽带可手动开，有 IPv6 需求保持关。',
      _portsFrozen,
    ],
    keepNotes: <String>[
      'Mux / Fragment / 指纹 / 日志：保持你的现状，方案不碰。',
      'FakeIP：保持关闭（局域网共享/直连应用多时开会坏）。',
    ],
  ),
  PresetScenario(
    id: 'B',
    title: '全局代理',
    description: '全部流量走代理；直连验货（DirectExpectedIPs）将被清空。',
    schemeKeyword: '全局',
    writes: <String, Map<String, Object?>>{
      'RoutingBasicItem': <String, Object?>{'DomainStrategy': 'AsIs'},
      'SimpleDNSItem': <String, Object?>{
        'RemoteDNS': _cloudflareDoh,
        'DirectDNS': _directDns,
        'BootstrapDNS': _directDns,
        'FakeIP': false,
        'DirectExpectedIPs': null,
      },
      'Inbound': <String, Object?>{
        'SniffingEnabled': true,
        'DestOverride': ['http', 'tls'],
        'UdpEnabled': true,
      },
      'TunModeItem': <String, Object?>{'EnableTun': false},
    },
    manualSteps: <String>[
      _sysproxyManual,
      'FakeIP：无直连应用混用时可手动开（减少 DNS 泄漏），有混用保持关。',
      _portsFrozen,
    ],
    keepNotes: <String>['Mux / Fragment / 指纹 / 日志：保持你的现状，方案不碰。'],
  ),
  PresetScenario(
    id: 'C',
    title: '游戏 / 低延迟',
    description: '少中间环节＋UDP 通畅；无实测加速参数，不碰 Mux/分片调参（效果待核实）。',
    schemeKeyword: '绕过大陆',
    writes: <String, Map<String, Object?>>{
      'Inbound': <String, Object?>{'UdpEnabled': true, 'SniffingEnabled': true},
      'CoreBasicItem': <String, Object?>{'EnableFragment': false},
      'TunModeItem': <String, Object?>{'EnableTun': false},
    },
    manualSteps: <String>[
      '活动方案：外服/加速器分区用"全局"，国服直连多用"绕过大陆"，请在下方下拉里二选一。',
      'HappyEyeballs：双栈宽带有益，可到 DNS 页手动开（效果待核实）。',
      _sysproxyManual,
      _portsFrozen,
    ],
    keepNotes: <String>[
      'Mux 并发保持现状：调参加速效果待核实，方案不碰（本场景明确无实测加速参数）。',
      'Fragment 参数 / 指纹 / 日志 / DNS：保持你的现状。',
    ],
  ),
  PresetScenario(
    id: 'D',
    title: '仅浏览器 · 不改系统',
    description: '系统代理保持"不改变"，只在浏览器里填本机端口；其他软件零感知。',
    schemeKeyword: '绕过大陆',
    writes: <String, Map<String, Object?>>{
      'RoutingBasicItem': <String, Object?>{'DomainStrategy': 'AsIs'},
      'SimpleDNSItem': <String, Object?>{
        'RemoteDNS': _cloudflareDoh,
        'DirectDNS': _directDns,
        'BootstrapDNS': _directDns,
        'FakeIP': false,
      },
      'Inbound': <String, Object?>{
        'SniffingEnabled': true,
        'DestOverride': ['http', 'tls'],
        'UdpEnabled': true,
        'AllowLANConn': false,
      },
      'TunModeItem': <String, Object?>{'EnableTun': false},
    },
    manualSteps: <String>[
      '系统代理：请手动设为"不改变"，由浏览器插件/SwitchyOmega 填本机端口（现端口不动）。',
      _portsFrozen,
    ],
    keepNotes: <String>[
      'Mux / Fragment / 指纹 / 日志：保持你的现状，方案不碰。',
      'AllowLANConn 以外局域网项：保持现状。',
    ],
  ),
];

/// Diff-driven preview: only fields whose desired value differs from the
/// merged persisted document are returned (and later written). Missing keys
/// read through the canonical defaults so older documents without
/// null-default keys do not count as dirty.
List<PresetDiffRow> computePresetDiff(
  Map<String, dynamic> persisted,
  PresetScenario scenario,
) {
  final merged = mergeWithSettingsDefaults(
    jsonDecode(jsonEncode(persisted)) as Map<String, dynamic>,
  );
  final rows = <PresetDiffRow>[];
  scenario.writes.forEach((group, patch) {
    if (group == 'Inbound') {
      final list = merged['Inbound'];
      final oldRow = list is List && list.isNotEmpty && list.first is Map
          ? Map<String, dynamic>.from(list.first as Map)
          : <String, dynamic>{};
      patch.forEach((key, want) {
        if (!settingsValueEquals(oldRow[key], want)) {
          rows.add(PresetDiffRow(group, key, oldRow[key], want));
        }
      });
      return;
    }
    final oldGroup = merged[group];
    final oldMap = oldGroup is Map
        ? Map<String, dynamic>.from(oldGroup)
        : <String, dynamic>{};
    patch.forEach((key, want) {
      if (!settingsValueEquals(oldMap[key], want)) {
        rows.add(PresetDiffRow(group, key, oldMap[key], want));
      }
    });
  });
  return rows;
}

String _short(Object? value) {
  if (value == null) return '(空)';
  final text = value is String ? value : jsonEncode(value);
  return text.length > 40 ? '${text.substring(0, 40)}…' : text;
}

/// Outcome popped by the dialog: which settings groups were rewritten and
/// whether the active routing scheme changed. The host window reseeds its
/// draft from these groups.
class PresetApplyOutcome {
  const PresetApplyOutcome(this.groups, this.wroteCount, this.routingChanged);

  final Set<String> groups;
  final int wroteCount;
  final bool routingChanged;
}

/// Opens the preset dialog. [readDocument] returns the persisted document in
/// the independent window, or the live draft in host mode. [writeGroup]
/// persists one full group (independent) or merges into the draft (host: no
/// direct writes, the host relay still owns saving). Returns the outcome, or
/// null when the user cancels.
Future<PresetApplyOutcome?> showPresetDialog({
  required BuildContext context,
  required bool hostMode,
  required Map<String, dynamic> Function() readDocument,
  required Future<bool> Function(String group, Object? fullValue) writeGroup,
}) => showDialog<PresetApplyOutcome>(
  context: context,
  builder: (dialogContext) => Dialog(
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 560, maxHeight: 640),
      child: PresetDialog(
        hostMode: hostMode,
        readDocument: readDocument,
        writeGroup: writeGroup,
      ),
    ),
  ),
);

class PresetDialog extends ConsumerStatefulWidget {
  const PresetDialog({
    super.key,
    required this.hostMode,
    required this.readDocument,
    required this.writeGroup,
  });

  final bool hostMode;
  final Map<String, dynamic> Function() readDocument;
  final Future<bool> Function(String group, Object? fullValue) writeGroup;

  @override
  ConsumerState<PresetDialog> createState() => _PresetDialogState();
}

class _PresetDialogState extends ConsumerState<PresetDialog> {
  String _scenarioId = 'A';
  String? _schemeId;
  String? _error;

  PresetScenario get _scenario =>
      presetScenarios().firstWhere((s) => s.id == _scenarioId);

  List<PresetDiffRow> _diffs() {
    try {
      return computePresetDiff(widget.readDocument(), _scenario);
    } catch (_) {
      return const <PresetDiffRow>[];
    }
  }

  /// Routing schemes known to the controller (may be empty before load).
  List<({String id, String remarks, bool isActive})> _schemes() {
    try {
      return ref
          .watch(routingControllerProvider)
          .items
          .map((s) => (id: s.id, remarks: s.remarks, isActive: s.isActive))
          .toList();
    } catch (_) {
      return const [];
    }
  }

  String? _activeSchemeId() {
    for (final s in _schemes()) {
      if (s.isActive) return s.id;
    }
    return null;
  }

  void _ensureSchemePreselect() {
    if (_schemeId != null) return;
    final active = _activeSchemeId();
    if (active != null) {
      _schemeId = active;
      return;
    }
    for (final s in _schemes()) {
      if (s.remarks.contains(_scenario.schemeKeyword)) {
        _schemeId = s.id;
        return;
      }
    }
  }

  Future<void> _apply(List<PresetDiffRow> diffs, bool routingChange) async {
    final byGroup = <String, Map<String, Object?>>{};
    for (final d in diffs) {
      byGroup.putIfAbsent(d.group, () => <String, Object?>{})[d.key] =
          d.newValue;
    }
    for (final entry in byGroup.entries) {
      final Object? fullValue = entry.key == 'Inbound'
          ? _inboundWith(entry.value)
          : _groupWith(entry.key, entry.value);
      if (fullValue == null) {
        if (mounted) setState(() => _error = '预设数据缺失，未写入任何内容');
        return;
      }
      final ok = await widget.writeGroup(entry.key, fullValue);
      if (!mounted) return;
      if (!ok) {
        setState(() => _error = '保存 ${entry.key} 失败，已写入的组保持不动');
        return;
      }
    }
    if (routingChange && _schemeId != null) {
      if (widget.hostMode) {
        if (mounted) {
          setState(() => _error = 'host 模式下路由方案请到路由页手动切换，未自动改');
        }
        return;
      }
      final result = ref
          .read(routingControllerProvider.notifier)
          .setDefault(_schemeId!);
      if (!mounted) return;
      if (!result.ok) {
        setState(() => _error = '切换路由方案失败，设置项已写入');
        return;
      }
    }
    if (!mounted) return;
    Navigator.of(context).pop(
      PresetApplyOutcome(byGroup.keys.toSet(), diffs.length, routingChange),
    );
  }

  /// Full Inbound list value with the first-row patch applied.
  Object? _inboundWith(Map<String, Object?> patch) {
    try {
      final doc = widget.readDocument();
      final list = doc['Inbound'];
      if (list is! List || list.isEmpty || list.first is! Map) return null;
      final rows = (jsonDecode(jsonEncode(list)) as List)
          .map((e) => Map<String, dynamic>.from(e as Map))
          .toList();
      rows[0].addAll(patch);
      return rows;
    } catch (_) {
      return null;
    }
  }

  /// Full group map value with the patch applied.
  Object? _groupWith(String group, Map<String, Object?> patch) {
    try {
      final doc = widget.readDocument();
      final current = doc[group];
      final base = current is Map
          ? Map<String, dynamic>.from(current)
          : <String, dynamic>{};
      base.addAll(patch);
      return base;
    } catch (_) {
      return null;
    }
  }

  @override
  Widget build(BuildContext context) {
    _ensureSchemePreselect();
    final diffs = _diffs();
    final schemes = _schemes();
    final routingChange = _schemeId != null && _schemeId != _activeSchemeId();
    final hasWork = diffs.isNotEmpty || routingChange;

    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          const Text(
            '一键推荐方案',
            style: TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
          ),
          const SizedBox(height: 4),
          Text(
            '先预览、再应用；只写预览列出的项。应用前建议先到备份页做一次本地备份。',
            style: TextStyle(
              fontSize: AppTokens.fontSizeCaption,
              color: context.semantics.muted,
            ),
          ),
          const SizedBox(height: 8),
          Flexible(
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  ...presetScenarios().map(
                    (s) => RadioListTile<String>(
                      key: ValueKey('settings-preset-scenario-${s.id}'),
                      value: s.id,
                      // ignore: deprecated_member_use
                      groupValue: _scenarioId,
                      // ignore: deprecated_member_use
                      onChanged: (v) => setState(() {
                        _scenarioId = v ?? 'A';
                        _schemeId = null;
                        _error = null;
                      }),
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(
                        s.title,
                        style: const TextStyle(fontSize: AppTokens.fontSize),
                      ),
                      subtitle: Text(
                        s.description,
                        style: TextStyle(
                          fontSize: AppTokens.fontSizeCaption,
                          color: context.semantics.muted,
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    '活动路由方案',
                    style: TextStyle(fontSize: AppTokens.fontSize),
                  ),
                  if (schemes.isEmpty)
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            '路由方案列表为空：点"导入内置规则"，或到路由页查看后再来。',
                            style: TextStyle(
                              fontSize: AppTokens.fontSizeCaption,
                              color: context.semantics.muted,
                            ),
                          ),
                        ),
                        TextButton(
                          key: const ValueKey('settings-preset-import-builtin'),
                          onPressed: () async {
                            ref
                                .read(routingControllerProvider.notifier)
                                .importBuiltin();
                            if (mounted) setState(() {});
                          },
                          child: const Text('导入内置规则'),
                        ),
                      ],
                    )
                  else
                    DropdownButton<String>(
                      key: const ValueKey('settings-preset-scheme'),
                      value: schemes.any((s) => s.id == _schemeId)
                          ? _schemeId
                          : null,
                      hint: const Text('选择活动方案'),
                      isExpanded: true,
                      items: [
                        for (final s in schemes)
                          DropdownMenuItem<String>(
                            value: s.id,
                            child: Text(
                              s.remarks.isEmpty ? s.id : s.remarks,
                              style: const TextStyle(
                                fontSize: AppTokens.fontSize,
                              ),
                            ),
                          ),
                      ],
                      onChanged: (v) => setState(() => _schemeId = v),
                    ),
                  const SizedBox(height: 8),
                  Text(
                    '将写入（${diffs.length}）${routingChange ? '＋切换路由方案' : ''}',
                    style: const TextStyle(fontSize: AppTokens.fontSize),
                  ),
                  if (diffs.isEmpty && !routingChange)
                    Text(
                      '已全部符合，无需写入。',
                      style: TextStyle(
                        fontSize: AppTokens.fontSizeCaption,
                        color: context.semantics.muted,
                      ),
                    ),
                  for (final d in diffs)
                    Padding(
                      key: ValueKey(d.locator),
                      padding: const EdgeInsets.only(left: 8, top: 2),
                      child: Text(
                        '${d.group}.${d.key}：${_short(d.oldValue)} → ${_short(d.newValue)}',
                        style: const TextStyle(
                          fontSize: AppTokens.fontSizeSmall,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ),
                  const SizedBox(height: 6),
                  const Text(
                    '需手动（方案永不自动做）',
                    style: TextStyle(fontSize: AppTokens.fontSize),
                  ),
                  for (final step in _scenario.manualSteps)
                    Padding(
                      padding: const EdgeInsets.only(left: 8, top: 2),
                      child: Text(
                        '· $step',
                        style: TextStyle(
                          fontSize: AppTokens.fontSizeCaption,
                          color: context.semantics.muted,
                        ),
                      ),
                    ),
                  const SizedBox(height: 6),
                  const Text(
                    '保持现状（方案不碰）',
                    style: TextStyle(fontSize: AppTokens.fontSize),
                  ),
                  for (final note in _scenario.keepNotes)
                    Padding(
                      padding: const EdgeInsets.only(left: 8, top: 2),
                      child: Text(
                        '· $note',
                        style: TextStyle(
                          fontSize: AppTokens.fontSizeCaption,
                          color: context.semantics.muted,
                        ),
                      ),
                    ),
                  const SizedBox(height: 6),
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: Text(
                          '撤销暂不可用：没有方案快照 API（UX3-12），应用前请先备份。',
                          style: TextStyle(
                            fontSize: AppTokens.fontSizeCaption,
                            color: context.semantics.warning,
                          ),
                        ),
                      ),
                      TextButton(
                        key: const ValueKey('settings-preset-undo'),
                        onPressed: null,
                        child: const Text('撤销'),
                      ),
                    ],
                  ),
                  if (_error != null)
                    Text(
                      _error!,
                      style: TextStyle(
                        fontSize: AppTokens.fontSizeCaption,
                        color: context.semantics.warning,
                      ),
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              TextButton(
                key: const ValueKey('settings-preset-cancel'),
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('取消'),
              ),
              const SizedBox(width: 8),
              FilledButton(
                key: const ValueKey('settings-preset-apply'),
                onPressed: hasWork ? () => _apply(diffs, routingChange) : null,
                child: const Text('应用'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
