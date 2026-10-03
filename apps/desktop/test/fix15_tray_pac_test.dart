import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';

import 'support/fake_platform_bridge.dart';

void main() {
  group('tray shared use cases and dynamic sync', () {
    test('tray leaf actions map to the main window command ids', () {
      expect(sharedCommandForTrayAction('ACT-TRAY-008'), 'ACT-MAIN-016');
      expect(sharedCommandForTrayAction('ACT-TRAY-009'), 'ACT-MAIN-017');
      expect(sharedCommandForTrayAction('ACT-TRAY-010'), 'ACT-MAIN-020');
      expect(sharedCommandForTrayAction('ACT-TRAY-011'), 'ACT-MAIN-021');
      expect(sharedCommandForTrayAction('ACT-TRAY-013'), isNull);
    });

    test('routing/nodes submenus carry live entries, respecting the limit', () {
      final items = trayMenuModel(
        currentMode: SysProxyMode.unchanged,
        pacVisible: true,
        routings: const <TraySubEntry>[
          TraySubEntry(id: 'r1', label: '默认', checked: true),
          TraySubEntry(id: 'r2', label: '绕过局域网'),
        ],
        nodes: const <TraySubEntry>[
          TraySubEntry(id: 'n1', label: 'HK-1'),
          TraySubEntry(id: 'n2', label: 'US-1'),
        ],
        serversLimit: 10,
      );
      final routing = items.firstWhere((i) => i.actionId == 'ACT-TRAY-006');
      final servers = items.firstWhere((i) => i.actionId == 'ACT-TRAY-007');
      expect(routing.children.map((c) => c.id), <String>['r1', 'r2']);
      expect(routing.children.first.checked, isTrue);
      expect(servers.children.map((c) => c.id), <String>['n1', 'n2']);
    });

    test('nodes submenu hides above the configured limit', () {
      final items = trayMenuModel(
        currentMode: SysProxyMode.unchanged,
        pacVisible: false,
        nodes: <TraySubEntry>[
          for (var i = 0; i < 21; i++) TraySubEntry(id: '$i', label: 'n$i'),
        ],
        serversLimit: 20,
      );
      final servers = items.firstWhere((i) => i.actionId == 'ACT-TRAY-007');
      expect(servers.children, isEmpty);
    });

    test('TrayAction exposes the shared command for a menu item', () {
      final action = TrayAction.fromMenuItem(
        const TrayMenuItem(actionId: 'ACT-TRAY-009', label: '扫描屏幕上的二维码'),
      );
      expect(action.sharedCommand, 'ACT-MAIN-017');
      final node = TrayAction.selectNode('n1');
      expect(node.actionId, 'ACT-TRAY-007');
      expect(node.nodeId, 'n1');
    });
  });

  group('system proxy mode persistence', () {
    test('mode toggle preserves the other SystemProxyItem fields', () {
      final updated = systemProxyItemWithMode(<String, dynamic>{
        'SystemProxyItem': <String, dynamic>{
          'SysProxyType': 2,
          'SystemProxyExceptions': 'localhost',
          'NotProxyLocalAddress': true,
          'CustomSystemProxyPacPath': 'C:/pac.txt',
        },
      }, SysProxyMode.pac);
      expect(updated['SysProxyType'], 3);
      expect(updated['SystemProxyExceptions'], 'localhost');
      expect(updated['NotProxyLocalAddress'], isTrue);
      expect(updated['CustomSystemProxyPacPath'], 'C:/pac.txt');
    });

    test('missing group yields just the mode', () {
      final updated = systemProxyItemWithMode(
        <String, dynamic>{},
        SysProxyMode.forcedClear,
      );
      expect(updated, <String, dynamic>{'SysProxyType': 0});
    });
  });

  group('platform controller PAC lifecycle', () {
    ProviderContainer containerWith(FakePlatformBridge bridge) {
      final container = ProviderContainer(
        overrides: [platformBridgeProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      return container;
    }

    test('startPac publishes the real handle and stopPac clears it', () {
      final bridge = FakePlatformBridge();
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);

      final handle = controller.startPac(
        pacText: 'function FindProxyForURL(){}',
      );
      expect(handle.ok, isTrue);
      expect(bridge.pacStartCount, 1);
      var view = container.read(platformControllerProvider);
      expect(view.pacRunning, isTrue);
      expect(view.pacPort, 11808);
      expect(view.pacUrl, contains('11808'));

      controller.stopPac();
      expect(bridge.pacStopCount, 1);
      view = container.read(platformControllerProvider);
      expect(view.pacRunning, isFalse);
      expect(view.pacUrl, isNull);
    });

    test('pac failure surfaces the error without faking success', () {
      final bridge = FakePlatformBridge()..failNextPac = true;
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);
      final handle = controller.startPac(pacText: 'x');
      expect(handle.ok, isFalse);
      final view = container.read(platformControllerProvider);
      expect(view.error?.code, 'E_PAC_START');
    });
  });
}
