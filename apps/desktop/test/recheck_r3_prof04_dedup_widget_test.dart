// R3-PROF-04 widget regression: dedup runs through the real menu, re-picks a
// live node when the active duplicate is deleted, and surfaces the real delete
// error instead of "没有重复节点".
import 'dart:convert';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/profiles_harness.dart';
import 'support/synthetic_runtime_bridge.dart';

c.ProfileDto _vless(String id, String address) => c.ProfileDto(
  indexId: id,
  configType: ConfigType.vless,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: '',
  isSub: true,
  displayLog: true,
  remarks: id,
  address: address,
  port: 443,
  password: '',
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

void main() {
  testWidgets('dedup re-picks the active node and reports delete failure', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    bridge.saveImportedProfile(_vless('old', '192.0.2.60'), 0);
    bridge.saveImportedProfile(_vless('new', '192.0.2.60'), 1);
    bridge.saveImportedProfile(_vless('fallback', '192.0.2.61'), 2);
    bridge.setActiveProfile('old');
    // Explicit: newer-wins dedup (`KeepOlderDedupl = false`).
    bridge.saveSettingsJson(
      jsonEncode(<String, Object>{
        'GuiItem': <String, Object>{'KeepOlderDedupl': false},
      }),
      bridge.getSettings().revision.toInt(),
    );

    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
        runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
      ],
    );

    final resolved = await pumpApp(tester, container: container);
    final controller = resolved.read(profilesControllerProvider.notifier);
    controller.reload();

    // First dedup removes the active duplicate and re-picks a live node.
    await tester.tapAt(
      tester.getCenter(find.byKey(const ValueKey('cell-syn-000000-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('ctx-移除重复')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('dedup-confirm-ok')));
    await tester.pumpAndSettle();

    expect(
      bridge.queryAllProfiles().map((p) => p.indexId).contains('old'),
      isFalse,
      reason: 'the older duplicate is removed',
    );
    expect(
      bridge.getActiveProfile(),
      'new',
      reason: 'the active node falls back to a live node',
    );

    // A real delete failure must be surfaced, not reported as "no duplicates".
    bridge.failDeleteProfiles = true;
    bridge.saveImportedProfile(
      _vless('o2', '192.0.2.62'),
      bridge.profileRevision(),
    );
    bridge.saveImportedProfile(
      _vless('n2', '192.0.2.62'),
      bridge.profileRevision(),
    );
    controller.reload();

    await tester.tapAt(
      tester.getCenter(find.byKey(const ValueKey('cell-syn-000000-Remarks'))),
      buttons: kSecondaryButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('ctx-移除重复')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('dedup-confirm-ok')));
    await tester.pumpAndSettle();

    final message = resolved.read(uiShellControllerProvider).message ?? '';
    expect(message, contains('移除重复失败'));
    expect(message, contains('E_DELETE_FAILED'));
  });
}
