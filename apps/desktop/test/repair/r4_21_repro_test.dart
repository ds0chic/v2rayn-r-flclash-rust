// R4-21 repro (fails on the pre-fix window, green after the fix).
//
// Before the fix the ordinary update window conflated the 11 frozen cores that
// have no in-app download asset with genuinely unsupported targets: every
// non-auto row rendered "不支持更新", so a user could not tell a manually
// installed core from a blocked one. The runtime adapter authority
// (`CoreType::PROXY_CORES`) has 14 cores, all selectable once present.
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

/// The three cores with a frozen in-app download asset (upstream
/// `CoreInfoManager.DownloadUrl*`).
const autoCores = <String>['xray', 'sing_box', 'mihomo'];

/// The remaining frozen proxy cores: installable by hand, but the update window
/// must still list them and label them "需手动安装".
const manualCores = <String>[
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

const proxyCores = <String>[...autoCores, ...manualCores];

/// A T16 double that returns the full production matrix (14 proxy cores + the
/// application row) exactly as `application::builtin_targets(false)` now does.
class _MatrixBridge extends SyntheticBridgePort {
  @override
  List<c.UpdateTargetDto> t16UpdateTargets() => <c.UpdateTargetDto>[
    const c.UpdateTargetDto(
      core: 'v2rayN',
      repo: '2dust/v2rayN',
      supported: true,
      prereleaseCapable: true,
    ),
    for (final core in autoCores)
      c.UpdateTargetDto(
        core: core,
        repo: 'repo/$core',
        supported: true,
        prereleaseCapable: core == 'xray',
      ),
    for (final core in manualCores)
      c.UpdateTargetDto(
        core: core,
        repo: 'repo/$core',
        supported: false,
        prereleaseCapable: false,
        note: 'error.update_manual',
      ),
  ];
}

void main() {
  testWidgets(
    'every frozen proxy core is listed and manual cores read as manual',
    (tester) async {
      final bridge = _MatrixBridge();
      final container = ProviderContainer(
        overrides: [bridgePortProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
        ),
      );
      await tester.pump();

      // The data contract: every frozen proxy core is present and the 11
      // non-auto ones are labelled manual, never omitted or mislabelled.
      final targets = container.read(updateControllerProvider).targets;
      final proxy = targets
          .where((target) => proxyCores.contains(target.core))
          .toList();
      expect(proxy.length, 14, reason: '14 frozen proxy cores expected');
      expect(
        proxy.where((target) => target.supported).map((t) => t.core).toSet(),
        autoCores.toSet(),
      );
      for (final target in proxy.where((target) => !target.supported)) {
        expect(
          target.note,
          'error.update_manual',
          reason: '${target.core} must be manual, not silently dropped',
        );
      }

      // The rendered contract: after scrolling to the manual rows they show the
      // manual install label (pre-fix every one rendered "不支持更新").
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
    },
  );
}
