import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import 'support/profiles_harness.dart';

Color _markerColor(WidgetTester tester, String id) {
  return tester
      .widget<ColoredBox>(find.byKey(ValueKey('active-marker-$id')))
      .color;
}

void main() {
  testWidgets('keyboard current row carries the focus outline', (tester) async {
    final container = await pumpApp(tester, rows: 5);
    await pressPlain(tester, LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    final focusedId = container.read(profilesControllerProvider).primaryId;
    expect(focusedId, isNotNull);
    final badge = find.byKey(ValueKey('active-marker-$focusedId'));
    expect(badge, findsOneWidget);
    final element = tester.element(badge);
    final expected = Theme.of(element).colorScheme.primary;
    expect(_markerColor(tester, focusedId!), expected);
  });

  testWidgets('focus outline hides when the table loses focus', (tester) async {
    final container = await pumpApp(tester, rows: 5);
    await pressPlain(tester, LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    final focusedId = container.read(profilesControllerProvider).primaryId!;
    // Move focus into the filter box: the table ring must clear.
    await tester.tap(find.byKey(const ValueKey('filter-field')));
    await tester.pumpAndSettle();
    expect(_markerColor(tester, focusedId), Colors.transparent);
  });

  test('production focus outline keeps 3:1 against row grounds', () {
    for (final brightness in [Brightness.light, Brightness.dark]) {
      final theme = buildAppTheme(brightness);
      final semantics = theme.extension<AppSemanticColors>()!;
      for (final ground in [
        theme.colorScheme.surface,
        theme.colorScheme.surfaceContainerHighest,
      ]) {
        expect(
          _contrast(semantics.focusOutline, ground),
          greaterThanOrEqualTo(3.0),
          reason: '$brightness focusOutline vs $ground',
        );
      }
    }
  });
}

double _contrast(Color a, Color b) {
  double lum(Color color) {
    double linearize(double v) {
      return v <= 0.04045 ? v / 12.92 : _pow24((v + 0.055) / 1.055);
    }

    return 0.2126 * linearize(color.r) +
        0.7152 * linearize(color.g) +
        0.0722 * linearize(color.b);
  }

  final l1 = lum(a);
  final l2 = lum(b);
  final high = l1 > l2 ? l1 : l2;
  final low = l1 > l2 ? l2 : l1;
  return (high + 0.05) / (low + 0.05);
}

// x^2.4 = x^2 * x^0.4; x^0.4 via 8 Newton-free bisection steps on y^5 = x^2.
double _pow24(double x) {
  if (x <= 0) return 0;
  final target = x * x;
  var lo = 0.0;
  var hi = x > 1 ? x : 1.0;
  for (var i = 0; i < 40; i++) {
    final mid = (lo + hi) / 2;
    var p = 1.0;
    for (var k = 0; k < 5; k++) {
      p *= mid;
    }
    if (p < target) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return x * x * (lo + hi) / 2;
}
