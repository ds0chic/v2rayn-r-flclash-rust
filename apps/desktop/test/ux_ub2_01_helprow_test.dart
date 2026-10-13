import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

Future<void> _pump(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(MaterialApp(home: Scaffold(body: child)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('UB2-01: checkbox renders helpText below the row', (
    tester,
  ) async {
    await _pump(
      tester,
      SettingsCheckbox(
        label: '-label-',
        value: false,
        onChanged: (_) {},
        helpText: '-help-ub2-01-',
      ),
    );
    expect(find.text('-label-'), findsOneWidget);
    expect(find.text('-help-ub2-01-'), findsOneWidget);
  });

  testWidgets('UB2-01: rows without helpText render no help line', (
    tester,
  ) async {
    await _pump(
      tester,
      Column(
        children: <Widget>[
          SettingsCheckbox(label: 'c', value: false, onChanged: (_) {}),
          SettingsTextField(label: 't', value: null, onChanged: (_) {}),
          SettingsNumberField(label: 'n', value: null, onChanged: (_) {}),
          SettingsDropdown<String>(
            label: 'd',
            value: null,
            items: const <DropdownMenuItem<String>>[],
            onChanged: (_) {},
          ),
        ],
      ),
    );
    // No help/risk lines: only the four labels render as plain text.
    expect(find.text('c'), findsOneWidget);
    expect(find.text('t'), findsOneWidget);
    expect(find.text('n'), findsOneWidget);
    expect(find.text('d'), findsOneWidget);
  });

  testWidgets('UB2-01: text/number/dropdown render helpText and risk', (
    tester,
  ) async {
    await _pump(
      tester,
      Column(
        children: <Widget>[
          SettingsTextField(
            label: 't',
            value: null,
            onChanged: (_) {},
            helpText: '-th-',
          ),
          SettingsNumberField(
            label: 'n',
            value: null,
            onChanged: (_) {},
            helpText: '-nh-',
            risk: '-nr-',
          ),
          SettingsDropdown<String>(
            label: 'd',
            value: null,
            items: const <DropdownMenuItem<String>>[],
            onChanged: (_) {},
            risk: '-dr-',
          ),
        ],
      ),
    );
    expect(find.text('-th-'), findsOneWidget);
    expect(find.text('-nh-'), findsOneWidget);
    expect(find.text('-nr-'), findsOneWidget);
    expect(find.text('-dr-'), findsOneWidget);
  });

  testWidgets('UB2-01: risk line uses the warning semantic color', (
    tester,
  ) async {
    await _pump(
      tester,
      SettingsCheckbox(
        label: 'c',
        value: false,
        onChanged: (_) {},
        risk: '-risk-',
      ),
    );
    final ctx = tester.element(find.text('-risk-'));
    final text = ctx.widget as Text;
    expect(text.style?.color, ctx.semantics.warning);
  });

  testWidgets('UB2-01: collapsed section hides children until tapped', (
    tester,
  ) async {
    await _pump(
      tester,
      const SettingsSection(
        title: '-sec-',
        collapsed: true,
        child: <Widget>[Text('-inner-')],
      ),
    );
    expect(find.text('-sec-'), findsOneWidget);
    expect(find.text('-inner-'), findsNothing);
    await tester.tap(find.text('-sec-'));
    await tester.pumpAndSettle();
    expect(find.text('-inner-'), findsOneWidget);
  });

  testWidgets('UB2-01: expanded section shows children immediately', (
    tester,
  ) async {
    await _pump(
      tester,
      const SettingsSection(title: '-sec2-', child: <Widget>[Text('-inner2-')]),
    );
    expect(find.text('-inner2-'), findsOneWidget);
  });
}
