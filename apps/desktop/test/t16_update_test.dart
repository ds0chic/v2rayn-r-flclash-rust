import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// Application self-update with no configured release source (blocked).
class _UnconfiguredAppBridge extends SyntheticBridgePort {
  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    t16Calls.add('app_update_spec');
    return const c.ExternalSpecDto(
      ok: false,
      helperExe: null,
      source: null,
      installRoot: null,
      waitForPid: 0,
      args: <String>[],
      error: c.ErrorDto(
        code: 'E_UNAVAILABLE',
        messageKey: 'error.update_app_source_unconfigured',
        retryable: false,
      ),
    );
  }
}

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
    // Upstream `CheckUpdateItem.UpdateViaProxy` defaults true, so the check is
    // already proxied; no local port is available -> structured error.
    final bridge = SyntheticBridgePort()..proxyAvailable = false;
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

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
            call.startsWith('check_updates:') && call.endsWith(':true:true'),
      ),
      isTrue,
    );
    expect(find.textContaining('发现'), findsOneWidget);
  });

  testWidgets('check-update applies core updates and reports counts', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort()..proxyAvailable = true;
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

  testWidgets('application update launches the runner and exits', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final launches = <String>[];
    var exited = 0;
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        updateControllerProvider.overrideWith(
          () => UpdateController(
            launchRunner: (helper, args, cwd) async =>
                launches.add('$helper|${args.join(",")}|$cwd'),
            exitApp: () => exited++,
          ),
        ),
      ],
    );
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec'));
    expect(launches, hasLength(1));
    expect(launches.single, startsWith('/app/v2rayN-upgrade.exe|'));
    expect(launches.single, endsWith('|/app'));
    expect(exited, 1);
    expect(find.textContaining('已启动更新程序，应用将退出'), findsOneWidget);
  });

  testWidgets('unconfigured application source is reported as blocked', (
    tester,
  ) async {
    final bridge = _UnconfiguredAppBridge();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec'));
    expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);
    expect(find.textContaining('E_UNAVAILABLE'), findsOneWidget);
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
