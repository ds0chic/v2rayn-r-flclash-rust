// R4-26 tray / hotkey / autostart action contract tests.
//
// Covers the plan D30 dispatch gap (ACT-TRAY-012 copy proxy command), the tray
// icon/checkmark facts (applied session + default route/mode, never desired),
// and the OS hotkey dispatch chain (same-combo grouping, pause dispatch gate,
// unregister). Synthetic only: fake registrar/plugin/probe and in-memory
// bridges. No native library, no port, no host proxy/TUN/Run key, no user data.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

/// WPF `Key.A` = 44 (upstream `KeyEventItem.KeyCode`); the codec resolves it.
const int _wpfKeyA = 44;

/// Fake registrar that records the grouped registrations and retains the
/// dispatcher so a test can fire a registered combination without the OS.
class _FakeRegistrar implements HotkeyRegistrar {
  final List<List<HotkeyRegistration>> registeredGroups =
      <List<HotkeyRegistration>>[];
  HotkeyTriggerHandler? onTriggered;
  int unregisterCount = 0;

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> registrations, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    registeredGroups.add(registrations);
    this.onTriggered = onTriggered;
    final actions = <GlobalHotkeyAction>{
      for (final r in registrations) ...r.actions,
    };
    return (actions, <String>[]);
  }

  @override
  Future<void> unregisterAll() async {
    unregisterCount++;
  }
}

/// Fake native plugin: records the grouped `HotKey`s and exposes the handler.
class _FakeHotkeyPlugin implements HotkeyPlugin {
  final List<HotKey> registered = <HotKey>[];
  void Function(HotKey)? handler;

  @override
  Future<void> unregisterAll() async {}

  @override
  Future<void> register(
    HotKey hotKey, {
    void Function(HotKey)? keyDownHandler,
  }) async {
    registered.add(hotKey);
    handler = keyDownHandler;
  }
}

class _FreeProbe implements HotkeyProbe {
  const _FreeProbe();

  @override
  HotkeyProbeResult probe(HotkeyCombo combo, HotkeyKeyCodec codec) =>
      const HotkeyProbeResult(free: true);
}

PlatformView _platformView({
  required SysProxyMode desired,
  bool enabled = false,
  String? server,
  String? autoConfigUrl,
  bool pacRunning = false,
}) => PlatformView(
  loaded: true,
  desiredMode: desired,
  enabled: enabled,
  server: server,
  autoConfigUrl: autoConfigUrl,
  pacRunning: pacRunning,
  pacUrl: autoConfigUrl,
);

void main() {
  group('ACT-TRAY-012 copy proxy command (D30 / UFS-15)', () {
    test('builds the exact upstream Windows `set` command text', () {
      final text = buildProxyCommandText(port: 11808, windows: true);
      expect(
        text,
        'set http_proxy=http://127.0.0.1:11808\n'
        'set https_proxy=http://127.0.0.1:11808\n'
        'set all_proxy=socks5://127.0.0.1:11808\n'
        '\n'
        'set HTTP_PROXY=http://127.0.0.1:11808\n'
        'set HTTPS_PROXY=http://127.0.0.1:11808\n'
        'set ALL_PROXY=socks5://127.0.0.1:11808\n',
      );
    });

    test('uses POSIX `export` syntax off Windows', () {
      final text = buildProxyCommandText(port: 11809, windows: false);
      expect(text.contains('export http_proxy=http://127.0.0.1:11809'), isTrue);
      expect(
        text.contains('export ALL_PROXY=socks5://127.0.0.1:11809'),
        isTrue,
      );
      expect(text.contains('set '), isFalse);
    });

    test('no running session is an honest failure, never a wrong port', () {
      final outcome = resolveTrayProxyCommand(
        coreRunning: false,
        appliedPorts: const <int>[11808],
        windows: true,
      );
      expect(outcome.ok, isFalse);
      expect(outcome.text, isNull);
      expect(outcome.message, contains('没有运行中'));
    });

    test('running session with no applied port is an honest failure', () {
      final outcome = resolveTrayProxyCommand(
        coreRunning: true,
        appliedPorts: const <int>[],
        windows: true,
      );
      expect(outcome.ok, isFalse);
      expect(outcome.text, isNull);
    });

    test('prefers the configured port only when it is actually applied', () {
      final outcome = resolveTrayProxyCommand(
        coreRunning: true,
        appliedPorts: const <int>[11810, 11811],
        configuredPort: 11809,
        windows: true,
      );
      // 11809 is configured but not applied: fall back to the first applied
      // port instead of copying a port the session did not publish.
      expect(outcome.ok, isTrue);
      expect(outcome.text, contains('127.0.0.1:11810'));
      expect(outcome.text, isNot(contains('11809')));

      final matched = resolveTrayProxyCommand(
        coreRunning: true,
        appliedPorts: const <int>[11809, 11810],
        configuredPort: 11809,
        windows: true,
      );
      expect(matched.text, contains('127.0.0.1:11809'));
    });

    test('tray menu still carries the copy item in upstream order', () {
      final menu = trayMenuModel(
        currentMode: SysProxyMode.unchanged,
        pacVisible: true,
      );
      final copy = menu.firstWhere((e) => e.actionId == 'ACT-TRAY-012');
      expect(copy.label, '复制代理命令到剪贴板');
      expect(copy.separatorBefore, isTrue);
    });
  });

  group('tray icon follows applied facts, not the desired mode', () {
    test('a failed apply does not show a proxy icon', () {
      final view = _platformView(
        desired: SysProxyMode.forcedChange,
        enabled: false,
      );
      expect(appliedSysProxyMode(view), isNull);

      final model = TrayReadModel(
        desiredMode: SysProxyMode.forcedChange,
        iconMode: appliedSysProxyMode(view) ?? SysProxyMode.unchanged,
        pacVisible: true,
        routings: const <TraySubEntry>[],
        nodes: const <TraySubEntry>[],
        serversLimit: 0,
        coreRunning: true,
        pacRunning: false,
      );
      expect(model.iconStatus, TrayIconStatus.coreRunning);
    });

    test('an actually applied forced proxy shows proxyActive', () {
      final view = _platformView(
        desired: SysProxyMode.forcedChange,
        enabled: true,
        server: '127.0.0.1:11809',
      );
      expect(appliedSysProxyMode(view), SysProxyMode.forcedChange);
      final model = TrayReadModel(
        desiredMode: SysProxyMode.forcedChange,
        iconMode: appliedSysProxyMode(view),
        pacVisible: true,
        routings: const <TraySubEntry>[],
        nodes: const <TraySubEntry>[],
        serversLimit: 0,
        coreRunning: true,
        pacRunning: false,
      );
      expect(model.iconStatus, TrayIconStatus.proxyActive);
    });

    test('a serving PAC server shows proxyPac', () {
      final view = _platformView(
        desired: SysProxyMode.pac,
        autoConfigUrl: 'http://127.0.0.1:11808/pac',
        pacRunning: true,
      );
      expect(appliedSysProxyMode(view), SysProxyMode.pac);
    });

    test(
      'model without an applied mode keeps the R3-09 mapping (no regress)',
      () {
        const model = TrayReadModel(
          desiredMode: SysProxyMode.forcedChange,
          pacVisible: true,
          routings: <TraySubEntry>[],
          nodes: <TraySubEntry>[],
          serversLimit: 0,
          coreRunning: true,
          pacRunning: false,
        );
        expect(model.iconStatus, TrayIconStatus.proxyActive);
      },
    );

    test('node/route checkmarks come from the default selection', () {
      final model = TrayReadModel(
        desiredMode: SysProxyMode.unchanged,
        pacVisible: true,
        routings: const <TraySubEntry>[
          TraySubEntry(id: 'r1', label: 'R1', checked: false),
          TraySubEntry(id: 'r2', label: 'R2', checked: true),
        ],
        nodes: const <TraySubEntry>[
          TraySubEntry(id: 'n1', label: 'N1', checked: true),
        ],
        serversLimit: 0,
        coreRunning: false,
        pacRunning: false,
      );
      final menu = model.buildMenu();
      final routes = menu
          .firstWhere((e) => e.actionId == 'ACT-TRAY-006')
          .children;
      final nodes = menu
          .firstWhere((e) => e.actionId == 'ACT-TRAY-007')
          .children;
      expect(routes.firstWhere((e) => e.id == 'r2').checked, isTrue);
      expect(routes.firstWhere((e) => e.id == 'r1').checked, isFalse);
      expect(nodes.firstWhere((e) => e.id == 'n1').checked, isTrue);
    });
  });

  group('tray leaf dispatch mapping', () {
    test('clipboard / scan / subscription actions map to shared commands', () {
      expect(sharedCommandForTrayAction('ACT-TRAY-008'), isNotNull);
      expect(sharedCommandForTrayAction('ACT-TRAY-009'), isNotNull);
      expect(sharedCommandForTrayAction('ACT-TRAY-010'), isNotNull);
      expect(sharedCommandForTrayAction('ACT-TRAY-011'), isNotNull);
    });

    test('proxy-mode / routing / node / copy / exit are tray-only', () {
      for (final id in const <String>[
        'ACT-TRAY-002',
        'ACT-TRAY-003',
        'ACT-TRAY-004',
        'ACT-TRAY-005',
        'ACT-TRAY-006',
        'ACT-TRAY-007',
        'ACT-TRAY-012',
        'ACT-TRAY-013',
      ]) {
        expect(sharedCommandForTrayAction(id), isNull, reason: id);
      }
    });
  });

  group('hotkey same-combo grouping + dispatch chain', () {
    final bindings = <HotkeyBinding>[
      const HotkeyBinding(
        action: GlobalHotkeyAction.showForm,
        control: true,
        keyCode: _wpfKeyA,
      ),
      const HotkeyBinding(
        action: GlobalHotkeyAction.systemProxySet,
        control: true,
        keyCode: _wpfKeyA,
      ),
      // Duplicate action in one combo is de-duplicated (upstream `Init`).
      const HotkeyBinding(
        action: GlobalHotkeyAction.showForm,
        control: true,
        keyCode: _wpfKeyA,
      ),
    ];

    test('groups one combo with its ordered, de-duplicated actions', () {
      final groups = groupHotkeyBindings(bindings);
      expect(groups, hasLength(1));
      expect(groups.single.actions, <GlobalHotkeyAction>[
        GlobalHotkeyAction.showForm,
        GlobalHotkeyAction.systemProxySet,
      ]);
    });

    test(
      'one OS registration dispatches every bound action in order',
      () async {
        final plugin = _FakeHotkeyPlugin();
        final registrar = PluginHotkeyRegistrar(
          probe: const _FreeProbe(),
          plugin: plugin,
        );
        final fired = <GlobalHotkeyAction>[];
        final (accepted, failures) = await registrar.register(
          groupHotkeyBindings(bindings),
          onTriggered: fired.add,
        );

        expect(failures, isEmpty);
        expect(accepted, <GlobalHotkeyAction>{
          GlobalHotkeyAction.showForm,
          GlobalHotkeyAction.systemProxySet,
        });
        expect(plugin.registered, hasLength(1), reason: 'exactly one combo');
        plugin.handler!(plugin.registered.single);
        expect(fired, <GlobalHotkeyAction>[
          GlobalHotkeyAction.showForm,
          GlobalHotkeyAction.systemProxySet,
        ]);
      },
    );
  });

  group('hotkey pause is a dispatch gate', () {
    test('a combo fired while editing does not run its saved action', () async {
      final fake = _FakeRegistrar();
      final container = ProviderContainer(
        overrides: [hotkeyRegistrarProvider.overrideWithValue(fake)],
      );
      addTearDown(container.dispose);

      final fired = <GlobalHotkeyAction>[];
      container.read(hotkeyDispatchProvider).handler = fired.add;
      final controller = container.read(hotkeyControllerProvider.notifier);

      await controller.registerAll();
      expect(fake.onTriggered, isNotNull);
      fake.onTriggered!(GlobalHotkeyAction.showForm);
      expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.showForm]);

      // Editor opened: native dispatch is paused.
      await controller.beginEdit();
      expect(controller.isPaused, isTrue);
      fired.clear();
      fake.onTriggered!(GlobalHotkeyAction.showForm);
      expect(fired, isEmpty, reason: 'paused dispatch must be dropped');

      // Editor cancelled: dispatch resumes.
      await controller.cancelEdit();
      expect(controller.isPaused, isFalse);
      fake.onTriggered!(GlobalHotkeyAction.systemProxySet);
      expect(fired, <GlobalHotkeyAction>[GlobalHotkeyAction.systemProxySet]);
    });

    test('unregisterAll is best-effort and never throws', () async {
      final fake = _FakeRegistrar();
      final container = ProviderContainer(
        overrides: [hotkeyRegistrarProvider.overrideWithValue(fake)],
      );
      addTearDown(container.dispose);
      await container.read(hotkeyControllerProvider.notifier).unregisterAll();
      expect(fake.unregisterCount, 1);
    });
  });

  group('hide / startup regressions (R4-05 / FIX-15C)', () {
    test('Windows close always hides; hide is not a stop', () {
      expect(
        DesktopIntegration.hideOnClose(
          isWindows: true,
          hide2TrayWhenClose: false,
        ),
        isTrue,
      );
    });

    test('AutoHideStartup is a launch-only decision', () {
      expect(
        DesktopIntegration.shouldHideOnStartup(<String, dynamic>{
          'UiItem': <String, dynamic>{'AutoHideStartup': true},
        }),
        isTrue,
      );
      expect(
        DesktopIntegration.shouldHideOnStartup(<String, dynamic>{
          'UiItem': <String, dynamic>{'AutoHideStartup': false},
        }),
        isFalse,
      );
    });
  });
}
