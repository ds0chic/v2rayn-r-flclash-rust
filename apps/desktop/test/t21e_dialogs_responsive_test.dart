import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/theme_setting_dialog.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

/// T21-E: the settings/backup/update/routing/DNS/subscription/theme windows
/// must not overflow at the 1000x700 supported minimum.
///
/// A single `pumpWidget` drives every window through a `ValueListenableBuilder`
/// so the locked flutter_tester never rebuilds the whole app per window.
void main() {
  final windows = <(String, Widget Function())>[
    ('设置', OptionSettingWindow.new),
    ('主题', ThemeSettingDialog.new),
    ('热键', GlobalHotkeyWindow.new),
    ('备份', BackupAndRestoreView.new),
    ('更新', CheckUpdateView.new),
    ('路由', RoutingSettingWindow.new),
    ('DNS', DnsSettingWindow.new),
    ('订阅', SubSettingWindow.new),
  ];

  testWidgets('windows render without overflow at 1000x700', (tester) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(const Size(1000, 700));

    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort(count: 20)),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      ],
    );
    addTearDown(container.dispose);

    final index = ValueNotifier<int>(0);
    addTearDown(index.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: ValueListenableBuilder<int>(
          valueListenable: index,
          builder: (context, i, _) =>
              MaterialApp(home: Scaffold(body: windows[i].$2())),
        ),
      ),
    );
    await tester.pump();

    for (var i = 0; i < windows.length; i++) {
      index.value = i;
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 40));
      expect(
        tester.takeException(),
        isNull,
        reason: 'overflow in ${windows[i].$1} window at 1000x700',
      );
    }
  });
}
