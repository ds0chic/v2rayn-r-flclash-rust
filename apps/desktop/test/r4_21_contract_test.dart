// R4-21 contract: the ordinary UI entry covers the full 14-core matrix and the
// missing-core -> install -> retry flow works for the auto-updatable cores.
//
// Synthetic only: no native library, no network, no port use, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

const _autoCores = <String>['xray', 'sing_box', 'mihomo'];
const _manualCores = <String>[
  'v2fly',
  'v2fly_v5',
  'hysteria',
  'naiveproxy',
  'tuic',
  'juicity',
  'hysteria2',
  'brook',
  'overtls',
  'shadowquic',
  'mieru',
];

/// Full production matrix (app + 14 proxy cores) with a switchable failure so
/// the failure/retry path can be exercised. Never fabricates a Running fact.
class _InstallBridge extends SyntheticBridgePort {
  _InstallBridge({this.failInstall = false});

  bool failInstall;

  @override
  List<c.UpdateTargetDto> t16UpdateTargets() => <c.UpdateTargetDto>[
    const c.UpdateTargetDto(
      core: 'v2rayN',
      repo: '2dust/v2rayN',
      supported: true,
      prereleaseCapable: true,
    ),
    for (final core in _autoCores)
      c.UpdateTargetDto(
        core: core,
        repo: 'repo/$core',
        supported: true,
        prereleaseCapable: core == 'xray',
      ),
    for (final core in _manualCores)
      c.UpdateTargetDto(
        core: core,
        repo: 'repo/$core',
        supported: false,
        prereleaseCapable: false,
        note: 'error.update_manual',
      ),
  ];

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

ProviderContainer _makeContainer(_InstallBridge bridge) {
  // A reachable local proxy so the synthetic check/apply pipeline runs.
  bridge.proxyAvailable = true;
  final container = ProviderContainer(
    overrides: [bridgePortProvider.overrideWithValue(bridge)],
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

void main() {
  testWidgets('matrix lists 14 proxy cores; auto enabled, manual labelled', (
    tester,
  ) async {
    final bridge = _InstallBridge();
    final container = _makeContainer(bridge);
    await _pump(tester, container);

    expect(_autoCores.length + _manualCores.length, 14);
    final targets = container.read(updateControllerProvider).targets;
    for (final core in <String>[..._autoCores, ..._manualCores]) {
      expect(
        targets.any((target) => target.core == core),
        isTrue,
        reason: '$core row missing',
      );
    }
    for (final core in _autoCores) {
      final box = tester.widget<Checkbox>(
        find.byKey(ValueKey('update-core-$core')),
      );
      expect(box.onChanged, isNotNull, reason: '$core must be selectable');
    }

    // Manual rows carry the manual note in data and render the manual label.
    for (final core in _manualCores) {
      final target = targets.firstWhere((target) => target.core == core);
      expect(target.supported, isFalse, reason: '$core has no in-app download');
      expect(target.note, 'error.update_manual');
    }
    await tester.drag(
      find.byKey(const ValueKey('update-target-list')),
      const Offset(0, -800),
    );
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('update-core-mieru')),
      findsOneWidget,
      reason: 'the tail of the matrix must be reachable',
    );
    expect(find.text('需手动安装'), findsWidgets);
    expect(find.text('不支持更新'), findsNothing);
  });

  testWidgets('missing xray+mihomo install via normal entry then clear', (
    tester,
  ) async {
    final bridge = _InstallBridge();
    final container = _makeContainer(bridge);
    container.read(updateControllerProvider.notifier).seedMissingCores(
      const <String>['xray', 'mihomo'],
    );
    await _pump(tester, container);

    expect(find.byKey(const ValueKey('update-missing-cores')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('update-install-missing-btn')));
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.any(
        (call) => call.startsWith('check_updates:xray,mihomo'),
      ),
      isTrue,
    );
    expect(
      bridge.t16Calls.any((call) => call.startsWith('apply_core:xray,mihomo')),
      isTrue,
    );
    expect(find.textContaining('已安装'), findsOneWidget);
    // The same GUI can retry launching: the missing banner is cleared.
    expect(find.byKey(const ValueKey('update-missing-cores')), findsNothing);
  });

  testWidgets('failed install is readable and retryable, then succeeds', (
    tester,
  ) async {
    final bridge = _InstallBridge(failInstall: true);
    final container = _makeContainer(bridge);
    container.read(updateControllerProvider.notifier).seedMissingCores(
      const <String>['xray'],
    );
    await _pump(tester, container);

    // The pipeline reports the failure verbatim and keeps the entry so the
    // user can retry.
    await tester.tap(find.byKey(const ValueKey('update-install-missing-btn')));
    await tester.pumpAndSettle();
    expect(find.textContaining('内核安装失败'), findsOneWidget);
    expect(find.textContaining('E_UNAVAILABLE'), findsOneWidget);
    expect(find.byKey(const ValueKey('update-missing-cores')), findsOneWidget);

    // Repaired: the retry succeeds and clears the banner.
    bridge.failInstall = false;
    await tester.tap(find.byKey(const ValueKey('update-install-missing-btn')));
    await tester.pumpAndSettle();
    expect(find.textContaining('已安装'), findsOneWidget);
    expect(find.byKey(const ValueKey('update-missing-cores')), findsNothing);
  });
}
