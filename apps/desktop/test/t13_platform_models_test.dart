import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';

import 'support/fake_platform_bridge.dart';

void main() {
  group('tray menu model', () {
    test(
      'follows the upstream ContextMenu order and marks the active mode',
      () {
        final items = trayMenuModel(
          currentMode: SysProxyMode.forcedChange,
          pacVisible: true,
        );
        expect(items.first.actionId, 'ACT-TRAY-002');
        final ids = items.map((i) => i.actionId).toList();
        expect(ids, <String>[
          'ACT-TRAY-002',
          'ACT-TRAY-003',
          'ACT-TRAY-004',
          'ACT-TRAY-005',
          'ACT-TRAY-006',
          'ACT-TRAY-007',
          'ACT-TRAY-008',
          'ACT-TRAY-009',
          'ACT-TRAY-010',
          'ACT-TRAY-011',
          'ACT-TRAY-012',
          'ACT-TRAY-013',
        ]);
        final checked = items.where((i) => i.checked).toList();
        expect(checked.length, 1);
        expect(checked.single.radioGroup, SysProxyMode.forcedChange);
        // Separator placement matches the XAML.
        expect(items[4].separatorBefore, isTrue); // routing
        expect(items[6].separatorBefore, isTrue); // clipboard import
        expect(items[10].separatorBefore, isTrue); // copy proxy cmd
        expect(items[11].separatorBefore, isTrue); // exit
      },
    );

    test('PAC entry is Windows-only', () {
      final without = trayMenuModel(
        currentMode: SysProxyMode.unchanged,
        pacVisible: false,
      );
      expect(without.any((i) => i.actionId == 'ACT-TRAY-005'), isFalse);
      expect(without.length, 11);
    });

    test('node submenu hides above TrayMenuServersLimit', () {
      expect(trayServersVisible(nodeCount: 20, limit: 20), isTrue);
      expect(trayServersVisible(nodeCount: 21, limit: 20), isFalse);
      final hidden = trayNodeEntries(
        nodes: <TrayNodeEntry>[
          for (var i = 0; i < 25; i++)
            TrayNodeEntry(id: '$i', label: 'node $i'),
        ],
        limit: 20,
      );
      expect(hidden, isEmpty);
      final shown = trayNodeEntries(
        nodes: <TrayNodeEntry>[
          for (var i = 0; i < 10; i++)
            TrayNodeEntry(id: '$i', label: 'node $i'),
        ],
        limit: 20,
      );
      expect(shown.length, 10);
    });
  });

  group('proxy settings view', () {
    test('parses SystemProxyItem into a typed view', () {
      final view = ProxySettingsView.fromDocument(<String, dynamic>{
        'SystemProxyItem': <String, dynamic>{
          'SysProxyType': 1,
          'SystemProxyExceptions': 'localhost;127.*',
          'NotProxyLocalAddress': true,
          'SystemProxyAdvancedProtocol': null,
        },
      });
      expect(view.mode, SysProxyMode.forcedChange);
      expect(view.exceptions, 'localhost;127.*');
      expect(view.notProxyLocalAddress, isTrue);
    });

    test('builds server with and without the advanced template', () {
      expect(buildProxyServer(port: 10809), '127.0.0.1:10809');
      expect(
        buildProxyServer(
          port: 10809,
          advancedProtocol: 'http={ip}:{http_port};socks={ip}:{socks_port}',
        ),
        'http=127.0.0.1:10809;socks=127.0.0.1:10809',
      );
    });

    test('builds bypass adding <local>', () {
      expect(
        buildProxyBypass(exceptions: 'a; b ;c', notProxyLocalAddress: true),
        '<local>;a;b;c',
      );
      expect(
        buildProxyBypass(exceptions: '', notProxyLocalAddress: true),
        '<local>',
      );
      expect(
        buildProxyBypass(exceptions: 'a;b', notProxyLocalAddress: false),
        'a;b',
      );
    });
  });

  group('platform controller', () {
    test('apply records the mode and reads state back from the bridge', () {
      final bridge = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
      final container = ProviderContainer(
        overrides: [platformBridgeProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      final controller = container.read(platformControllerProvider.notifier);

      final result = controller.apply(
        mode: SysProxyMode.forcedChange,
        server: '127.0.0.1:10809',
        bypass: '<local>',
      );
      expect(result.ok, isTrue);
      expect(bridge.appliedModes, <SysProxyMode>[SysProxyMode.forcedChange]);
      final view = container.read(platformControllerProvider);
      expect(view.desiredMode, SysProxyMode.forcedChange);
      expect(view.enabled, isTrue);
      expect(view.server, '127.0.0.1:10809');
    });

    test('failed apply keeps the error and does not fake success', () {
      final bridge = FakePlatformBridge()..failNextApply = true;
      final container = ProviderContainer(
        overrides: [platformBridgeProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      final controller = container.read(platformControllerProvider.notifier);

      final result = controller.apply(mode: SysProxyMode.forcedChange);
      expect(result.ok, isFalse);
      final view = container.read(platformControllerProvider);
      expect(view.error?.code, 'E_PLATFORM_BACKEND');
      expect(view.message, isNotNull);
      expect(bridge.appliedModes, isEmpty);
    });

    test(
      'restore reports user conflicts without pretending a clean restore',
      () {
        final bridge = FakePlatformBridge()..restoreConflict = true;
        final container = ProviderContainer(
          overrides: [platformBridgeProvider.overrideWithValue(bridge)],
        );
        addTearDown(container.dispose);
        final controller = container.read(platformControllerProvider.notifier);

        controller.refresh(SysProxyMode.forcedChange);
        final result = controller.restore();
        expect(result.ok, isTrue);
        expect(result.clean, isFalse);
        expect(result.conflicts.single.field, 'Server');
        final view = container.read(platformControllerProvider);
        expect(view.message, contains('用户已修改字段保留'));
      },
    );

    test('desired mode is read from the settings document', () {
      final container = ProviderContainer(
        overrides: [
          platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(platformControllerProvider.notifier);
      final mode = controller.desiredModeFromSettings(<String, dynamic>{
        'SystemProxyItem': <String, dynamic>{'SysProxyType': 3},
      });
      expect(mode, SysProxyMode.pac);
      expect(
        controller.desiredModeFromSettings(<String, dynamic>{}),
        SysProxyMode.unchanged,
      );
    });
  });
}
