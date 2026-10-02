import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

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
}
