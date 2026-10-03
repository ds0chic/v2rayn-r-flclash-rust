// FIX-16D: the node-column editor exposes upstream `MainColumnItem.Width`
// (LAY-PROFILES-003) and persists it through the unified ui_state source, so a
// reopen restores the width without changing the stable column key.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

double _widthOf(WidgetTester tester, String key) {
  final text = tester.widget<Text>(
    find.byKey(ValueKey<String>('colwidth-$key')),
  );
  return double.parse(text.data!);
}

Future<void> _openDialog(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: const TextScaler.linear(1.5)),
          child: child!,
        ),
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => ElevatedButton(
              key: const ValueKey<String>('open-columns'),
              onPressed: () => showColumnSettingsDialog(context, ref),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey<String>('open-columns')));
  await tester.pumpAndSettle();
}

ProviderContainer _container(UiStateStore store) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
    uiStateStoreProvider.overrideWithValue(store),
    profileRowCountProvider.overrideWithValue(10),
  ],
);

void main() {
  testWidgets('column editor adjusts width, persists it and stays reachable', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(const Size(1280, 720));

    final store = MemoryUiStateStore();
    final first = _container(store);
    addTearDown(first.dispose);

    await _openDialog(tester, first);
    expect(tester.takeException(), isNull);

    final before = _widthOf(tester, 'Remarks');
    expect(before, 150);

    await tester.tap(find.byKey(const ValueKey<String>('colwidthinc-Remarks')));
    await tester.pump();
    expect(_widthOf(tester, 'Remarks'), before + 10);

    // The stable key is persisted, never the localized label.
    final widths = store.loadSection('column_layout')!['widths'] as Map;
    expect(widths['Remarks'], 160);

    // Close button is reachable and dismisses the dialog.
    await tester.tap(
      find.byKey(const ValueKey<String>('column-settings-close')),
    );
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey<String>('column-settings-dialog')),
      findsNothing,
    );

    // Reopen with a fresh container over the same store: width restored.
    final second = _container(store);
    addTearDown(second.dispose);
    await _openDialog(tester, second);
    expect(_widthOf(tester, 'Remarks'), 160);
    expect(tester.takeException(), isNull);
  });
}
