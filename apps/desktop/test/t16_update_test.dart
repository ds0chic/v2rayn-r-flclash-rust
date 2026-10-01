import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';

ProviderContainer makeContainer(SyntheticBridgePort bridge) =>
    ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );

Future<void> pumpUpdate(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
    ),
  );
  await tester.pump();
}

void main() {
  testWidgets('unsupported targets render disabled', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    final supported = tester.widget<Checkbox>(
      find.byKey(const ValueKey('update-core-xray')),
    );
    expect(supported.onChanged, isNotNull);

    final unsupported = tester.widget<Checkbox>(
      find.byKey(const ValueKey('update-core-tuic')),
    );
    expect(unsupported.onChanged, isNull);
    expect(find.textContaining('不支持更新'), findsOneWidget);
  });

  testWidgets('via-proxy check without a local port is structured', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort()..proxyAvailable = false;
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-via-proxy')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('update-check-only-btn')));
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.any((call) => call.startsWith('check_updates:')),
      isTrue,
    );
    expect(find.textContaining('E_PROXY_UNAVAILABLE'), findsOneWidget);
  });

  testWidgets('prerelease toggle is forwarded to the bridge', (tester) async {
    final bridge = SyntheticBridgePort()..proxyAvailable = true;
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-prerelease')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('update-check-only-btn')));
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.any(
        (call) =>
            call.startsWith('check_updates:') && call.endsWith(':true:false'),
      ),
      isTrue,
    );
    expect(find.textContaining('发现'), findsOneWidget);
  });

  testWidgets('check-update applies core updates and reports counts', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-apply-btn')));
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.any((call) => call.startsWith('apply_core:')),
      isTrue,
    );
    expect(find.textContaining('已更新'), findsOneWidget);
  });

  testWidgets('application update only produces an external spec', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec'));
    expect(find.textContaining('需要外部进程执行'), findsOneWidget);
  });

  testWidgets('rendering the window performs no update call', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    // Merely opening the window must not check or apply anything.
    expect(
      bridge.t16Calls.any((call) => call.startsWith('check_updates:')),
      isFalse,
    );
    expect(
      bridge.t16Calls.any((call) => call.startsWith('apply_core:')),
      isFalse,
    );
  });
}
