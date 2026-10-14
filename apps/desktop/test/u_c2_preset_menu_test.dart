import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/profiles_harness.dart';

/// U-C2 C2-1: the settings menu carries UI-PRESET-DLG, opening the U-B2
/// preset dialog in independent mode (desired chain only, undo disabled).
void main() {
  test('settings carries the preset entry after cleanup, before main', () {
    final settings = mainMenuModel.firstWhere((e) => e.label == '设置').submenu;
    final ids = settings.map((e) => e.actionId).toList();
    expect(ids.indexOf('UI-PRESET-DLG'), ids.indexOf('UI-CLEANUP') + 1);
    expect(ids.indexOf('UI-MAIN'), ids.indexOf('UI-PRESET-DLG') + 1);
    final entry = settings.firstWhere((e) => e.actionId == 'UI-PRESET-DLG');
    expect(entry.label, '推荐配置…');
    expect(entry.l10nKey, 'uiPresetDialog');
    expect(entry.isInvocable, isTrue);
    expect(entry.separatorAfter, isFalse);
    // M8 separator lock: the new entry adds none.
    expect(settings.where((e) => e.separatorAfter).length, 2);
  });

  testWidgets('menu opens the preset dialog without crashing when empty', (
    tester,
  ) async {
    await pumpApp(tester, rows: 0);
    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-推荐配置…')));
    await tester.pumpAndSettle();
    expect(find.text('一键推荐方案'), findsOneWidget);
    // Condition C: empty library / unloaded schemes must degrade honestly
    // (dropdown or the import-builtin hint), never crash.
    final hasDropdown = find
        .byKey(const ValueKey('settings-preset-scheme'))
        .evaluate()
        .isNotEmpty;
    final hasImportHint = find.textContaining('导入内置规则').evaluate().isNotEmpty;
    expect(hasDropdown || hasImportHint, isTrue);
    // Undo stays disabled: there is no snapshot API (UX3-12).
    final undo = tester.widget<TextButton>(
      find.byKey(const ValueKey('settings-preset-undo')),
    );
    expect(undo.onPressed, isNull);
    await tester.tap(find.byKey(const ValueKey('settings-preset-cancel')));
    await tester.pumpAndSettle();
    expect(find.text('一键推荐方案'), findsNothing);
  });

  testWidgets('cancel writes nothing to the settings document', (tester) async {
    final container = await pumpApp(tester, rows: 0);
    final before = Map<String, dynamic>.from(
      container.read(settingsControllerProvider).document,
    );
    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-推荐配置…')));
    await tester.pumpAndSettle();
    expect(find.text('一键推荐方案'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('settings-preset-cancel')));
    await tester.pumpAndSettle();
    final after = container.read(settingsControllerProvider).document;
    expect(after['RoutingBasicItem'], before['RoutingBasicItem']);
    expect(after['SimpleDNSItem'], before['SimpleDNSItem']);
    expect(after['Inbound'], before['Inbound']);
    expect(after['TunModeItem'], before['TunModeItem']);
  });

  testWidgets('seeded diff applies through the desired chain with a message', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 0);
    // Force exactly one diff so the apply path is deterministic. The fresh
    // document may not carry the group yet (defaults merge at read time).
    final doc = container.read(settingsControllerProvider).document;
    final Object? existing = doc['RoutingBasicItem'];
    final Map<String, dynamic> routing = existing is Map
        ? Map<String, dynamic>.from(existing)
        : <String, dynamic>{};
    routing['DomainStrategy'] = 'UseIP';
    final seeded = container
        .read(settingsControllerProvider.notifier)
        .saveGroup('RoutingBasicItem', routing);
    expect(seeded.ok, isTrue);
    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-推荐配置…')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-preset-scenario-B')));
    await tester.pump();
    expect(
      find.byKey(
        const ValueKey('settings-preset-diff-RoutingBasicItem.DomainStrategy'),
      ),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('settings-preset-apply')));
    // UB2-10 lesson: bounded pumps, never a bare pumpAndSettle after saves.
    for (var i = 0; i < 30; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    await tester.pumpAndSettle();
    // Apply pops only after every listed group wrote successfully.
    expect(find.text('一键推荐方案'), findsNothing);
    final message = container.read(uiShellControllerProvider).message;
    expect(message, contains('已应用'));
    expect(message, contains('系统代理/TUN/端口未动'));
    final restored =
        (container.read(settingsControllerProvider).document['RoutingBasicItem']
            as Map)['DomainStrategy'];
    expect(restored, 'AsIs');
  });
}
