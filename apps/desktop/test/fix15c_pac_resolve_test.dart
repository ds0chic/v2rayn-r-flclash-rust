import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';

import 'support/fake_platform_bridge.dart';

/// FIX-15C: PAC script source resolution (`data/pac.txt` / custom script) and
/// the seed-when-missing behavior, mirroring upstream `PacManager.InitText`.
void main() {
  late Directory tempDir;

  setUp(() {
    tempDir = Directory.systemTemp.createTempSync('fix15c_pac_');
  });

  tearDown(() {
    if (tempDir.existsSync()) tempDir.deleteSync(recursive: true);
  });

  group('PAC file selection (upstream PacManager.InitText)', () {
    test('custom path wins when configured', () {
      final selection = selectPacFile(
        customPacPath: 'C:/custom/my.pac',
        configDir: tempDir.path,
      );
      expect(selection.isCustom, isTrue);
      expect(selection.path, 'C:/custom/my.pac');
    });

    test('empty/missing custom path falls back to configDir/pac.txt', () {
      for (final custom in <String?>[null, '', '   ']) {
        final selection = selectPacFile(
          customPacPath: custom,
          configDir: tempDir.path,
        );
        expect(selection.isCustom, isFalse);
        expect(selection.path, endsWith('pac.txt'));
        expect(selection.path, startsWith(tempDir.path));
      }
    });

    test('bundled default template carries the __PROXY__ placeholder', () {
      expect(defaultPacScriptTemplate, contains('__PROXY__'));
    });
  });

  group('PlatformController.startPacFromConfig', () {
    ProviderContainer containerWith(FakePlatformBridge bridge) {
      final container = ProviderContainer(
        overrides: [platformBridgeProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      return container;
    }

    test('seeds pac.txt from the default template when missing', () {
      final bridge = FakePlatformBridge();
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);

      final handle = controller.startPacFromConfig(
        configDir: tempDir.path,
        proxyRule: 'PROXY 127.0.0.1:10809;DIRECT;',
      );

      expect(handle.ok, isTrue);
      final pacFile = File('${tempDir.path}${Platform.pathSeparator}pac.txt');
      expect(pacFile.existsSync(), isTrue, reason: 'default pac.txt seeded');
      expect(pacFile.readAsStringSync(), defaultPacScriptTemplate);
      expect(bridge.pacFilePaths, hasLength(1));
      expect(bridge.pacFilePaths.single, pacFile.path);
      expect(bridge.lastPacFileText, defaultPacScriptTemplate);
      final view = container.read(platformControllerProvider);
      expect(view.pacRunning, isTrue);
      expect(view.pacPort, 11808);
    });

    test('reads an existing pac.txt instead of reseeding', () {
      final bridge = FakePlatformBridge();
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);

      final pacFile = File('${tempDir.path}${Platform.pathSeparator}pac.txt');
      pacFile.writeAsStringSync('var proxy = "__PROXY__"; // user file');

      controller.startPacFromConfig(configDir: tempDir.path);

      expect(
        pacFile.readAsStringSync(),
        'var proxy = "__PROXY__"; // user file',
      );
      expect(bridge.lastPacFileText, 'var proxy = "__PROXY__"; // user file');
    });

    test('uses the configured custom PAC path verbatim', () {
      final bridge = FakePlatformBridge();
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);

      final custom = File('${tempDir.path}${Platform.pathSeparator}custom.pac');
      custom.writeAsStringSync(
        'function FindProxyForURL(){return "__PROXY__";}',
      );

      controller.startPacFromConfig(
        configDir: tempDir.path,
        customPacPath: custom.path,
      );

      expect(bridge.pacFilePaths.single, custom.path);
      expect(bridge.lastPacFileText, custom.readAsStringSync());
    });

    test('surfaces a structured error when the file cannot be read', () {
      final bridge = FakePlatformBridge();
      final container = containerWith(bridge);
      final controller = container.read(platformControllerProvider.notifier);

      // A directory where a file is expected cannot be read as a PAC script.
      final dirAsFile = Directory(
        '${tempDir.path}${Platform.pathSeparator}pac.txt',
      )..createSync();
      expect(dirAsFile.existsSync(), isTrue);

      final handle = controller.startPacFromConfig(configDir: tempDir.path);

      expect(handle.ok, isFalse);
      expect(bridge.pacStartCount, 0, reason: 'no backend call on read fail');
      final view = container.read(platformControllerProvider);
      expect(view.pacRunning, isFalse);
    });
  });

  group('mode persistence boundary', () {
    test('PAC mode write preserves sibling SystemProxyItem fields', () {
      final updated = systemProxyItemWithMode(<String, dynamic>{
        'SystemProxyItem': <String, dynamic>{
          'SysProxyType': 2,
          'SystemProxyExceptions': 'localhost',
          'CustomSystemProxyPacPath': 'C:/pac.txt',
        },
      }, SysProxyMode.pac);
      expect(updated['SysProxyType'], 3);
      expect(updated['SystemProxyExceptions'], 'localhost');
      expect(updated['CustomSystemProxyPacPath'], 'C:/pac.txt');
    });
  });
}
