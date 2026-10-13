import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

double _linearChannel(int c) {
  final s = c / 255.0;
  return s <= 0.04045
      ? s / 12.92
      : math.pow((s + 0.055) / 1.055, 2.4).toDouble();
}

double _luminance(Color c) {
  final v = c.value;
  return 0.2126 * _linearChannel((v >> 16) & 0xFF) +
      0.7152 * _linearChannel((v >> 8) & 0xFF) +
      0.0722 * _linearChannel(v & 0xFF);
}

/// WCAG contrast ratio of two opaque colors.
double _contrast(Color a, Color b) {
  final l1 = _luminance(a);
  final l2 = _luminance(b);
  final hi = l1 > l2 ? l1 : l2;
  final lo = l1 > l2 ? l2 : l1;
  return (hi + 0.05) / (lo + 0.05);
}

void main() {
  test('row height follows the configured font size within 24..28', () {
    expect(AppTokens.rowHeightFor(null), AppTokens.tableRowHeight);
    expect(AppTokens.rowHeightFor(AppTokens.fontSizeSmall), 24);
    expect(AppTokens.rowHeightFor(AppTokens.fontSize), 26);
    expect(AppTokens.rowHeightFor(14), 28);
    for (final size in <double>[11.5, 12.5, 14]) {
      final h = AppTokens.rowHeightFor(size);
      expect(h, inInclusiveRange(24, 28));
    }
  });

  test('light and dark themes carry distinct semantic tokens', () {
    final light = buildAppTheme(Brightness.light);
    final dark = buildAppTheme(Brightness.dark);
    final ls = light.extension<AppSemanticColors>();
    final ds = dark.extension<AppSemanticColors>();
    expect(ls, isNotNull);
    expect(ds, isNotNull);
    expect(ls!.success, isNot(equals(ds!.success)));
    expect(ls.warning, isNot(equals(ds.warning)));
    expect(ls.gridLine, isNot(equals(ds.gridLine)));
    expect(ls.disabledForeground, isNot(equals(ds.disabledForeground)));
    expect(ls.zebraEnabled, isFalse);
    expect(ds.zebraEnabled, isFalse);
  });

  test('font size and zebra flag reach the theme extension', () {
    final theme = buildAppTheme(
      Brightness.light,
      fontSize: 14,
      zebraEnabled: true,
    );
    final semantics = theme.extension<AppSemanticColors>()!;
    expect(semantics.tableRowHeight, 28);
    expect(semantics.zebraEnabled, isTrue);
    expect(theme.textTheme.bodyMedium!.fontSize, 14);
    expect(theme.visualDensity, VisualDensity.compact);
  });

  test('accent mapping falls back and the icon family is stable', () {
    expect(accentColorFor('Blue'), accentColors['Blue']);
    expect(accentColorFor('Nope'), defaultAccent);
    expect(AppTokens.icon('settings'), Icons.settings_outlined);
    expect(AppTokens.icon('logs'), Icons.message_outlined);
    expect(AppTokens.icon('missing-semantic'), Icons.circle_outlined);
  });

  test('window metrics match the ledger minimums', () {
    expect(AppWindowMetrics.title, 'v2rayN');
    expect(AppWindowMetrics.defaultSize, const Size(1200, 800));
    expect(AppWindowMetrics.minWidth, 800);
  });

  test('UX2-01/03: every text/background pair meets WCAG AA (4.5:1)', () {
    final light = buildAppTheme(Brightness.light);
    final dark = buildAppTheme(Brightness.dark);
    final ls = light.extension<AppSemanticColors>()!;
    final ds = dark.extension<AppSemanticColors>()!;
    final lightBg = AppTokens.lightSurface;
    final darkBg = AppTokens.darkSurface;
    final pairs = <String, (Color, Color)>{
      'light primary': (const Color(0xFF0E6B5E), lightBg),
      'light success': (ls.success, lightBg),
      'light warning': (ls.warning, lightBg),
      'light error': (ls.error, lightBg),
      'light info': (ls.info, lightBg),
      'light muted': (ls.muted, lightBg),
      'light onSurface': (light.colorScheme.onSurface, lightBg),
      'light onSurfaceVariant': (light.colorScheme.onSurfaceVariant, lightBg),
      'dark primary': (const Color(0xFF5FD0B5), darkBg),
      'dark success': (ds.success, darkBg),
      'dark warning': (ds.warning, darkBg),
      'dark error': (ds.error, darkBg),
      'dark info': (ds.info, darkBg),
      'dark muted': (ds.muted, darkBg),
      'dark onSurface': (dark.colorScheme.onSurface, darkBg),
      'dark onSurfaceVariant': (dark.colorScheme.onSurfaceVariant, darkBg),
    };
    final ratios = <String, double>{};
    pairs.forEach((name, pair) {
      final ratio = _contrast(pair.$1, pair.$2);
      ratios[name] = ratio;
      expect(
        ratio,
        greaterThanOrEqualTo(4.5),
        reason: '$name contrast $ratio < 4.5',
      );
    });
    debugPrint(
      ratios.entries
          .map((e) => '${e.key}=${e.value.toStringAsFixed(2)}')
          .join(' '),
    );
  });

  test('UX2-03: default accent is display fallback only, never persisted', () {
    expect(defaultAccent, const Color(0xFF0E6B5E));
    expect(accentColorFor(null), defaultAccent);
    expect(accentColorFor(''), defaultAccent);
    expect(accentColorFor('Nope'), defaultAccent);
    expect(accentColorFor('TealGreen'), const Color(0xFF0E6B5E));
    final ui = defaultSettingsJson()['UiItem'] as Map<String, dynamic>;
    expect(
      ui['ColorPrimaryName'],
      isNull,
      reason:
          'persisted accent default stays null; the teal-green seed '
          'only renders when no accent is saved',
    );
  });
}
