// RR-08: tray menu snapshot stays continuously in sync with the shared read
// model (business / runtime / configuration changes), the node and route
// checkmarks come from the same active/default state, and the icon status
// follows the proxy/core state. No real tray click is performed; the OS tray is
// replaced by a recording surface.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

class _RecordingTraySurface implements TraySurface {
  final List<List<TrayMenuItem>> menus = <List<TrayMenuItem>>[];
  final List<TrayIconStatus> icons = <TrayIconStatus>[];

  @override
  Future<void> applyMenu(List<TrayMenuItem> items) async => menus.add(items);

  @override
  Future<void> applyIcon(TrayIconStatus status) async => icons.add(status);
}

List<TraySubEntry> _children(List<TrayMenuItem> model, String actionId) =>
    model.firstWhere((e) => e.actionId == actionId).children;

TrayReadModel _model({
  SysProxyMode mode = SysProxyMode.unchanged,
  List<TraySubEntry> routings = const <TraySubEntry>[],
  List<TraySubEntry> nodes = const <TraySubEntry>[],
  bool coreRunning = false,
  bool pacRunning = false,
}) => TrayReadModel(
  desiredMode: mode,
  pacVisible: true,
  routings: routings,
  nodes: nodes,
  serversLimit: 0,
  coreRunning: coreRunning,
  pacRunning: pacRunning,
);

void main() {
  group('trayIconStatus', () {
    test('idle when nothing runs', () {
      expect(
        trayIconStatus(
          mode: SysProxyMode.unchanged,
          coreRunning: false,
          pacRunning: false,
        ),
        TrayIconStatus.normal,
      );
    });

    test('proxyActive when ForcedChange is applied to a running session', () {
      expect(
        trayIconStatus(
          mode: SysProxyMode.forcedChange,
          coreRunning: true,
          pacRunning: false,
        ),
        TrayIconStatus.proxyActive,
      );
    });

    test('coreRunning when a session runs without a forced proxy', () {
      expect(
        trayIconStatus(
          mode: SysProxyMode.forcedClear,
          coreRunning: true,
          pacRunning: false,
        ),
        TrayIconStatus.coreRunning,
      );
    });

    test('proxyPac when PAC is serving', () {
      expect(
        trayIconStatus(
          mode: SysProxyMode.pac,
          coreRunning: false,
          pacRunning: true,
        ),
        TrayIconStatus.proxyPac,
      );
      // PAC mode without a serving PAC server is not reported as active.
      expect(
        trayIconStatus(
          mode: SysProxyMode.pac,
          coreRunning: true,
          pacRunning: false,
        ),
        TrayIconStatus.coreRunning,
      );
    });
  });

  test('sync applies the first model and de-duplicates repeats', () async {
    final surface = _RecordingTraySurface();
    final sync = TrayMenuSync(surface: surface);
    final a = _model(
      routings: const <TraySubEntry>[
        TraySubEntry(id: 'r1', label: '规则A', checked: false),
        TraySubEntry(id: 'r2', label: '规则B', checked: true),
      ],
      nodes: const <TraySubEntry>[
        TraySubEntry(id: 'n1', label: 'HK', checked: true),
        TraySubEntry(id: 'n2', label: 'US', checked: false),
      ],
    );
    await sync.update(a);
    expect(surface.menus, hasLength(1));
    expect(surface.icons, <TrayIconStatus>[TrayIconStatus.normal]);
    // Identical model is a no-op: an unrelated rebuild never touches the tray.
    await sync.update(a);
    expect(surface.menus, hasLength(1));
    expect(surface.icons, hasLength(1));
  });

  test('menu checkmarks follow the same active/default state', () async {
    final surface = _RecordingTraySurface();
    final sync = TrayMenuSync(surface: surface);
    await sync.update(
      _model(
        mode: SysProxyMode.forcedChange,
        coreRunning: true,
        routings: const <TraySubEntry>[
          TraySubEntry(id: 'r1', label: '规则A', checked: false),
          TraySubEntry(id: 'r2', label: '规则B', checked: true),
        ],
        nodes: const <TraySubEntry>[
          TraySubEntry(id: 'n1', label: 'HK', checked: false),
          TraySubEntry(id: 'n2', label: 'US', checked: true),
        ],
      ),
    );
    // A changed model re-applies and updates the icon.
    await sync.update(
      _model(
        mode: SysProxyMode.forcedChange,
        coreRunning: true,
        routings: const <TraySubEntry>[
          TraySubEntry(id: 'r1', label: '规则A', checked: true),
          TraySubEntry(id: 'r2', label: '规则B', checked: false),
        ],
        nodes: const <TraySubEntry>[
          TraySubEntry(id: 'n1', label: 'HK', checked: true),
          TraySubEntry(id: 'n2', label: 'US', checked: false),
        ],
      ),
    );
    expect(surface.icons, <TrayIconStatus>[
      TrayIconStatus.proxyActive,
      TrayIconStatus.proxyActive,
    ]);
    final menu = surface.menus.last;
    final routes = _children(menu, 'ACT-TRAY-006');
    expect(routes.firstWhere((e) => e.id == 'r1').checked, isTrue);
    expect(routes.firstWhere((e) => e.id == 'r2').checked, isFalse);
    final nodes = _children(menu, 'ACT-TRAY-007');
    expect(nodes.firstWhere((e) => e.id == 'n1').checked, isTrue);
    expect(nodes.firstWhere((e) => e.id == 'n2').checked, isFalse);
  });

  test('business change re-syncs the node checkmark through a provider', () {
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);
    final surface = _RecordingTraySurface();
    final sync = TrayMenuSync(surface: surface);

    void push() {
      final profiles = container.read(profilesControllerProvider);
      sync.update(
        _model(
          nodes: <TraySubEntry>[
            for (final p in profiles.all)
              TraySubEntry(
                id: p.id,
                label: p.remarks.isEmpty ? p.id : p.remarks,
                checked: p.id == profiles.activeId,
              ),
          ],
        ),
      );
    }

    container.listen(
      profilesControllerProvider,
      (_, _) => push(),
      fireImmediately: true,
    );
    final profiles = container.read(profilesControllerProvider);
    expect(profiles.all, isNotEmpty);
    final target = profiles.all.first.id;
    container.read(profilesControllerProvider.notifier).setActive(target);

    final nodes = _children(surface.menus.last, 'ACT-TRAY-007');
    expect(
      nodes.firstWhere((e) => e.id == target).checked,
      isTrue,
      reason: 'the active node must be the checked tray entry',
    );
    expect(
      nodes.where((e) => e.checked),
      hasLength(1),
      reason: 'exactly one node entry is checked',
    );
  });
}
