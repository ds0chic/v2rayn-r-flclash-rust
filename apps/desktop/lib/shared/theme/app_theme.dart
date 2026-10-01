import 'package:flutter/material.dart';

/// Central design tokens for the desktop shell.
///
/// T05 keeps a single component system (Material 3) and only re-skins it with
/// the compact desktop density called for by the plan (see
/// outputs/V2RAYN_FLUTTER_RUST_PLAN.md §7). No second component library is
/// introduced, so focus, menus and font metrics stay consistent.
///
/// Information density anchors come from compat/layouts.yaml:
///   - LAY-MAIN-002 default window 1200x800
///   - LAY-PROFILES-002 table row height ~26 logical px
class AppTokens {
  const AppTokens._();

  static const double fontSize = 12.5;
  static const double fontSizeSmall = 11.5;
  static const double menuBarHeight = 30;
  static const double toolbarHeight = 32;
  static const double statusBarHeight = 30;
  static const double tableRowHeight = 26;
  static const double tableHeaderHeight = 30;
  static const double tableHandleWidth = 48;
  static const double splitterThickness = 6;

  static const Color lightSurface = Color(0xFFF3F4F6);
  static const Color lightSurfaceContainer = Color(0xFFEAECEF);
  static const Color lightSurfaceHighest = Color(0xFFE1E4E8);
  static const Color lightOutline = Color(0xFFB9BEC6);
  static const Color lightGrid = Color(0x33000000);

  static const Color darkSurface = Color(0xFF1E2126);
  static const Color darkSurfaceContainer = Color(0xFF262A31);
  static const Color darkSurfaceHighest = Color(0xFF2E333B);
  static const Color darkOutline = Color(0xFF4A505A);
  static const Color darkGrid = Color(0x33FFFFFF);
}

ThemeData buildAppTheme(Brightness brightness) {
  final isLight = brightness == Brightness.light;
  final seed = const Color(0xFF1565C0);
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

  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    visualDensity: VisualDensity.compact,
    fontFamily: isLight ? null : null,
    dividerTheme: DividerThemeData(
      space: 1,
      thickness: 1,
      color: scheme.outlineVariant,
    ),
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
        shape: const WidgetStatePropertyAll(
          RoundedRectangleBorder(borderRadius: BorderRadius.zero),
        ),
        visualDensity: VisualDensity.compact,
      ),
    ),
    textTheme: const TextTheme(
      bodyMedium: TextStyle(fontSize: AppTokens.fontSize),
      bodySmall: TextStyle(fontSize: AppTokens.fontSizeSmall),
      labelLarge: TextStyle(fontSize: AppTokens.fontSize),
    ),
  );
}
