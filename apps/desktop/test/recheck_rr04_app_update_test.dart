import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// Bridge whose app-update spec can be configured per test.
class _SpecBridge extends SyntheticBridgePort {
  _SpecBridge(this.spec);

  final c.ExternalSpecDto spec;

  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    t16Calls.add('app_update_spec');
    return spec;
  }
}

c.ExternalSpecDto okSpec({String? helper = '/app/v2rayN-upgrade.exe'}) =>
    c.ExternalSpecDto(
      ok: true,
      helperExe: helper,
      source: '/app/.staging/v2rayN-7.99.0',
      installRoot: '/app',
      waitForPid: 4242,
      args: const <String>[
        '--plan',
        '/app/.staging/plan.json',
        '--result',
        '/app/.staging/result.json',
        '--pid',
        '4242',
        '--restart-exe',
        '/app/v2rayn_desktop.exe',
        '--restart-cwd',
        '/app',
      ],
    );

const _unconfiguredSpec = c.ExternalSpecDto(
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

/// Recorded runner launch + exit.
class _Handoff {
  final List<String> launches = <String>[];
  int exits = 0;
}

ProviderContainer _containerFor(
  SyntheticBridgePort bridge,
  _Handoff handoff, {
  bool failLaunch = false,
}) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    updateControllerProvider.overrideWith(
      () => UpdateController(
        launchRunner: (helper, args, cwd) async {
          if (failLaunch) throw Exception('spawn failed');
          handoff.launches.add('$helper|${args.join(",")}|$cwd');
        },
        exitApp: () => handoff.exits++,
      ),
    ),
  ],
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
  testWidgets('success: runner launched detached-ish and app exits', (
    tester,
  ) async {
    final bridge = _SpecBridge(okSpec());
    final handoff = _Handoff();
    final container = _containerFor(bridge, handoff);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec'));
    expect(handoff.launches, hasLength(1));
    expect(handoff.launches.single, startsWith('/app/v2rayN-upgrade.exe|'));
    expect(handoff.launches.single, contains('--plan'));
    expect(handoff.launches.single, endsWith('|/app'));
    expect(handoff.exits, 1);
    expect(find.textContaining('已启动更新程序，应用将退出'), findsOneWidget);
  });

  testWidgets('launch failure is reported and app does not exit', (
    tester,
  ) async {
    final bridge = _SpecBridge(okSpec());
    final handoff = _Handoff();
    final container = _containerFor(bridge, handoff, failLaunch: true);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
    expect(find.textContaining('启动更新程序失败'), findsOneWidget);
    expect(find.textContaining('已启动更新程序'), findsNothing);
  });

  testWidgets('missing helper path is reported, no launch, no exit', (
    tester,
  ) async {
    final bridge = _SpecBridge(okSpec(helper: null));
    final handoff = _Handoff();
    final container = _containerFor(bridge, handoff);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
    expect(find.textContaining('启动更新程序失败'), findsOneWidget);
  });

  testWidgets('spec unavailable: blocked message and no launch', (
    tester,
  ) async {
    final bridge = _SpecBridge(_unconfiguredSpec);
    final handoff = _Handoff();
    final container = _containerFor(bridge, handoff);
    addTearDown(container.dispose);
    await pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec'));
    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
    expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);
  });
}
