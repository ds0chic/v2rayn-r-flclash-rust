import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// S8-07: checking with zero selected targets must say so explicitly instead
/// of reporting a fake "all up to date".
void main() {
  testWidgets('empty selection reports "no targets selected"', (tester) async {
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);
    // The synthetic bridge has no proxy; check directly instead.
    (container.read(bridgePortProvider) as SyntheticBridgePort).proxyAvailable =
        true;

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
      ),
    );
    await tester.pumpAndSettle();

    // Deselect every supported core.
    for (final core in const <String>['v2rayN', 'xray', 'mihomo', 'sing_box']) {
      final box = find.byKey(ValueKey('update-core-$core'));
      expect(box, findsOneWidget, reason: 'missing checkbox for $core');
      await tester.tap(box);
      await tester.pumpAndSettle();
    }
    expect(
      container.read(updateControllerProvider).selected,
      isEmpty,
      reason: 'all cores must be deselected',
    );

    await tester.tap(find.byKey(const ValueKey('update-check-only-btn')));
    await tester.pumpAndSettle();

    expect(find.textContaining('未选择检查目标'), findsOneWidget);
    expect(find.textContaining('全部为最新版本'), findsNothing);
  });
}
