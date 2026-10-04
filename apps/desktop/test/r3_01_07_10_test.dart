import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_platform_bridge.dart';

const RuntimeView _runningView = RuntimeView(
  state: 'Running',
  hostAlive: true,
  ports: <int>[11808],
  sessionId: 'synthetic',
);

Map<String, dynamic> _document({
  List<Map<String, dynamic>>? inbound,
  String? advancedProtocol,
  int mode = 3,
}) => <String, dynamic>{
  'SystemProxyItem': <String, dynamic>{
    'SysProxyType': mode,
    'SystemProxyAdvancedProtocol': ?advancedProtocol,
  },
  'Inbound': ?inbound,
};

ProviderContainer _container({
  required FakePlatformBridge platform,
  required RuntimeBridge runtime,
}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      platformBridgeProvider.overrideWithValue(platform),
      runtimeBridgeProvider.overrideWithValue(runtime),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('R3-01 PAC renderer', () {
    test('HTTP/mixed endpoint renders the upstream PROXY ...;DIRECT;', () {
      expect(buildPacProxyRule(port: 11808), 'PROXY 127.0.0.1:11808;DIRECT;');
      expect(
        buildPacProxyRule(port: 11808, protocol: ProxyProtocolKind.mixed),
        'PROXY 127.0.0.1:11808;DIRECT;',
      );
    });

    test('SOCKS endpoint renders a SOCKS5 directive, not a bare address', () {
      expect(
        buildPacProxyRule(port: 11808, protocol: ProxyProtocolKind.socks),
        'SOCKS5 127.0.0.1:11808;DIRECT;',
      );
    });

    test('applyModeFromConfig(Pac) substitutes a full legal directive into pac.txt', () async {
      final platform = FakePlatformBridge(initialMode: SysProxyMode.unchanged);
      final runtime = CountingRuntimeBridge(initial: _runningView);
      final container = _container(platform: platform, runtime: runtime);
      await container.read(runtimeControllerProvider.notifier).refresh();

      container
          .read(platformControllerProvider.notifier)
          .applyModeFromConfig(
            SysProxyMode.pac,
            document: _document(
              inbound: <Map<String, dynamic>>[
                <String, dynamic>{'LocalPort': 11808, 'Protocol': 0},
              ],
            ),
          );

      expect(platform.pacStartCount, 1);
      expect(platform.lastPacProxyRule, 'SOCKS5 127.0.0.1:11808;DIRECT;');
    });
  });

  group('R3-07 system proxy / PAC protocol distinction', () {
    test('primaryLocalProxyInbound prefers the SOCKS inbound', () {
      final inbound = primaryLocalProxyInbound(
        _document(
          inbound: <Map<String, dynamic>>[
            <String, dynamic>{'LocalPort': 11809, 'Protocol': 6},
            <String, dynamic>{'LocalPort': 11810, 'Protocol': 0},
          ],
        ),
      );
      expect(inbound, isNotNull);
      expect(inbound!.port, 11810);
      expect(inbound.protocol, ProxyProtocolKind.socks);
    });

    test(
      'SOCKS endpoint drives socks= for ForcedChange and SOCKS5 for PAC',
      () async {
        final platform = FakePlatformBridge(
          initialMode: SysProxyMode.unchanged,
        );
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = _container(platform: platform, runtime: runtime);
        await container.read(runtimeControllerProvider.notifier).refresh();
        final controller = container.read(platformControllerProvider.notifier);
        final doc = _document(
          inbound: <Map<String, dynamic>>[
            <String, dynamic>{'LocalPort': 11808, 'Protocol': 0},
          ],
        );

        controller.applyModeFromConfig(
          SysProxyMode.forcedChange,
          document: doc,
        );
        expect(platform.lastServer, 'socks=127.0.0.1:11808');

        controller.applyModeFromConfig(SysProxyMode.pac, document: doc);
        expect(platform.lastPacProxyRule, 'SOCKS5 127.0.0.1:11808;DIRECT;');
      },
    );

    test(
      'HTTP/mixed endpoint keeps the bare named proxy and PROXY directive',
      () async {
        final platform = FakePlatformBridge(
          initialMode: SysProxyMode.unchanged,
        );
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = _container(platform: platform, runtime: runtime);
        await container.read(runtimeControllerProvider.notifier).refresh();
        final controller = container.read(platformControllerProvider.notifier);
        final doc = _document(
          inbound: <Map<String, dynamic>>[
            <String, dynamic>{'LocalPort': 11808, 'Protocol': 6},
          ],
        );

        controller.applyModeFromConfig(
          SysProxyMode.forcedChange,
          document: doc,
        );
        expect(platform.lastServer, '127.0.0.1:11808');

        controller.applyModeFromConfig(SysProxyMode.pac, document: doc);
        expect(platform.lastPacProxyRule, 'PROXY 127.0.0.1:11808;DIRECT;');
      },
    );

    test(
      'no running session reports honestly for both proxy and PAC',
      () async {
        final platform = FakePlatformBridge(
          initialMode: SysProxyMode.unchanged,
        );
        final runtime = CountingRuntimeBridge();
        final container = _container(platform: platform, runtime: runtime);
        await container.read(runtimeControllerProvider.notifier).refresh();
        final controller = container.read(platformControllerProvider.notifier);

        controller.applyModeFromConfig(SysProxyMode.pac);
        expect(platform.pacStartCount, 0);
        expect(
          container.read(platformControllerProvider).error?.code,
          'E_NO_RUNNING_SESSION',
        );

        controller.applyModeFromConfig(SysProxyMode.forcedChange);
        expect(platform.appliedModes, isEmpty);
        expect(
          container.read(platformControllerProvider).error?.code,
          'E_NO_RUNNING_SESSION',
        );
      },
    );
  });

  group('R3-10 routing reload honesty', () {
    test(
      'successful reload keeps the applied fact and reports success',
      () async {
        final bridge = SyntheticBridgePort();
        final runtime = CountingRuntimeBridge(initial: _runningView);
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(bridge),
            runtimeBridgeProvider.overrideWithValue(runtime),
          ],
        );
        addTearDown(container.dispose);
        final controller = container.read(routingControllerProvider.notifier);
        final first = container.read(routingControllerProvider).items.first;
        bridge.saveRouting(_secondRouting(first, 'r-bypass'));
        controller.reload();

        await controller.setDefaultAndReload('r-bypass');
        expect(container.read(routingControllerProvider).status, '已切换默认路由并重载');
      },
    );

    test(
      'failed reload saves the default but reports the concrete failure',
      () async {
        final bridge = SyntheticBridgePort();
        const failure = RuntimeErrorView(
          code: 'E_CONFIG_CHECK',
          messageKey: 'error.config_check_failed',
        );
        final runtime = _FailingReloadRuntimeBridge(error: failure);
        final container = ProviderContainer(
          overrides: [
            bridgePortProvider.overrideWithValue(bridge),
            runtimeBridgeProvider.overrideWithValue(runtime),
          ],
        );
        addTearDown(container.dispose);
        final controller = container.read(routingControllerProvider.notifier);
        final first = container.read(routingControllerProvider).items.first;
        bridge.saveRouting(_secondRouting(first, 'r-bypass'));
        controller.reload();

        await controller.setDefaultAndReload('r-bypass');

        final routing = container.read(routingControllerProvider);
        // The default is saved, but the old applied revision is retained.
        expect(routing.items.firstWhere((e) => e.isActive).id, 'r-bypass');
        expect(routing.status, contains('重载失败'));
        expect(routing.status, contains('E_CONFIG_CHECK'));
        final view = container.read(runtimeControllerProvider);
        expect(view.error?.code, 'E_CONFIG_CHECK');
        expect(view.appliedRevision, BigInt.from(7));
        expect(view.desiredRevision, BigInt.from(7));
      },
    );
  });
}

r.RoutingProfileDto _secondRouting(r.RoutingProfileDto first, String id) =>
    r.RoutingProfileDto(
      id: id,
      remarks: '绕过大陆',
      url: first.url,
      ruleSet: first.ruleSet,
      ruleNum: first.ruleNum,
      enabled: true,
      locked: false,
      customIcon: '',
      customRulesetPath4Singbox: '',
      domainStrategy: first.domainStrategy,
      domainStrategy4Singbox: first.domainStrategy4Singbox,
      sort: first.sort + 1,
      isActive: false,
    );

/// A runtime double whose "old applied" session stays in place: a failed
/// reload returns the structured error while the applied/desired revisions
/// (rev 7) are preserved, mirroring the recheck contract.
class _FailingReloadRuntimeBridge implements RuntimeBridge {
  _FailingReloadRuntimeBridge({required this.error});

  final RuntimeErrorView error;
  static final RuntimeView _view = RuntimeView(
    state: 'Running',
    hostAlive: true,
    ports: <int>[11808],
    sessionId: 'applied-7',
    desiredRevision: BigInt.from(7),
    appliedRevision: BigInt.from(7),
  );

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  String? activeProfileId() => 'synthetic-active';

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => RuntimeActionResult(ok: false, error: error);

  @override
  Future<RuntimeActionResult> stop() async =>
      const RuntimeActionResult(ok: true);

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();
}
