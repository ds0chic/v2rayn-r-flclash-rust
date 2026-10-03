import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

final routingControllerProvider =
    NotifierProvider<RoutingController, RoutingState>(RoutingController.new);

/// Routing settings window state (upstream `RoutingSettingViewModel`,
/// F-ROUTING-002). The list mirrors the `RoutingItem` table in `Sort` order;
/// the active row is the default routing.
class RoutingState {
  const RoutingState({
    this.items = const <r.RoutingProfileDto>[],
    this.selectedId,
    this.rules = const <r.RoutingRuleDto>[],
    this.warnings = const <r.RoutingWarningDto>[],
    this.ruleMode = 'Rule',
    this.busy = false,
    this.status,
  });

  final List<r.RoutingProfileDto> items;
  final String? selectedId;
  final List<r.RoutingRuleDto> rules;
  final List<r.RoutingWarningDto> warnings;
  final String ruleMode;
  final bool busy;
  final String? status;

  r.RoutingProfileDto? get selected {
    for (final item in items) {
      if (item.id == selectedId) return item;
    }
    return null;
  }

  RoutingState copyWith({
    List<r.RoutingProfileDto>? items,
    String? selectedId,
    bool clearSelected = false,
    List<r.RoutingRuleDto>? rules,
    List<r.RoutingWarningDto>? warnings,
    String? ruleMode,
    bool? busy,
    String? status,
  }) => RoutingState(
    items: items ?? this.items,
    selectedId: clearSelected ? null : (selectedId ?? this.selectedId),
    rules: rules ?? this.rules,
    warnings: warnings ?? this.warnings,
    ruleMode: ruleMode ?? this.ruleMode,
    busy: busy ?? this.busy,
    status: status ?? this.status,
  );
}

class RoutingController extends Notifier<RoutingState> {
  static int _ruleSeq = 0;

  /// Fresh draft rule id (upstream `RoutingRuleDetailsViewModel` assigns a
  /// GUID when a new rule enters; the draft keeps it stable across edits).
  static String newRuleId() {
    final micros = DateTime.now().microsecondsSinceEpoch.toRadixString(16);
    return 'r$micros-${(_ruleSeq++).toRadixString(16).padLeft(4, '0')}';
  }

  /// Parse imported rule JSON into draft DTOs (file/clipboard/URL bodies).
  ///
  /// Accepts the upstream camelCase `RulesItem` shape and the stored
  /// snake_case shape. Every rule gets a fresh [newRuleId] (upstream
  /// `AddBatchRoutingRules` assigns new GUIDs); rules without a match
  /// criterion are rejected like the engine does. Throws [FormatException]
  /// on invalid input; the caller keeps its draft untouched.
  static List<r.RoutingRuleDto> parseImportedRuleDtos(String text) {
    final Object? decoded = _decodeJson(text);
    if (decoded is! List) {
      throw const FormatException('error.routing_rules_invalid');
    }
    final rules = <r.RoutingRuleDto>[];
    for (final entry in decoded) {
      if (entry is! Map) {
        throw const FormatException('error.routing_rules_invalid');
      }
      final rule = _ruleFromJson(Map<String, Object?>.from(entry));
      final has =
          (rule.port ?? '').trim().isNotEmpty ||
          (rule.network ?? '').trim().isNotEmpty ||
          rule.protocol.isNotEmpty ||
          rule.domain.isNotEmpty ||
          rule.ip.isNotEmpty ||
          rule.process.isNotEmpty ||
          rule.inboundTag.isNotEmpty;
      if (!has) {
        throw const FormatException('error.routing_rule_empty');
      }
      rules.add(rule);
    }
    if (rules.isEmpty) {
      throw const FormatException('error.routing_rules_empty');
    }
    return rules;
  }

  static Object? _decodeJson(String text) {
    return jsonDecode(text);
  }

  static r.RoutingRuleDto _ruleFromJson(Map<String, Object?> json) {
    String? str(String camel, String snake) {
      final value = json[camel] ?? json[snake];
      if (value == null) return null;
      final text = value.toString();
      return text.trim().isEmpty ? null : text;
    }

    List<String> list(String camel, String snake) {
      final value = json[camel] ?? json[snake];
      if (value is List) {
        return value
            .map((e) => e.toString().trim())
            .where((e) => e.isNotEmpty)
            .toList();
      }
      if (value is String && value.trim().isNotEmpty) {
        return value
            .split(RegExp(r'[\r\n,]+'))
            .map((e) => e.trim())
            .where((e) => e.isNotEmpty)
            .toList();
      }
      return const <String>[];
    }

    int? ruleType() {
      final value = json['ruleType'] ?? json['rule_type'];
      if (value is num) return value.toInt();
      return int.tryParse(value?.toString() ?? '');
    }

    final inbound = list('inboundTag', 'inbound_tag');
    final ip = list('ip', 'ip');
    final domain = list('domain', 'domain');
    final protocol = list('protocol', 'protocol');
    final process = list('process', 'process');
    return r.RoutingRuleDto(
      id: newRuleId(),
      ruleKind: str('ruleKind', 'rule_kind') ?? str('type', 'type'),
      port: str('port', 'port'),
      network: str('network', 'network'),
      inboundTag: inbound,
      hasInboundTag: inbound.isNotEmpty,
      outboundTag: str('outboundTag', 'outbound_tag'),
      ip: ip,
      hasIp: ip.isNotEmpty,
      domain: domain,
      hasDomain: domain.isNotEmpty,
      protocol: protocol,
      hasProtocol: protocol.isNotEmpty,
      process: process,
      hasProcess: process.isNotEmpty,
      enabled: json['enabled'] is bool ? json['enabled'] as bool : true,
      remarks: str('remarks', 'remarks'),
      ruleType: ruleType(),
    );
  }

  /// Serialize draft rules to the upstream clipboard shape: indented camelCase
  /// `RulesItem` JSON with ids cleared (upstream `RuleExportSelectedAsync`).
  /// Re-importing assigns fresh ids via [parseImportedRuleDtos].
  static String exportDraftRulesJson(
    List<r.RoutingRuleDto> rules, [
    List<String>? ids,
  ]) {
    final selected = ids == null || ids.isEmpty
        ? rules
        : rules.where((r) => ids.contains(r.id)).toList();
    final maps = selected.map(_ruleToUpstreamJson).toList();
    return const JsonEncoder.withIndent('  ').convert(maps);
  }

  static Map<String, Object?> _ruleToUpstreamJson(r.RoutingRuleDto rule) {
    final map = <String, Object?>{};
    void put(String key, Object? value) {
      if (value == null) return;
      if (value is String && value.trim().isEmpty) return;
      if (value is List && value.isEmpty) return;
      map[key] = value;
    }

    put('type', rule.ruleKind);
    put('port', rule.port);
    put('network', rule.network);
    if (rule.hasInboundTag) map['inboundTag'] = rule.inboundTag;
    put('outboundTag', rule.outboundTag);
    if (rule.hasIp) map['ip'] = rule.ip;
    if (rule.hasDomain) map['domain'] = rule.domain;
    if (rule.hasProtocol) map['protocol'] = rule.protocol;
    if (rule.hasProcess) map['process'] = rule.process;
    map['enabled'] = rule.enabled;
    put('remarks', rule.remarks);
    if (rule.ruleType != null) map['ruleType'] = rule.ruleType;
    return map;
  }

  /// Serialize draft rules into the stored `RuleSet` shape (the domain
  /// `RoutingRule` serde form) so a scheme + its rules persist with one
  /// `save_routing` call. `has*` false maps to null, matching the bridge DTO
  /// conversion; ids are kept stable.
  static String rulesToRuleSetJson(List<r.RoutingRuleDto> rules) {
    final maps = rules.map((rule) {
      Object? optStr(String? value) =>
          value == null || value.trim().isEmpty ? null : value;
      return <String, Object?>{
        'id': rule.id,
        'rule_kind': optStr(rule.ruleKind),
        'port': optStr(rule.port),
        'network': optStr(rule.network),
        'inbound_tag': rule.hasInboundTag ? rule.inboundTag : null,
        'outbound_tag': optStr(rule.outboundTag),
        'ip': rule.hasIp ? rule.ip : null,
        'domain': rule.hasDomain ? rule.domain : null,
        'protocol': rule.hasProtocol ? rule.protocol : null,
        'process': rule.hasProcess ? rule.process : null,
        'enabled': rule.enabled,
        'remarks': optStr(rule.remarks),
        'rule_type': rule.ruleType,
      };
    }).toList();
    return jsonEncode(maps);
  }

  @override
  RoutingState build() {
    final page = ref.read(bridgePortProvider).listRoutings();
    final mode = ref.read(bridgePortProvider).getRuleMode();
    final selectedId = _activeId(page.items);
    return RoutingState(
      items: page.items,
      selectedId: selectedId,
      ruleMode: mode.mode,
      status: page.error?.messageKey,
    );
  }

  r.RoutingsPageDto _page() => ref.read(bridgePortProvider).listRoutings();

  /// Reload the scheme list and the current rule mode.
  void reload() {
    final page = _page();
    final mode = ref.read(bridgePortProvider).getRuleMode();
    var selectedId = state.selectedId;
    if (selectedId != null && !page.items.any((e) => e.id == selectedId)) {
      selectedId = null;
    }
    selectedId ??= _activeId(page.items);
    state = state.copyWith(
      items: page.items,
      selectedId: selectedId,
      ruleMode: mode.mode,
      status: page.error?.messageKey,
    );
    if (selectedId != null) reloadRules(selectedId);
  }

  static String? _activeId(List<r.RoutingProfileDto> items) {
    for (final item in items) {
      if (item.isActive) return item.id;
    }
    return items.isEmpty ? null : items.first.id;
  }

  void select(String? id) {
    state = id == null
        ? state.copyWith(
            clearSelected: true,
            rules: const [],
            warnings: const [],
          )
        : state.copyWith(selectedId: id);
    if (id != null) reloadRules(id);
  }

  /// Reload the parsed rules + warnings of one scheme.
  void reloadRules(String routingId) {
    final page = ref.read(bridgePortProvider).listRoutingRules(routingId);
    if (page.ok) {
      state = state.copyWith(rules: page.rules, warnings: page.warnings);
    } else {
      state = state.copyWith(
        rules: const [],
        warnings: const [],
        status: page.error?.messageKey,
      );
    }
  }

  r.RoutingDtoResult save(r.RoutingProfileDto draft) {
    final result = ref.read(bridgePortProvider).saveRouting(draft);
    if (result.ok) {
      reload();
      state = state.copyWith(status: '路由方案已保存（未应用，点“应用”生效）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '保存失败');
    }
    return result;
  }

  c.SimpleResult delete(String id) {
    final result = ref.read(bridgePortProvider).deleteRouting(id);
    if (result.ok) {
      state = state.copyWith(clearSelected: true);
      reload();
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '删除失败');
    }
    return result;
  }

  c.SimpleResult setDefault(String id) {
    final result = ref.read(bridgePortProvider).setDefaultRouting(id);
    if (result.ok) {
      reload();
      state = state.copyWith(status: '已切换默认路由（未应用，点“应用”生效）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '切换失败');
    }
    return result;
  }

  r.RoutingDtoResult moveRule(String routingId, int index, int direction) {
    final result = ref
        .read(bridgePortProvider)
        .moveRoutingRule(routingId, index, direction);
    if (result.ok) {
      reloadRules(routingId);
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '移动失败');
    }
    return result;
  }

  r.RoutingDtoResult saveRules(String routingId, List<r.RoutingRuleDto> rules) {
    final result = ref
        .read(bridgePortProvider)
        .saveRoutingRules(routingId, rules);
    if (result.ok) {
      reloadRules(routingId);
      state = state.copyWith(status: '规则已保存（未应用，点“应用”生效）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '保存失败');
    }
    return result;
  }

  r.RoutingRulesTextResult importRules(
    String routingId,
    String text, {
    bool replace = false,
  }) {
    final result = ref
        .read(bridgePortProvider)
        .importRoutingRules(routingId, text, replace);
    if (result.ok) {
      reload();
      state = state.copyWith(status: '已导入 ${result.ruleCount} 条规则');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '导入失败');
    }
    return result;
  }

  r.RoutingRulesTextResult exportRules(String routingId, List<String> ids) =>
      ref.read(bridgePortProvider).exportRoutingRules(routingId, ids);

  c.SimpleResult setRuleMode(String mode) {
    final result = ref.read(bridgePortProvider).setRuleMode(mode);
    if (result.ok) {
      state = state.copyWith(ruleMode: mode, status: '路由模式：$mode（未应用，点“应用”生效）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '切换失败');
    }
    return result;
  }

  /// Fresh scheme draft (remarks required on save).
  r.RoutingProfileDto newDraft() => const r.RoutingProfileDto(
    id: '',
    remarks: '',
    url: '',
    ruleSet: '[]',
    ruleNum: 0,
    enabled: true,
    locked: false,
    customIcon: '',
    customRulesetPath4Singbox: '',
    domainStrategy: '',
    domainStrategy4Singbox: '',
    sort: 0,
    isActive: false,
  );

  /// Fresh rule draft (upstream defaults: `proxy` outbound, enabled).
  /// The draft carries a stable unique id from entry, so selecting/editing
  /// one new rule can never hit another (SET-07).
  r.RoutingRuleDto newRuleDraft() => r.RoutingRuleDto(
    id: newRuleId(),
    inboundTag: [],
    hasInboundTag: false,
    outboundTag: 'proxy',
    ip: [],
    hasIp: false,
    domain: [],
    hasDomain: false,
    protocol: [],
    hasProtocol: false,
    process: [],
    hasProcess: false,
    enabled: true,
    ruleType: 1,
  );
}
