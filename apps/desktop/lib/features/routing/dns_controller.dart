import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

final dnsControllerProvider = NotifierProvider<DnsController, DnsState>(
  DnsController.new,
);

/// DNS settings window state (upstream `DNSSettingViewModel`, F-DNS-001/002).
class DnsState {
  const DnsState({
    this.items = const <d.DnsProfileDto>[],
    this.simple,
    this.revision = 0,
    this.preset = 'Default',
    this.pendingUrls = const <String>[],
    this.busy = false,
    this.status,
    this.loadFailed = false,
  });

  final List<d.DnsProfileDto> items;
  final d.SimpleDnsDto? simple;
  final int revision;
  final String preset;
  final List<String> pendingUrls;
  final bool busy;
  final String? status;

  /// SP-13/CP-08: true when the last reload read no usable baseline (both
  /// reads failed on an empty cache). The window must then refuse to save:
  /// persisting the empty draft would wipe stored DNS rows.
  final bool loadFailed;

  d.DnsProfileDto? forCore(CoreType core) {
    for (final item in items) {
      if (item.coreType == core) return item;
    }
    return null;
  }

  DnsState copyWith({
    List<d.DnsProfileDto>? items,
    d.SimpleDnsDto? simple,
    int? revision,
    String? preset,
    List<String>? pendingUrls,
    bool? busy,
    String? status,
    bool? loadFailed,
  }) => DnsState(
    items: items ?? this.items,
    simple: simple ?? this.simple,
    revision: revision ?? this.revision,
    preset: preset ?? this.preset,
    pendingUrls: pendingUrls ?? this.pendingUrls,
    busy: busy ?? this.busy,
    status: status ?? this.status,
    loadFailed: loadFailed ?? this.loadFailed,
  );
}

class DnsController extends Notifier<DnsState> {
  @override
  DnsState build() => const DnsState();

  /// SP-13: a read counts as failed only when no usable baseline exists.
  /// A partial failure over a cached baseline keeps the baseline editable;
  /// a total failure on an empty cache must block saving the empty draft.
  static bool loadFailedFor({
    required bool listOk,
    required bool simpleOk,
    required bool hasItems,
    required bool hasSimple,
  }) => (!listOk || !simpleOk) && !hasItems && !hasSimple;

  void reload() {
    final page = ref.read(bridgePortProvider).listDns();
    final simple = ref.read(bridgePortProvider).loadSimpleDns();
    final effectiveSimple = simple.item ?? state.simple;
    state = state.copyWith(
      items: page.items,
      simple: effectiveSimple,
      revision: simple.revision.toInt(),
      status: page.error?.messageKey ?? simple.error?.messageKey,
      loadFailed: loadFailedFor(
        listOk: page.error == null,
        simpleOk: simple.error == null,
        // A partial failure over a cached baseline keeps it editable.
        hasItems: page.items.isNotEmpty || state.items.isNotEmpty,
        hasSimple: effectiveSimple != null,
      ),
    );
  }

  d.DnsDtoResult save(d.DnsProfileDto draft) {
    final result = ref.read(bridgePortProvider).saveDns(draft);
    if (result.ok) {
      reload();
      state = state.copyWith(status: 'DNS 配置已保存（保存成功即按原版重载）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '保存失败');
    }
    return result;
  }

  d.DnsDtoResult importDefault(CoreType core) {
    final result = ref.read(bridgePortProvider).importDefaultDns(core);
    if (result.ok) {
      reload();
      state = state.copyWith(status: '已导入内置默认 DNS 配置（离线模板）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '导入失败');
    }
    return result;
  }

  d.SimpleDnsDtoResult saveSimple(d.SimpleDnsDto draft) {
    final result = ref
        .read(bridgePortProvider)
        .saveSimpleDns(draft, state.revision);
    if (result.ok) {
      reload();
      state = state.copyWith(status: 'DNS 设置已保存（保存成功即按原版重载）');
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '保存失败');
    }
    return result;
  }

  d.RegionalPresetResult applyPreset(String preset) {
    final result = ref.read(bridgePortProvider).applyRegionalPreset(preset);
    if (result.ok) {
      reload();
      state = state.copyWith(
        preset: result.preset,
        pendingUrls: result.pendingUrls,
        status: result.pendingUrls.isEmpty
            ? '区域预设已应用：${result.preset}'
            : '区域预设已应用（离线）：${result.preset}，${result.pendingUrls.length} 个远程模板待下载',
      );
    } else {
      state = state.copyWith(status: result.error?.messageKey ?? '应用失败');
    }
    return result;
  }

  String defaultText(String kind) =>
      ref.read(bridgePortProvider).defaultDnsText(kind);

  /// Field-level validation mirroring the Rust side (empty = fallback).
  c.ErrorDto? validateSimple(d.SimpleDnsDto draft) {
    // Direct/remote/bootstrap are free-form address lists; only the custom
    // DNS JSON texts are strictly validated by the backend on save.
    return null;
  }
}
