import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

/// Main-window grid orientation, mirroring upstream `EGirdOrientation`
/// (compat/layouts.yaml LAY-MAIN-001/002/003).
enum AppLayoutMode {
  horizontal('horizontal', '水平布局', Icons.view_column_outlined),
  vertical('vertical', '垂直布局', Icons.view_agenda_outlined),
  tab('tab', '标签布局', Icons.tab_outlined);

  const AppLayoutMode(this.id, this.label, this.icon);

  final String id;
  final String label;
  final IconData icon;

  static AppLayoutMode fromId(String? id) {
    return AppLayoutMode.values.firstWhere(
      (m) => m.id == id,
      orElse: () => AppLayoutMode.vertical,
    );
  }
}

/// The four system-proxy semantics from LAY-STATUSBAR-001 /
/// outputs plan §09 (ForcedClear / ForcedChange / Unchanged / Pac).
/// T05 only switches UI state; the real WinINET work lands in T13.
class SystemProxyMode {
  const SystemProxyMode(this.label);

  final String label;

  static const modes = <SystemProxyMode>[
    SystemProxyMode('清除系统代理'),
    SystemProxyMode('自动配置系统代理'),
    SystemProxyMode('不改变系统代理'),
    SystemProxyMode('Pac 模式'),
  ];
}

class TrafficSpeed {
  const TrafficSpeed({this.up = '--', this.down = '--'});

  final String up;
  final String down;

  bool get hasData => up != '--' || down != '--';
}

/// Single UI-state source of truth for the shell. Values that require a live
/// backend (kernel, subscription, Clash API) stay at their "unknown" defaults
/// and are rendered as `--` / `未运行`; nothing is fabricated.
class UiShellState {
  const UiShellState({
    this.layout = AppLayoutMode.vertical,
    this.horizontalSplit = 0.5,
    this.verticalSplit = 0.5,
    this.tabIndex = 0,
    this.themeMode = ThemeMode.light,
    this.systemProxyIndex = 2,
    this.routingLabel,
    this.tunEnabled = false,
    this.runningNode,
    this.proxySpeed = const TrafficSpeed(),
    this.directSpeed = const TrafficSpeed(),
    this.inbound,
    this.inboundLan,
    this.message,
  });

  final AppLayoutMode layout;

  /// Fraction of the horizontal layout owned by the left (profiles) panel.
  final double horizontalSplit;

  /// Fraction of the vertical layout owned by the top (profiles) panel.
  final double verticalSplit;

  final int tabIndex;
  final ThemeMode themeMode;
  final int systemProxyIndex;
  final String? routingLabel;
  final bool tunEnabled;
  final String? runningNode;
  final TrafficSpeed proxySpeed;
  final TrafficSpeed directSpeed;
  final String? inbound;
  final String? inboundLan;

  /// Last transient UI feedback, shown in the status bar. Used for
  /// "尚未实现" notices so the shell never pretends a backend action ran.
  final String? message;

  UiShellState copyWith({
    AppLayoutMode? layout,
    double? horizontalSplit,
    double? verticalSplit,
    int? tabIndex,
    ThemeMode? themeMode,
    int? systemProxyIndex,
    String? routingLabel,
    bool? tunEnabled,
    String? runningNode,
    TrafficSpeed? proxySpeed,
    TrafficSpeed? directSpeed,
    String? inbound,
    String? inboundLan,
    String? message,
    bool clearMessage = false,
  }) {
    return UiShellState(
      layout: layout ?? this.layout,
      horizontalSplit: horizontalSplit ?? this.horizontalSplit,
      verticalSplit: verticalSplit ?? this.verticalSplit,
      tabIndex: tabIndex ?? this.tabIndex,
      themeMode: themeMode ?? this.themeMode,
      systemProxyIndex: systemProxyIndex ?? this.systemProxyIndex,
      routingLabel: routingLabel ?? this.routingLabel,
      tunEnabled: tunEnabled ?? this.tunEnabled,
      runningNode: runningNode ?? this.runningNode,
      proxySpeed: proxySpeed ?? this.proxySpeed,
      directSpeed: directSpeed ?? this.directSpeed,
      inbound: inbound ?? this.inbound,
      inboundLan: inboundLan ?? this.inboundLan,
      message: clearMessage ? null : (message ?? this.message),
    );
  }
}

final uiShellControllerProvider =
    NotifierProvider<UiShellController, UiShellState>(UiShellController.new);

class UiShellController extends Notifier<UiShellState> {
  static const layoutSection = 'layout';
  static const themeSection = 'theme';
  static const statusSection = 'status_ui';

  UiStateStore get _store => ref.read(uiStateStoreProvider);

  @override
  UiShellState build() {
    final layout = _store.loadSection(layoutSection) ?? const {};
    final theme = _store.loadSection(themeSection) ?? const {};
    final status = _store.loadSection(statusSection) ?? const {};
    return UiShellState(
      layout: AppLayoutMode.fromId(layout['mode'] as String?),
      horizontalSplit: (layout['horizontal_split'] as num?)?.toDouble() ?? 0.5,
      verticalSplit: (layout['vertical_split'] as num?)?.toDouble() ?? 0.5,
      themeMode: (theme['mode'] as String?) == 'dark'
          ? ThemeMode.dark
          : ThemeMode.light,
      systemProxyIndex: (status['system_proxy'] as num?)?.toInt() ?? 2,
    );
  }

  void setLayout(AppLayoutMode mode) {
    state = state.copyWith(layout: mode);
    _persistLayout();
  }

  void setHorizontalSplit(double fraction) {
    state = state.copyWith(horizontalSplit: fraction.clamp(0.1, 0.9));
    _persistLayout();
  }

  void nudgeHorizontalSplit(double delta) =>
      setHorizontalSplit(state.horizontalSplit + delta);

  void setVerticalSplit(double fraction) {
    state = state.copyWith(verticalSplit: fraction.clamp(0.1, 0.9));
    _persistLayout();
  }

  void nudgeVerticalSplit(double delta) =>
      setVerticalSplit(state.verticalSplit + delta);

  void setTabIndex(int index) => state = state.copyWith(tabIndex: index);

  void toggleTheme() {
    final next = state.themeMode == ThemeMode.light
        ? ThemeMode.dark
        : ThemeMode.light;
    state = state.copyWith(themeMode: next);
    _store.saveSection(themeSection, <String, dynamic>{
      'mode': next == ThemeMode.dark ? 'dark' : 'light',
    });
  }

  void setSystemProxyIndex(int index) {
    state = state.copyWith(systemProxyIndex: index);
    _store.saveSection(statusSection, <String, dynamic>{'system_proxy': index});
    setMessage('系统代理仅切换 UI 状态，后端接入见 T13');
  }

  void setTunEnabled(bool value) {
    state = state.copyWith(tunEnabled: value);
    setMessage('TUN 模式后端尚未接入');
  }

  void setMessage(String? message) {
    state = state.copyWith(message: message, clearMessage: message == null);
  }

  /// Menu/context entries that still need the Rust backend. They must never
  /// report success. `actionId` is the ACT-* ledger id.
  void notImplemented(String label, String? actionId) {
    final suffix = actionId == null ? '' : ' ($actionId)';
    setMessage('尚未实现: $label$suffix');
  }

  void _persistLayout() {
    _store.saveSection(layoutSection, <String, dynamic>{
      'mode': state.layout.id,
      'horizontal_split': state.horizontalSplit,
      'vertical_split': state.verticalSplit,
    });
  }
}
