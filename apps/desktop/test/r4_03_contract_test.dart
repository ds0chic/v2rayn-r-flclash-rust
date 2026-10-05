// R4-03 contract: a missing core has an explicit install/repair entry wired to
// the existing update flow, and a failed install stays visible and retryable.
//
// Synthetic only: no native library, no network, no port use and no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// T16 double that records the existing check/apply pipeline and can fail the
/// install on demand. It never fabricates a Running runtime fact.
class _UpdateBridge extends SyntheticBridgePort {
  _UpdateBridge({this.failInstall = false});

  bool failInstall;

  @override
  Future<c.ApplyCoreResultDto> t16ApplyCoreUpdate(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async {
    final result = await super.t16ApplyCoreUpdate(cores, prerelease, viaProxy);
    if (!failInstall) return result;
    return const c.ApplyCoreResultDto(
      ok: false,
      applied: <c.AppliedCoreDto>[],
      skipped: <String>[],
      error: c.ErrorDto(
        code: 'E_UNAVAILABLE',
        messageKey: 'error.update_download_failed',
        retryable: true,
      ),
    );
  }
}

ProviderContainer _makeContainer(_UpdateBridge bridge) {
  // Upstream `CheckUpdateItem.UpdateViaProxy` defaults true; this synthetic
  // scenario models a reachable local proxy so the check/apply pipeline runs.
  bridge.proxyAvailable = true;
  final container = ProviderContainer(
    overrides: [bridgePortProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  return container;
}

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
  testWidgets('no missing core keeps the install entry hidden', (tester) async {
    final bridge = _UpdateBridge();
    final container = _makeContainer(bridge);
    await pumpUpdate(tester, container);
    expect(find.byKey(const ValueKey('update-missing-cores')), findsNothing);
    expect(
      find.byKey(const ValueKey('update-install-missing-btn')),
      findsNothing,
    );
  });

  testWidgets(
    'a missing core exposes an install entry that runs the update flow',
    (tester) async {
      final bridge = _UpdateBridge();
      final container = _makeContainer(bridge);
      container.read(updateControllerProvider.notifier).seedMissingCores(
        const <String>['xray'],
      );
      await pumpUpdate(tester, container);

      expect(
        find.byKey(const ValueKey('update-missing-cores')),
        findsOneWidget,
      );
      expect(find.textContaining('缺少内核'), findsOneWidget);

      await tester.tap(
        find.byKey(const ValueKey('update-install-missing-btn')),
      );
      await tester.pumpAndSettle();

      expect(
        bridge.t16Calls.any((call) => call.startsWith('check_updates:xray')),
        isTrue,
      );
      expect(
        bridge.t16Calls.any((call) => call.startsWith('apply_core:xray')),
        isTrue,
      );
      expect(find.textContaining('已安装'), findsOneWidget);
      // Repaired: the missing banner is cleared and a restart can be retried.
      expect(find.byKey(const ValueKey('update-missing-cores')), findsNothing);
    },
  );

  testWidgets('a failed install stays visible and retryable', (tester) async {
    final bridge = _UpdateBridge(failInstall: true);
    final container = _makeContainer(bridge);
    container.read(updateControllerProvider.notifier).seedMissingCores(
      const <String>['xray'],
    );
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-install-missing-btn')));
    await tester.pumpAndSettle();

    expect(find.textContaining('内核安装失败'), findsOneWidget);
    expect(find.textContaining('E_UNAVAILABLE'), findsOneWidget);
    // The core is still missing, so the entry (and its retry) stays.
    expect(find.byKey(const ValueKey('update-missing-cores')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('update-install-missing-btn')),
      findsOneWidget,
    );
  });
}
