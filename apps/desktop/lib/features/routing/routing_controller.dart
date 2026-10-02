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
  r.RoutingRuleDto newRuleDraft() => const r.RoutingRuleDto(
    id: '',
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
