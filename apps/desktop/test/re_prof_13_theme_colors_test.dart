// RE-PROF-13 / P2: the active-row marker must stay distinguishable from the
// selection highlight in both light and dark themes. Pure theme resolution, so
// no page build (keeps the leak-prone flutter_tester out of this file).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';

typedef _Resolved = ({Color active, Color selected, Brightness brightness});

Future<_Resolved> _resolve(WidgetTester tester, ThemeData theme) async {
  late Color active;
  late Color selected;
  late Brightness brightness;
  await tester.pumpWidget(
    Theme(
      data: theme,
      child: Builder(
        builder: (context) {
          active = activeRowFill(context);
          selected = Theme.of(context).colorScheme.primaryContainer;
          brightness = Theme.of(context).brightness;
          return const SizedBox.shrink();
        },
      ),
    ),
  );
  return (active: active, selected: selected, brightness: brightness);
}

void main() {
  testWidgets('active fill differs from selection in light and dark themes', (
    tester,
  ) async {
    final light = await _resolve(tester, ThemeData.light());
    final dark = await _resolve(tester, ThemeData.dark());

    expect(light.brightness, Brightness.light);
    expect(dark.brightness, Brightness.dark);
    expect(light.active, isNot(dark.active));
    expect(light.active, isNot(light.selected));
    expect(dark.active, isNot(dark.selected));
  });
}
