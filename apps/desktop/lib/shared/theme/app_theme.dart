import 'package:flutter/material.dart';

/// Window metrics from compat/layouts.yaml (LAY-MAIN-002 / INV-WPF-002).
///
/// Upstream title is `v2rayN`; default window 1200x800 with min width 800.
/// The Win32 runner persists the size, so only the title and minimum size are
/// enforced at runtime by `DesktopIntegration`.
class AppWindowMetrics {
  const AppWindowMetrics._();

  static const String title = 'v2rayN';
  static const Size defaultSize = Size(1200, 800);
  static const double minWidth = 800;
  static const double minHeight = 600;
}

/// Central design tokens for the desktop shell.
///
/// T05 keeps a single component system (Material 3) and only re-skins it with
/// the compact desktop density called for by the plan (see
/// outputs/V2RAYN_FLUTTER_RUST_PLAN.md §7). No second component library is
/// introduced, so focus, menus and font metrics stay consistent.
///
/// T17 adds the semantic layer (success/warning/hover/selected/disabled/zebra),
/// a typographic scale, the configurable compact row heights and the shared
/// icon family. Information density anchors come from compat/layouts.yaml:
///   - LAY-MAIN-002 default window 1200x800
///   - LAY-PROFILES-002 table row height ~26 logical px
class AppTokens {
  const AppTokens._();

  // -- Typography scale --------------------------------------------------
  static const double fontSizeTitle = 14;
  static const double fontSizeSubtitle = 13;
  static const double fontSize = 12.5;
  static const double fontSizeSmall = 11.5;
  static const double fontSizeTiny = 11;
  static const double monoFontSize = 12;
  static const String monoFontFamily = 'monospace';

  // -- Compact desktop chrome heights ------------------------------------
  static const double menuBarHeight = 30;
  static const double toolbarHeight = 32;
  static const double statusBarHeight = 30;
  static const double tableHeaderHeight = 30;
  static const double tableHandleWidth = 48;
  static const double splitterThickness = 6;

  /// Selectable compact row heights (logical px); the plan forbids turning the
  /// desktop table into touch-sized rows, so the range stays 24..28.
  static const List<double> rowHeightOptions = <double>[24, 26, 28];
  static const double tableRowHeight = 26;

  /// Row height derived from the configured base font size. Keeps the table
  /// compact at every supported size while honouring `UIItem.CurrentFontSize`.
  static double rowHeightFor(double? baseFontSize) {
    if (baseFontSize == null) return tableRowHeight;
    if (baseFontSize <= fontSizeSmall) return rowHeightOptions.first;
    if (baseFontSize <= fontSize) return tableRowHeight;
    return rowHeightOptions.last;
  }

  // -- Icon sizes --------------------------------------------------------
  static const double iconSizeSmall = 14;
  static const double iconSize = 16;
  static const double iconSizeToolbar = 18;

  // -- Neutrals (light) --------------------------------------------------
  static const Color lightSurface = Color(0xFFF3F4F6);
  static const Color lightSurfaceContainer = Color(0xFFEAECEF);
  static const Color lightSurfaceHighest = Color(0xFFE1E4E8);
  static const Color lightOutline = Color(0xFFB9BEC6);
  static const Color lightGrid = Color(0x33000000);
  static const Color lightHover = Color(0x14000000);
  static const Color lightZebra = Color(0x08000000);
  static const Color lightDisabled = Color(0x61000000);

  // -- Neutrals (dark) ---------------------------------------------------
  static const Color darkSurface = Color(0xFF1E2126);
  static const Color darkSurfaceContainer = Color(0xFF262A31);
  static const Color darkSurfaceHighest = Color(0xFF2E333B);
  static const Color darkOutline = Color(0xFF4A505A);
  static const Color darkGrid = Color(0x33FFFFFF);
  static const Color darkHover = Color(0x1FFFFFFF);
  static const Color darkZebra = Color(0x0AFFFFFF);
  static const Color darkDisabled = Color(0x61FFFFFF);

  // -- Semantic status colors -------------------------------------------
  static const Color lightSuccess = Color(0xFF2E7D32);
  static const Color darkSuccess = Color(0xFF81C784);
  static const Color lightWarning = Color(0xFFED6C02);
  static const Color darkWarning = Color(0xFFFFB74D);
  static const Color lightInfo = Color(0xFF0277BD);
  static const Color darkInfo = Color(0xFF4FC3F7);

  /// Upstream glyph map: one family for toolbar/menu/status bar so a semantic
  /// name always resolves to the same outlined Material icon.
  static const Map<String, IconData> iconBySemantic = <String, IconData>{
    'add': Icons.add,
    'edit': Icons.edit_outlined,
    'delete': Icons.delete_outline,
    'copy': Icons.copy_outlined,
    'remarks': Icons.drive_file_rename_outline,
    'activate': Icons.check_circle_outline,
    'test': Icons.speed_outlined,
    'refresh': Icons.refresh,
    'reload': Icons.restart_alt,
    'settings': Icons.settings_outlined,
    'routing': Icons.alt_route_outlined,
    'dns': Icons.dns_outlined,
    'subscribe': Icons.rss_feed,
    'backup': Icons.backup_outlined,
    'update': Icons.system_update_alt_outlined,
    'help': Icons.help_outline,
    'promotion': Icons.campaign_outlined,
    'close': Icons.minimize,
    'exit': Icons.power_settings_new_outlined,
    'logs': Icons.message_outlined,
    'proxies': Icons.call_split_outlined,
    'connections': Icons.lan_outlined,
    'layout-horizontal': Icons.view_column_outlined,
    'layout-vertical': Icons.view_agenda_outlined,
    'layout-tab': Icons.tab_outlined,
    'theme': Icons.brightness_6_outlined,
    'columns': Icons.view_column_outlined,
    'filter': Icons.filter_alt_outlined,
    'search': Icons.search,
    'warning': Icons.warning_amber_outlined,
    'error': Icons.error_outline,
    'success': Icons.check_circle_outline,
    'info': Icons.info_outline,
    'empty': Icons.inbox_outlined,
    'running': Icons.play_circle_outline,
    'stopped': Icons.stop_circle_outlined,
  };

  static IconData icon(String semantic) =>
      iconBySemantic[semantic] ?? Icons.circle_outlined;
}

/// Theme-extension carrying the semantic colors and density values so widgets
/// do not hard-code brightness-specific constants.
@immutable
class AppSemanticColors extends ThemeExtension<AppSemanticColors> {
  const AppSemanticColors({
    required this.success,
    required this.warning,
    required this.info,
    required this.hover,
    required this.selectedRow,
    required this.disabledForeground,
    required this.zebraStripe,
    required this.focusOutline,
    required this.gridLine,
    required this.monoFontFamily,
    required this.tableRowHeight,
    required this.zebraEnabled,
  });

  final Color success;
  final Color warning;
  final Color info;
  final Color hover;
  final Color selectedRow;
  final Color disabledForeground;
  final Color zebraStripe;
  final Color focusOutline;
  final Color gridLine;
  final String monoFontFamily;
  final double tableRowHeight;
  final bool zebraEnabled;

  static AppSemanticColors forScheme(
    ColorScheme scheme, {
    required double tableRowHeight,
    bool zebraEnabled = false,
  }) {
    final isLight = scheme.brightness == Brightness.light;
    return AppSemanticColors(
      success: isLight ? AppTokens.lightSuccess : AppTokens.darkSuccess,
      warning: isLight ? AppTokens.lightWarning : AppTokens.darkWarning,
      info: isLight ? AppTokens.lightInfo : AppTokens.darkInfo,
      hover: isLight ? AppTokens.lightHover : AppTokens.darkHover,
      selectedRow: scheme.primaryContainer,
      disabledForeground: isLight
          ? AppTokens.lightDisabled
          : AppTokens.darkDisabled,
      zebraStripe: isLight ? AppTokens.lightZebra : AppTokens.darkZebra,
      focusOutline: scheme.primary,
      gridLine: isLight ? AppTokens.lightGrid : AppTokens.darkGrid,
      monoFontFamily: AppTokens.monoFontFamily,
      tableRowHeight: tableRowHeight,
      zebraEnabled: zebraEnabled,
    );
  }

  @override
  AppSemanticColors copyWith({
    Color? success,
    Color? warning,
    Color? info,
    Color? hover,
    Color? selectedRow,
    Color? disabledForeground,
    Color? zebraStripe,
    Color? focusOutline,
    Color? gridLine,
    String? monoFontFamily,
    double? tableRowHeight,
    bool? zebraEnabled,
  }) {
    return AppSemanticColors(
      success: success ?? this.success,
      warning: warning ?? this.warning,
      info: info ?? this.info,
      hover: hover ?? this.hover,
      selectedRow: selectedRow ?? this.selectedRow,
      disabledForeground: disabledForeground ?? this.disabledForeground,
      zebraStripe: zebraStripe ?? this.zebraStripe,
      focusOutline: focusOutline ?? this.focusOutline,
      gridLine: gridLine ?? this.gridLine,
      monoFontFamily: monoFontFamily ?? this.monoFontFamily,
      tableRowHeight: tableRowHeight ?? this.tableRowHeight,
      zebraEnabled: zebraEnabled ?? this.zebraEnabled,
    );
  }

  @override
  AppSemanticColors lerp(ThemeExtension<AppSemanticColors>? other, double t) {
    if (other is! AppSemanticColors) return this;
    return AppSemanticColors(
      success: Color.lerp(success, other.success, t)!,
      warning: Color.lerp(warning, other.warning, t)!,
      info: Color.lerp(info, other.info, t)!,
      hover: Color.lerp(hover, other.hover, t)!,
      selectedRow: Color.lerp(selectedRow, other.selectedRow, t)!,
      disabledForeground: Color.lerp(
        disabledForeground,
        other.disabledForeground,
        t,
      )!,
      zebraStripe: Color.lerp(zebraStripe, other.zebraStripe, t)!,
      focusOutline: Color.lerp(focusOutline, other.focusOutline, t)!,
      gridLine: Color.lerp(gridLine, other.gridLine, t)!,
      monoFontFamily: other.monoFontFamily,
      tableRowHeight: other.tableRowHeight,
      zebraEnabled: other.zebraEnabled,
    );
  }
}

/// Convenience accessor with a brightness-correct fallback so widgets still
/// render under a bare `MaterialApp` (the widget tests pump one).
extension AppSemanticContext on BuildContext {
  AppSemanticColors get semantics {
    final theme = Theme.of(this);
    return theme.extension<AppSemanticColors>() ??
        AppSemanticColors.forScheme(
          theme.colorScheme,
          tableRowHeight: AppTokens.tableRowHeight,
        );
  }
}

/// Upstream `UIItem.ColorPrimaryName` values are Material swatch names. Map
/// them onto concrete seeds; unknown names fall back to the default blue.
const Map<String, Color> accentColors = <String, Color>{
  'Red': Color(0xFFF44336),
  'Pink': Color(0xFFE91E63),
  'Purple': Color(0xFF9C27B0),
  'DeepPurple': Color(0xFF673AB7),
  'Indigo': Color(0xFF3F51B5),
  'Blue': Color(0xFF2196F3),
  'LightBlue': Color(0xFF03A9F4),
  'Cyan': Color(0xFF00BCD4),
  'Teal': Color(0xFF009688),
  'Green': Color(0xFF4CAF50),
  'LightGreen': Color(0xFF8BC34A),
  'Lime': Color(0xFFCDDC39),
  'Yellow': Color(0xFFFFEB3B),
  'Amber': Color(0xFFFFC107),
  'Orange': Color(0xFFFF9800),
  'DeepOrange': Color(0xFFFF5722),
  'Brown': Color(0xFF795548),
  'Grey': Color(0xFF9E9E9E),
  'BlueGrey': Color(0xFF607D8B),
};

const Color defaultAccent = Color(0xFF1565C0);

Color accentColorFor(String? name) => accentColors[name] ?? defaultAccent;

ThemeData buildAppTheme(
  Brightness brightness, {
  String? accentName,
  String? fontFamily,
  double? fontSize,
  bool zebraEnabled = false,
}) {
  final isLight = brightness == Brightness.light;
  final seed = accentColorFor(accentName);
  final scheme = ColorScheme.fromSeed(seedColor: seed, brightness: brightness)
      .copyWith(
        surface: isLight ? AppTokens.lightSurface : AppTokens.darkSurface,
        surfaceContainer: isLight
            ? AppTokens.lightSurfaceContainer
            : AppTokens.darkSurfaceContainer,
        surfaceContainerHighest: isLight
            ? AppTokens.lightSurfaceHighest
            : AppTokens.darkSurfaceHighest,
        outline: isLight ? AppTokens.lightOutline : AppTokens.darkOutline,
      );

  final baseSize = (fontSize != null && fontSize >= 8)
      ? fontSize
      : AppTokens.fontSize;
  final family = (fontFamily != null && fontFamily.isNotEmpty)
      ? fontFamily
      : null;
  final rowHeight = AppTokens.rowHeightFor(
    (fontSize != null && fontSize >= 8) ? fontSize : null,
  );
  final semantics = AppSemanticColors.forScheme(
    scheme,
    tableRowHeight: rowHeight,
    zebraEnabled: zebraEnabled,
  );
  final hover = isLight ? AppTokens.lightHover : AppTokens.darkHover;

  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    visualDensity: VisualDensity.compact,
    fontFamily: family,
    extensions: <ThemeExtension<dynamic>>[semantics],
    dividerTheme: DividerThemeData(
      space: 1,
      thickness: 1,
      color: scheme.outlineVariant,
    ),
    splashFactory: InkRipple.splashFactory,
    focusColor: scheme.primary.withValues(alpha: 0.12),
    hoverColor: hover,
    iconTheme: IconThemeData(size: AppTokens.iconSize),
    menuBarTheme: MenuBarThemeData(
      style: MenuStyle(
        backgroundColor: WidgetStatePropertyAll(scheme.surfaceContainer),
        maximumSize: const WidgetStatePropertyAll(Size.infinite),
        minimumSize: const WidgetStatePropertyAll(
          Size(0, AppTokens.menuBarHeight),
        ),
        padding: const WidgetStatePropertyAll(EdgeInsets.zero),
        shape: const WidgetStatePropertyAll(
          RoundedRectangleBorder(borderRadius: BorderRadius.zero),
        ),
      ),
    ),
    menuTheme: MenuThemeData(
      style: MenuStyle(
        backgroundColor: WidgetStatePropertyAll(scheme.surface),
        shape: const WidgetStatePropertyAll(
          RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(4)),
          ),
        ),
        visualDensity: VisualDensity.compact,
        elevation: const WidgetStatePropertyAll(4),
      ),
    ),
    popupMenuTheme: PopupMenuThemeData(
      color: scheme.surface,
      surfaceTintColor: Colors.transparent,
      elevation: 4,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.all(Radius.circular(4)),
      ),
      textStyle: TextStyle(fontSize: AppTokens.fontSize),
    ),
    tooltipTheme: TooltipThemeData(
      waitDuration: const Duration(milliseconds: 400),
      textStyle: TextStyle(fontSize: AppTokens.fontSizeSmall),
      decoration: BoxDecoration(
        color: isLight ? const Color(0xE6303030) : const Color(0xF0E6E6E6),
        borderRadius: const BorderRadius.all(Radius.circular(3)),
      ),
    ),
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      width: 460,
      elevation: 4,
      backgroundColor: isLight
          ? const Color(0xFF303030)
          : const Color(0xFFE6E6E6),
      contentTextStyle: TextStyle(
        fontSize: AppTokens.fontSize,
        color: isLight ? Colors.white : Colors.black87,
      ),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thickness: const WidgetStatePropertyAll(8),
      radius: const Radius.circular(4),
      thumbColor: WidgetStatePropertyAll(scheme.outline.withValues(alpha: 0.6)),
      thumbVisibility: const WidgetStatePropertyAll(false),
    ),
    inputDecorationTheme: InputDecorationTheme(
      isDense: true,
      contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      hintStyle: TextStyle(
        fontSize: AppTokens.fontSizeSmall,
        color: scheme.onSurfaceVariant,
      ),
      border: const OutlineInputBorder(
        borderRadius: BorderRadius.all(Radius.circular(3)),
      ),
    ),
    dialogTheme: DialogThemeData(
      // Explicit so dialogs always track the active light/dark token set.
      backgroundColor: scheme.surfaceContainerHigh,
      surfaceTintColor: Colors.transparent,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.all(Radius.circular(6)),
      ),
      titleTextStyle: TextStyle(
        fontSize: AppTokens.fontSizeTitle,
        fontWeight: FontWeight.w600,
        color: scheme.onSurface,
      ),
      contentTextStyle: TextStyle(
        fontSize: AppTokens.fontSize,
        color: scheme.onSurface,
      ),
    ),
    textTheme: TextTheme(
      titleLarge: TextStyle(
        fontSize: AppTokens.fontSizeTitle,
        fontWeight: FontWeight.w600,
      ),
      titleMedium: TextStyle(
        fontSize: AppTokens.fontSizeSubtitle,
        fontWeight: FontWeight.w600,
      ),
      bodyMedium: TextStyle(fontSize: baseSize),
      bodySmall: TextStyle(fontSize: baseSize - 1),
      labelLarge: TextStyle(fontSize: baseSize),
    ),
  );
}
