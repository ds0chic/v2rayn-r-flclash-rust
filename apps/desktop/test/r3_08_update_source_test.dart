// R3-08: the check list must render the application target as explicitly
// blocked when this build has no own release source — never as "up to date"
// with the upstream v2rayN version.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';

class _BlockedAppBridge extends SyntheticBridgePort {
  @override
  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async {
    return const c.UpdateReportDto(
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
  }
}

void main() {
  testWidgets('blocked app source is shown instead of a fake up-to-date row', (
    tester,
  ) async {
    final container = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(_BlockedAppBridge())],
    );
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
      ),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('update-check-only-btn')));
    await tester.pumpAndSettle();

    expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);
    expect(find.textContaining('已是最新'), findsNothing);
  });
}
