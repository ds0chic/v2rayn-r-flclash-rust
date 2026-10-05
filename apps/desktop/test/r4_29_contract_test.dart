// R4-29 contract: this project's own release/upgrade flow.
//
// Covers the card's must-pass scenarios against a synthetic bridge and a stub
// runner (no real process, no exit, no network, no port, no install):
//   * an unconfigured own release source is an explicit block, never the
//     upstream v2rayN release presented as an available self-update (R3-08);
//   * a checked package hands the plan/result/pid/restart CLI to the external
//     runner and exits (exit hand-off), with the flat install root as cwd;
//   * signature failure / missing signature / digest mismatch and a missing
//     helper are rejected with the current install left untouched and no
//     success/exit fabricated (FIX-12B semantics);
//   * a thrown bridge call clears busy instead of latching the window.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

const _blockedReport = c.UpdateReportDto(
  ok: true,
  checks: <c.CoreUpdateDto>[
    c.CoreUpdateDto(
      core: 'v2rayN',
      supported: false,
      note: 'error.update_app_source_unconfigured',
      hasUpdate: false,
    ),
  ],
);

c.ExternalSpecDto _okSpec() => const c.ExternalSpecDto(
  ok: true,
  helperExe: r'C:\app\v2rayN-upgrade.exe',
  source: r'C:\app\.staging\app-7.99.0',
  installRoot: r'C:\app',
  waitForPid: 4242,
  args: <String>[
    '--plan',
    r'C:\app\.staging\upgrade-plan-7.99.0.json',
    '--result',
    r'C:\app\.staging\upgrade-result-7.99.0.json',
    '--pid',
    '4242',
    '--restart-exe',
    r'C:\app\v2rayn_desktop.exe',
    '--restart-cwd',
    r'C:\app',
  ],
);

c.ExternalSpecDto _errorSpec({required String code, required String key}) =>
    c.ExternalSpecDto(
      ok: false,
      waitForPid: 0,
      args: const <String>[],
      error: c.ErrorDto(code: code, messageKey: key, retryable: false),
    );

/// Bridge whose app-update spec and update report are configurable.
class _ContractBridge extends SyntheticBridgePort {
  _ContractBridge({this.spec, this.report});

  final c.ExternalSpecDto? spec;
  final c.UpdateReportDto? report;

  @override
  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async => report ?? super.t16CheckUpdates(cores, prerelease, viaProxy);

  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async =>
      spec ?? super.t16ApplyAppUpdateSpec();
}

class _Handoff {
  final List<String> launches = <String>[];
  int exits = 0;
}

ProviderContainer _container(_ContractBridge bridge, _Handoff handoff) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      updateControllerProvider.overrideWith(
        () => UpdateController(
          launchRunner: (helper, args, cwd) async {
            handoff.launches.add('$helper|${args.join(",")}|$cwd');
          },
          exitApp: () => handoff.exits++,
        ),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _pump(WidgetTester tester, ProviderContainer container) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
    ),
  );
  await tester.pump();
}

Future<void> _tap(WidgetTester tester, String key) async {
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('unconfigured source is an explicit block, not a fake update', (
    tester,
  ) async {
    final handoff = _Handoff();
    final container = _container(
      _ContractBridge(report: _blockedReport),
      handoff,
    );
    await _pump(tester, container);
    await _tap(tester, 'update-check-only-btn');

    // The target row itself shows the explicit block and never an upstream
    // release version presented as an available self-update (R3-08).
    expect(find.text('应用自身发行源未配置'), findsOneWidget);
    expect(find.textContaining('已是最新'), findsNothing);
    expect(find.textContaining('可更新 '), findsNothing);
    expect(handoff.launches, isEmpty);
  });

  testWidgets('unconfigured source blocks the app hand-off', (tester) async {
    final handoff = _Handoff();
    final container = _container(
      _ContractBridge(
        spec: _errorSpec(
          code: 'E_UNAVAILABLE',
          key: 'error.update_app_source_unconfigured',
        ),
      ),
      handoff,
    );
    await _pump(tester, container);
    await _tap(tester, 'update-app-spec-btn');

    expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);
    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
    expect(find.textContaining('已启动更新程序'), findsNothing);
  });

  testWidgets('checked package hands the runner CLI over and exits', (
    tester,
  ) async {
    final handoff = _Handoff();
    final container = _container(_ContractBridge(spec: _okSpec()), handoff);
    await _pump(tester, container);
    await _tap(tester, 'update-app-spec-btn');

    expect(handoff.launches, hasLength(1));
    final launch = handoff.launches.single;
    expect(launch, startsWith(r'C:\app\v2rayN-upgrade.exe|'));
    expect(launch, contains('--plan'));
    expect(launch, contains('--result'));
    expect(launch, contains('--pid'));
    expect(launch, contains('--restart-exe'));
    expect(launch, contains('--restart-cwd'));
    expect(launch, endsWith(r'|C:\app'));
    expect(handoff.exits, 1);
    expect(find.textContaining('已启动更新程序，应用将退出'), findsOneWidget);
  });

  for (final failure in <(String, String)>[
    ('E_PERMISSION_DENIED', 'error.update_signature'), // wrong signature
    ('E_UNAVAILABLE', 'error.update_signature_missing'), // absent signature
    ('E_CONFLICT', 'error.update_failed'), // digest mismatch
  ]) {
    testWidgets('${failure.$2} is rejected without launch, exit or success', (
      tester,
    ) async {
      final handoff = _Handoff();
      final container = _container(
        _ContractBridge(
          spec: _errorSpec(code: failure.$1, key: failure.$2),
        ),
        handoff,
      );
      await _pump(tester, container);
      await _tap(tester, 'update-app-spec-btn');

      expect(handoff.launches, isEmpty);
      expect(handoff.exits, 0);
      expect(find.textContaining('已启动更新程序'), findsNothing);
      expect(find.textContaining('应用更新不可用'), findsOneWidget);
    });
  }

  testWidgets('missing helper path is rejected, no launch, no exit', (
    tester,
  ) async {
    final handoff = _Handoff();
    final container = _container(
      _ContractBridge(
        spec: const c.ExternalSpecDto(
          ok: true,
          helperExe: null,
          installRoot: r'C:\app',
          waitForPid: 4242,
          args: <String>['--plan', 'p.json'],
        ),
      ),
      handoff,
    );
    await _pump(tester, container);
    await _tap(tester, 'update-app-spec-btn');

    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
    expect(find.textContaining('启动更新程序失败'), findsOneWidget);
  });

  testWidgets('a thrown spec call clears busy instead of latching', (
    tester,
  ) async {
    final handoff = _Handoff();
    final container = _container(_ThrowingBridge(), handoff);
    await _pump(tester, container);
    await _tap(tester, 'update-app-spec-btn');

    final state = container.read(updateControllerProvider);
    expect(state.busy, isFalse);
    expect(state.status?.isError, isTrue);
    expect(handoff.launches, isEmpty);
    expect(handoff.exits, 0);
  });
}

class _ThrowingBridge extends _ContractBridge {
  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    throw StateError('bridge exploded');
  }
}
