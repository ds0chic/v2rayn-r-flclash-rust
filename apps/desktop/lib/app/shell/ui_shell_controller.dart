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
    this.accentName,
    this.fontFamily,
    this.fontSize,
    this.language,
    this.hideIpInfo = false,
    this.showStatistics = false,
    this.autoAdjustColWidth = false,
    this.zebraStriping = false,
    this.trayMenuServersLimit = 20,
    this.systemProxyIndex = 2,
    this.routingLabel,
    this.tunEnabled = false,
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

  /// `UIItem.ColorPrimaryName` (Material swatch name), applied immediately.
  final String? accentName;
  final String? fontFamily;
  final double? fontSize;
  final String? language;
  final bool hideIpInfo;
  final bool showStatistics;
  final bool autoAdjustColWidth;

  /// Optional table zebra striping (LAY-PROFILES-002 density kept intact).
  final bool zebraStriping;
  final int trayMenuServersLimit;

  final int systemProxyIndex;
  final String? routingLabel;
  final bool tunEnabled;
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
    String? accentName,
    bool clearAccent = false,
    String? fontFamily,
    bool clearFontFamily = false,
    double? fontSize,
    bool clearFontSize = false,
    String? language,
    bool? hideIpInfo,
    bool? showStatistics,
    bool? autoAdjustColWidth,
    bool? zebraStriping,
    int? trayMenuServersLimit,
    int? systemProxyIndex,
    String? routingLabel,
    bool? tunEnabled,
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
      accentName: clearAccent ? null : (accentName ?? this.accentName),
      fontFamily: clearFontFamily ? null : (fontFamily ?? this.fontFamily),
      fontSize: clearFontSize ? null : (fontSize ?? this.fontSize),
      language: language ?? this.language,
      hideIpInfo: hideIpInfo ?? this.hideIpInfo,
      showStatistics: showStatistics ?? this.showStatistics,
      autoAdjustColWidth: autoAdjustColWidth ?? this.autoAdjustColWidth,
      zebraStriping: zebraStriping ?? this.zebraStriping,
      trayMenuServersLimit: trayMenuServersLimit ?? this.trayMenuServersLimit,
      systemProxyIndex: systemProxyIndex ?? this.systemProxyIndex,
      routingLabel: routingLabel ?? this.routingLabel,
      tunEnabled: tunEnabled ?? this.tunEnabled,
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
      themeMode: _themeModeFrom(theme['mode'] as String?),
      accentName: theme['accent'] as String?,
      zebraStriping: theme['zebra'] == true,
      fontFamily: theme['font_family'] as String?,
      fontSize: (theme['font_size'] as num?)?.toDouble(),
      language: theme['language'] as String?,
      systemProxyIndex: (status['system_proxy'] as num?)?.toInt() ?? 2,
    );
  }

  static ThemeMode _themeModeFrom(String? name) {
    switch (name) {
      case 'dark':
      case 'Dark':
        return ThemeMode.dark;
      case 'light':
      case 'Light':
        return ThemeMode.light;
      default:
        return ThemeMode.system;
    }
  }

  /// Apply the persisted settings document to the live UI.
  ///
  /// Handles the `immediate` / UI-layer fields from `compat/fields.settings.yaml`
  /// (theme, accent, font, language, layout, hide-IP, statistics). Fields that
  /// need a kernel or app restart are left to T13 and only surfaced as a note.
  void applySettingsDocument(Map<String, dynamic> document) {
    final ui = document['UiItem'] as Map<String, dynamic>? ?? const {};
    final gui = document['GuiItem'] as Map<String, dynamic>? ?? const {};
    final orientation = (ui['MainGirdOrientation'] as num?)?.toInt();
    final layout = switch (orientation) {
      0 => AppLayoutMode.horizontal,
      2 => AppLayoutMode.tab,
      _ => AppLayoutMode.vertical,
    };
    final fontSize = (ui['CurrentFontSize'] as num?)?.toDouble();
    final accentRaw = ui['ColorPrimaryName'] as String?;
    final familyRaw = ui['CurrentFontFamily'] as String?;
    state = state.copyWith(
      layout: layout,
      themeMode: _themeModeFrom(ui['CurrentTheme'] as String?),
      accentName: (accentRaw == null || accentRaw.isEmpty) ? null : accentRaw,
      clearAccent: accentRaw == null || accentRaw.isEmpty,
      fontFamily: (familyRaw == null || familyRaw.isEmpty) ? null : familyRaw,
      clearFontFamily: familyRaw == null || familyRaw.isEmpty,
      fontSize: (fontSize != null && fontSize >= 8) ? fontSize : null,
      clearFontSize: fontSize == null || fontSize < 8,
      language: ui['CurrentLanguage'] as String?,
      hideIpInfo: ui['HideColumnIpInfo'] == true,
      showStatistics: gui['EnableStatistics'] == true,
      autoAdjustColWidth: ui['EnableAutoAdjustMainLvColWidth'] == true,
      trayMenuServersLimit:
          (gui['TrayMenuServersLimit'] as num?)?.toInt() ??
          state.trayMenuServersLimit,
    );
    _persistTheme();
  }

  /// Apply a theme-dialog selection immediately (before/as it is persisted).
  void applyThemeSelection({
    String? theme,
    String? accent,
    String? fontFamily,
    int? fontSize,
    String? language,
  }) {
    state = state.copyWith(
      themeMode: theme == null ? null : _themeModeFrom(theme),
      accentName: accent,
      clearAccent: accent != null && accent.isEmpty,
      fontFamily: fontFamily,
      fontSize: fontSize?.toDouble(),
      language: language,
    );
    _persistTheme();
  }

  void _persistTheme() {
    _store.saveSection(themeSection, <String, dynamic>{
      'mode': switch (state.themeMode) {
        ThemeMode.dark => 'dark',
        ThemeMode.light => 'light',
        ThemeMode.system => 'system',
      },
      'accent': state.accentName,
      'font_family': state.fontFamily,
      'font_size': state.fontSize,
      'language': state.language,
      'zebra': state.zebraStriping,
    });
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
    _persistTheme();
  }

  /// Explicit theme mode (used by the theme dialog and evidence hooks so a
  /// `system` value is not mistaken for a resolved light/dark).
  void setThemeMode(ThemeMode mode) {
    state = state.copyWith(themeMode: mode);
    _persistTheme();
  }

  void toggleZebraStriping() {
    state = state.copyWith(zebraStriping: !state.zebraStriping);
    _persistTheme();
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
