// R4-24 contract: proxy PAC + applied-endpoint coordination.
//
// Covers the card's must-pass behaviors without any host side effect:
//  1. after an apply the selected mode is re-pointed at the actual applied
//     endpoint (protocol/port), with an explicit no-session prompt otherwise;
//  2. exit restore keeps per-field ownership and preserves external edits;
//  3. PAC renderer directives, custom PAC file path/content, mode-switch commit
//     boundary, and visible failure (no fake success).
//
// Synthetic data only; ports are >= 11808 and never 10808. No native library,
// kernel, network or host-proxy write runs here (fake bridge / fake runtime).
import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/fake_platform_bridge.dart';

class _RuntimeStub implements RuntimeBridge {
  _RuntimeStub(this._view);
  RuntimeView _view;

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  String? activeProfileId() => 'synthetic-active';

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async {
    _view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();
}

class _DocSettingsController extends SettingsController {
  _DocSettingsController(this._doc);
  final Map<String, dynamic> _doc;

  @override
  SettingsViewState build() => SettingsViewState(loaded: true, document: _doc);
}

const RuntimeView _running11808 = RuntimeView(
  state: 'Running',
  hostAlive: true,
  ports: <int>[11808],
  sessionId: 'session-1',
);

Map<String, dynamic> _doc({
  int mode = 1,
  int port = 11808,
  int protocol = 0,
  String? customPacPath,
  String? exceptions,
}) => <String, dynamic>{
  'SystemProxyItem': <String, dynamic>{
    'SysProxyType': mode,
    'CustomSystemProxyPacPath': ?customPacPath,
    'SystemProxyExceptions': ?exceptions,
  },
  'Inbound': <Map<String, dynamic>>[
    <String, dynamic>{'LocalPort': port, 'Protocol': protocol},
  ],
};

ProviderContainer _container({
  required FakePlatformBridge platform,
  required RuntimeBridge runtime,
  required Map<String, dynamic> document,
}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      platformBridgeProvider.overrideWithValue(platform),
      runtimeBridgeProvider.overrideWithValue(runtime),
      settingsControllerProvider.overrideWith(
        () => _DocSettingsController(document),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('R4-24 PAC renderer', () {
    test('HTTP endpoint renders the upstream PROXY ...;DIRECT;', () {
      expect(buildPacProxyRule(port: 11808), 'PROXY 127.0.0.1:11808;DIRECT;');
      expect(
        buildPacProxyRule(port: 11808, protocol: ProxyProtocolKind.mixed),
        'PROXY 127.0.0.1:11808;DIRECT;',
      );
    });

    test('SOCKS endpoint renders SOCKS5, never a bare HTTP rule', () {
      expect(
        buildPacProxyRule(port: 11808, protocol: ProxyProtocolKind.socks),
        'SOCKS5 127.0.0.1:11808;DIRECT;',
      );
    });
  });

  group('R4-24 applied endpoint reconciliation', () {
    test(
      'a new applied session re-points the mode at the applied port',
      () async {
        final platform = FakePlatformBridge();
        final runtime = _RuntimeStub(
          const RuntimeView(
            state: 'Running',
            hostAlive: true,
            ports: <int>[11809],
            sessionId: 'session-2',
          ),
        );
        final container = _container(
          platform: platform,
          runtime: runtime,
          document: _doc(port: 11809, protocol: 0),
        );
        container.read(platformControllerProvider.notifier);
        await container.read(runtimeControllerProvider.notifier).refresh();

        expect(platform.appliedModes, contains(SysProxyMode.forcedChange));
        expect(platform.lastServer, 'socks=127.0.0.1:11809');
      },
    );

    test(
      'protocol is only trusted when its port matches the applied port',
      () async {
        final platform = FakePlatformBridge();
        // Persisted inbound is SOCKS on 11809, but the session published 11810;
        // the endpoint must not be mislabeled as SOCKS.
        final runtime = _RuntimeStub(
          const RuntimeView(
            state: 'Running',
            hostAlive: true,
            ports: <int>[11810],
            sessionId: 'session-3',
          ),
        );
        final container = _container(
          platform: platform,
          runtime: runtime,
          document: _doc(port: 11809, protocol: 0),
        );
        await container.read(runtimeControllerProvider.notifier).refresh();
        container
            .read(platformControllerProvider.notifier)
            .applyModeFromConfig(SysProxyMode.forcedChange);

        expect(platform.lastServer, '127.0.0.1:11810');
      },
    );

    test('no running session reports honestly and applies nothing', () {
      final platform = FakePlatformBridge();
      final container = _container(
        platform: platform,
        runtime: _RuntimeStub(const RuntimeView()),
        document: _doc(),
      );
      final controller = container.read(platformControllerProvider.notifier);
      controller.applyModeFromConfig(SysProxyMode.forcedChange);
      expect(platform.appliedModes, isEmpty);
      expect(
        container.read(platformControllerProvider).error?.code,
        'E_NO_RUNNING_SESSION',
      );
    });

    test('unchanged mode is a no-op on reconciliation', () async {
      final platform = FakePlatformBridge();
      final runtime = _RuntimeStub(_running11808);
      final container = _container(
        platform: platform,
        runtime: runtime,
        document: _doc(mode: 2),
      );
      container.read(platformControllerProvider.notifier);
      await container.read(runtimeControllerProvider.notifier).refresh();
      expect(platform.appliedModes, isEmpty);
    });
  });

  group('R4-24 PAC file selection and failure visibility', () {
    late Directory tempDir;
    setUp(() => tempDir = Directory.systemTemp.createTempSync('r4_24_pac_'));
    tearDown(() {
      if (tempDir.existsSync()) tempDir.deleteSync(recursive: true);
    });

    test('custom PAC path wins and its content is served verbatim', () {
      final platform = FakePlatformBridge();
      final custom = File('${tempDir.path}${Platform.pathSeparator}my.pac')
        ..writeAsStringSync('function FindProxyForURL(){return "__PROXY__";}');
      final container = _container(
        platform: platform,
        runtime: _RuntimeStub(_running11808),
        document: _doc(mode: 3, customPacPath: custom.path),
      );
      final controller = container.read(platformControllerProvider.notifier);
      controller.startPacFromConfig(
        configDir: tempDir.path,
        customPacPath: custom.path,
        proxyRule: buildPacProxyRule(port: 11808),
      );
      expect(platform.pacFilePaths.single, custom.path);
      expect(platform.lastPacFileText, custom.readAsStringSync());
    });

    test(
      'PAC failure keeps the previous state and surfaces the error',
      () async {
        final platform = FakePlatformBridge()..failNextPac = true;
        final container = _container(
          platform: platform,
          runtime: _RuntimeStub(_running11808),
          document: _doc(mode: 3),
        );
        await container.read(runtimeControllerProvider.notifier).refresh();
        final controller = container.read(platformControllerProvider.notifier);
        final result = controller.applyModeFromConfig(
          SysProxyMode.pac,
          configDir: tempDir.path,
        );
        expect(result.ok, isFalse);
        final view = container.read(platformControllerProvider);
        expect(view.error?.code, 'E_PAC_START');
        expect(view.pacRunning, isFalse);
      },
    );
  });

  group('R4-24 exit restore ownership', () {
    test('external edits are preserved and reported, not overwritten', () {
      final platform = FakePlatformBridge()..restoreConflict = true;
      final container = _container(
        platform: platform,
        runtime: _RuntimeStub(_running11808),
        document: _doc(),
      );
      final controller = container.read(platformControllerProvider.notifier);
      final result = controller.restoreOnExit(SysProxyMode.forcedChange);
      expect(result.ok, isTrue);
      expect(result.clean, isFalse);
      expect(result.conflicts.single.field, 'Server');
      expect(container.read(platformControllerProvider).conflicts, isNotEmpty);
    });

    test('a clean restore reports the restored message', () {
      final platform = FakePlatformBridge();
      final container = _container(
        platform: platform,
        runtime: _RuntimeStub(_running11808),
        document: _doc(),
      );
      final controller = container.read(platformControllerProvider.notifier);
      final result = controller.restoreOnExit(SysProxyMode.forcedChange);
      expect(result.ok, isTrue);
      expect(result.clean, isTrue);
      expect(container.read(platformControllerProvider).message, '系统代理已恢复');
    });
  });
}
