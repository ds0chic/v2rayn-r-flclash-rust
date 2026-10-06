// SP-19: the independent settings window inherits the main-window
// presentation (theme/accent/font/locale envelope, UI-08) and keeps all five
// upstream groups plus the confirm/cancel pair readable and operable in an
// 800px-high window at 100/125/150/200% DPI.
//
// Correct contract: the second engine never builds a second settings backend;
// it renders the shared OptionSettingWindow body under the presentation passed
// from the main window. Default stays light for back-compat (native host passes
// the envelope once the runner forwards it; tracked as the SP-19 follow-up).
//
// Red first: OptionSettingsWindowApp currently hard-codes
// buildAppTheme(Brightness.light), so the dark-inheritance assertions fail.
// One pumpWidget per file (locked flutter_tester segfault note).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window_entry.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

const _scales = <double>[1.0, 1.25, 1.5, 2.0];

class _FakeSettingsHost implements SettingsEditorHost {
  @override
  Future<Map<String, dynamic>> loadSnapshot() async => <String, dynamic>{
    'GuiItem': <String, dynamic>{'AutoRun': false},
  };

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async =>
      const SettingsEditorOutcome(ok: true);

  @override
  Future<void> close() async {}
}

void main() {
  testWidgets('settings window inherits presentation at 800px high', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      OptionSettingsWindowApp(
        host: _FakeSettingsHost(),
        brightness: Brightness.dark,
        fontSize: 12.5,
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 40));

    // Presentation envelope is honoured (UI-08): dark scheme, base font size.
    final context = tester.element(find.byType(Scaffold).first);
    expect(
      Theme.of(context).brightness,
      Brightness.dark,
      reason: 'settings window must inherit the dark presentation',
    );
    expect(
      Theme.of(context).textTheme.bodyMedium?.fontSize,
      12.5,
      reason: 'settings window must inherit the main-window font size',
    );

    for (final scale in _scales) {
      final label = '${(scale * 100).round()}%';
      tester.view.devicePixelRatio = scale;
      tester.view.physicalSize = Size(1000 * scale, 800 * scale);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 40));
      expect(
        tester.takeException(),
        isNull,
        reason: 'layout exception in settings window @$label',
      );
      // All five upstream groups stay reachable by tab at 800px high.
      expect(
        find.byType(Tab),
        findsNWidgets(5),
        reason: 'settings must expose five groups @$label',
      );
      // Confirm/cancel pair stays on screen and tappable at 800px high.
      for (final key in <String>['settings-save', 'settings-cancel']) {
        final finder = find.byKey(ValueKey<String>(key));
        expect(finder, findsOneWidget, reason: 'missing $key @$label');
        expect(
          finder.hitTestable(),
          findsOneWidget,
          reason: 'unreachable $key @$label',
        );
      }
    }
  });
}
