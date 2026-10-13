import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:two_dimensional_scrollables/two_dimensional_scrollables.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import 'support/profiles_harness.dart';

double _tableRowHeight(WidgetTester tester) {
  final element = tester.element(find.byType(ProfilesTable));
  return Theme.of(element).extension<AppSemanticColors>()!.tableRowHeight;
}

void main() {
  test('density defaults to standard and round-trips the store', () {
    final store = MemoryUiStateStore();
    ProviderContainer open() {
      final container = ProviderContainer(
        overrides: [uiStateStoreProvider.overrideWithValue(store)],
      );
      addTearDown(container.dispose);
      return container;
    }

    // The shell controller only reads the UI-state store: no bridge fakes
    // needed for these units.
    expect(
      open().read(uiShellControllerProvider).tableDensity,
      TableDensity.standard,
    );
    final writer = open();
    writer
        .read(uiShellControllerProvider.notifier)
        .setDensity(TableDensity.relaxed);
    expect(store.loadSection('layout')!['density'], 'relaxed');
    expect(
      open().read(uiShellControllerProvider).tableDensity,
      TableDensity.relaxed,
    );
  });

  test('density persists and restores across controller builds', () {
    final store = MemoryUiStateStore();
    final container = ProviderContainer(
      overrides: [uiStateStoreProvider.overrideWithValue(store)],
    );
    addTearDown(container.dispose);
    // Only the shell controller is read, so missing bridge overrides are fine.
    container
        .read(uiShellControllerProvider.notifier)
        .setDensity(TableDensity.compact);
    expect(store.loadSection('layout')!['density'], 'compact');
    final restored = ProviderContainer(
      overrides: [uiStateStoreProvider.overrideWithValue(store)],
    );
    addTearDown(restored.dispose);
    expect(
      restored.read(uiShellControllerProvider).tableDensity,
      TableDensity.compact,
    );
    restored.read(uiShellControllerProvider.notifier).cycleDensity();
    expect(
      restored.read(uiShellControllerProvider).tableDensity,
      TableDensity.standard,
    );
  });

  testWidgets('density button cycles the table row height', (tester) async {
    final store = MemoryUiStateStore();
    final container = await pumpApp(tester, rows: 5, store: store);
    expect(_tableRowHeight(tester), 28);
    await tester.tap(find.byKey(const ValueKey('density-cycle')));
    await tester.pumpAndSettle();
    expect(
      container.read(uiShellControllerProvider).tableDensity,
      TableDensity.relaxed,
    );
    expect(_tableRowHeight(tester), 32);
    expect(store.loadSection('layout')!['density'], 'relaxed');
    // Switching density keeps the virtualized table intact.
    await tester.tap(find.byKey(const ValueKey('density-cycle')));
    await tester.pumpAndSettle();
    expect(_tableRowHeight(tester), 24);
    expect(find.byType(TableView), findsOneWidget);
  });
}
