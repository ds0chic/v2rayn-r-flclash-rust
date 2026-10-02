import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';

import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _containerWith(RuntimeBridge bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(10),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      runtimeBridgeProvider.overrideWithValue(bridge),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _pumpShell(WidgetTester tester, ProviderContainer container) {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  return tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: MainShell()),
    ),
  );
}

/// T18b: the toolbar applies the real persisted plan (active node), the
/// status bar shows desired/applied revisions with an unapplied hint, and
/// structured apply errors surface verbatim instead of a fake running state.
void main() {
  testWidgets('apply button drives the real plan; rev mismatch shows 未应用', (
    tester,
  ) async {
    final container = _containerWith(
      SyntheticRuntimeBridge(
        initial: RuntimeView(
          desiredRevision: BigInt.from(4),
          appliedRevision: BigInt.from(2),
        ),
      ),
    );
    // Load the snapshot (rev 4/2) before the first frame, mirroring the
    // app bootstrap which refreshes on launch.
    await container.read(runtimeControllerProvider.notifier).refresh();
    await _pumpShell(tester, container);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));

    // Revision mismatch renders the 未应用 hint in toolbar + status bar.
    expect(find.textContaining('未应用'), findsWidgets);
    expect(find.byKey(const ValueKey('runtime-start')), findsOneWidget);
    expect(find.byKey(const ValueKey('runtime-stop')), findsOneWidget);

    // Tapping 应用 applies and refreshes to the Running snapshot.
    await tester.tap(find.byKey(const ValueKey('runtime-start')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    final view = container.read(runtimeControllerProvider);
    expect(view.state, 'Running');
    expect(view.hasUnappliedChanges, isFalse);
    expect(find.textContaining('未应用'), findsNothing);
  });

  testWidgets('failed apply keeps the structured error on screen', (
    tester,
  ) async {
    final container = _containerWith(_FailingBridge());
    await _pumpShell(tester, container);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));

    await tester.tap(find.byKey(const ValueKey('runtime-start')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(find.byKey(const ValueKey('runtime-error')), findsOneWidget);
    expect(find.textContaining('E_NO_ACTIVE_PROFILE'), findsOneWidget);
  });

  test('hasUnappliedChanges is true only on a real mismatch', () {
    expect(const RuntimeView().hasUnappliedChanges, isFalse);
    expect(
      RuntimeView(
        desiredRevision: BigInt.from(3),
        appliedRevision: BigInt.from(3),
      ).hasUnappliedChanges,
      isFalse,
    );
    expect(
      RuntimeView(
        desiredRevision: BigInt.from(4),
        appliedRevision: BigInt.from(3),
      ).hasUnappliedChanges,
      isTrue,
    );
  });
}

class _FailingBridge implements RuntimeBridge {
  final StreamController<RuntimeEvent> _events =
      StreamController<RuntimeEvent>.broadcast();

  @override
  Future<RuntimeView> snapshot() async => const RuntimeView();

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(
    ok: false,
    error: RuntimeErrorView(
      code: 'E_NO_ACTIVE_PROFILE',
      messageKey: 'error.no_active_profile',
    ),
  );

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);

  @override
  Stream<RuntimeEvent> events() => _events.stream;
}
